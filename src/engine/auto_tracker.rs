use async_std::{prelude::*, stream::interval, task};
use futures::{channel::mpsc, select, FutureExt, SinkExt};
use std::collections::HashMap;
use std::{io::Cursor, thread, time::Duration};

use byteorder::{LittleEndian, ReadBytesExt};
use druid::Data;
use failure::{format_err, Error};
use rlua::{self, Function, Lua, Table, UserData, UserDataMethods};
use usb2snes::Connection;

use crate::{
    engine::{EventSink, ObjectiveState},
    ENGINE_UPDATE_AUTO_TRACKER_STATE, ENGINE_UPDATE_STATE,
};

#[derive(Clone, Debug)]
struct ObjectiveStateData(ObjectiveState);

impl UserData for ObjectiveStateData {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(_methods: &mut M) {}
}

#[derive(Debug)]
struct MemWatch {
    address: u32,
    len: usize,
    callback_index: u32,
}

#[derive(Clone)]
struct MemData {
    data: Vec<u8>,
}

impl UserData for MemData {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("get_u8", |_, data, offset: usize| {
            if offset < data.data.len() {
                Ok(data.data[offset])
            } else {
                Ok(0x0u8)
            }
        });

        methods.add_method("get_u16", |_, data, offset: usize| {
            if offset < data.data.len() - 1 {
                let mut c = Cursor::new(&data.data[offset..]);
                c.read_u16::<LittleEndian>()
                    .map_err(|e| rlua::Error::external(e))
            } else {
                Ok(0x0)
            }
        });

        methods.add_method("get_u24", |_, data, offset: usize| {
            if offset < data.data.len() - 2 {
                let mut c = Cursor::new(&data.data[offset..]);
                c.read_u24::<LittleEndian>()
                    .map_err(|e| rlua::Error::external(e))
            } else {
                Ok(0x0)
            }
        });

        methods.add_method("get_u32", |_, data, offset: usize| {
            if offset < data.data.len() - 3 {
                let mut c = Cursor::new(&data.data[offset..]);
                c.read_u32::<LittleEndian>()
                    .map_err(|e| rlua::Error::external(e))
            } else {
                Ok(0x0)
            }
        });
    }
}

pub(crate) struct AutoTracker {
    control_channel: mpsc::UnboundedReceiver<AutoTrackerCommand>,
    state: AutoTrackerState,
    lua: Lua,
}

enum AutoTrackerCommand {
    Start,
    Stop,
}

#[derive(Clone, Data, Debug, PartialEq)]
pub enum AutoTrackerState {
    Idle,
    Connecting,
    Running,
}

pub(crate) struct AutoTrackerController {
    control_channel: mpsc::UnboundedSender<AutoTrackerCommand>,
}

impl AutoTrackerController {
    pub async fn start(&mut self) -> Result<(), Error> {
        self.control_channel
            .send(AutoTrackerCommand::Start)
            .await
            .map_err(|e| format_err!("error sending start message: {}", e))
    }

    pub async fn stop(&mut self) -> Result<(), Error> {
        self.control_channel
            .send(AutoTrackerCommand::Stop)
            .await
            .map_err(|e| format_err!("error sending start message: {}", e))
    }
}

impl AutoTracker {
    pub fn new<T: 'static + EventSink + Send>(
        script: &String,
        event_sink: T,
    ) -> Result<AutoTrackerController, Error> {
        let lua = Lua::new();

        lua.context(|ctx| -> Result<(), Error> {
            let globals = ctx.globals();

            globals.set(
                "OBJECTIVE_LOCKED",
                ObjectiveStateData(ObjectiveState::Locked),
            )?;
            globals.set(
                "OBJECTIVE_GLITCH_LOCKED",
                ObjectiveStateData(ObjectiveState::GlitchLocked),
            )?;
            globals.set(
                "OBJECTIVE_UNLOCKED",
                ObjectiveStateData(ObjectiveState::Unlocked),
            )?;
            globals.set(
                "OBJECTIVE_COMPLETE",
                ObjectiveStateData(ObjectiveState::Complete),
            )?;

            let mem_watch = ctx.create_table()?;
            globals.set("__mem_watch", mem_watch)?;

            ctx.globals().set(
                "add_mem_watch",
                ctx.create_function(|ctx, (address, len, callback): (u32, usize, Function)| {
                    let globals = ctx.globals();
                    let watches = globals.get::<_, Table>("__mem_watch")?;
                    let entry = ctx.create_table()?;
                    entry.set("address", address)?;
                    entry.set("len", len)?;
                    entry.set("callback", callback)?;
                    watches.set(watches.len()? + 1, entry)?;

                    Ok(())
                })?,
            )?;

            ctx.load(&script).set_name("auto_tracker")?.exec()?;
            Ok(())
        })?;

        let (tx, rx) = mpsc::unbounded();

        let tracker = AutoTracker {
            control_channel: rx,
            state: AutoTrackerState::Idle,
            lua,
        };

        tracker.start(event_sink);

        Ok(AutoTrackerController {
            control_channel: tx,
        })
    }

    async fn sample<T: EventSink>(&mut self, c: &mut Connection, sink: &T) -> Result<(), Error> {
        let watches = self.lua.context(|ctx| -> Result<_, Error> {
            let mut watches = Vec::new();
            let globals = ctx.globals();
            let watches_table = globals.get::<_, Table>("__mem_watch")?;
            for pair in watches_table.pairs::<u32, Table>() {
                let (index, table) = pair?;
                let address = table.get::<_, u32>("address")?;
                let len = table.get::<_, usize>("len")?;
                watches.push(MemWatch {
                    address,
                    len,
                    callback_index: index,
                });
            }
            Ok(watches)
        })?;

        let mut bufs = Vec::new();
        for watch in &watches {
            let mut buf = vec![0u8; watch.len as usize];
            c.read_mem(watch.address, &mut buf).await?;
            bufs.push(MemData { data: buf });
        }

        let mut updates = HashMap::new();

        self.lua.context(|ctx| -> Result<(), Error> {
            let globals = ctx.globals();
            let watches_table = globals.get::<_, Table>("__mem_watch")?;

            // updates is protected by this scope.
            ctx.scope(|scope| -> Result<(), Error> {
                ctx.globals().set(
                    "set_objective_state",
                    scope.create_function_mut(|_, (id, state): (String, ObjectiveStateData)| {
                        updates.insert(id, state.0);
                        Ok(())
                    })?,
                )?;

                for (i, watch) in watches.iter().enumerate() {
                    let buf = &bufs[i];
                    let table = watches_table.get::<_, Table>(watch.callback_index)?;
                    let callback = table.get::<_, Function>("callback")?;
                    callback.call::<_, ()>(buf.clone())?;
                }
                Ok(())
            })?;
            Ok(())
        })?;

        sink.submit_command(ENGINE_UPDATE_STATE, updates, None)
            .map_err(|e| format_err!("Failed to send command: {}", e))
    }

    async fn auto_track<T: EventSink>(&mut self, sink: T) -> Result<(), Error> {
        let mut ticker = interval(Duration::from_millis(500));
        let mut connection = None;
        loop {
            select! {
                cmd = self.control_channel.next().fuse() => {
                    if let Some(cmd) = cmd {
                        match cmd {
                            AutoTrackerCommand::Start => {
                                self.state = AutoTrackerState::Connecting;
                                let mut c = Connection::new("ws://localhost:8080").await?;
                                let devs = c.get_device_list().await?;
                                let dev = devs[0].to_string();
                                println!("Attaching to {}.", dev);
                                c.attach(&dev).await?;

                                connection = Some(c);
                                self.state = AutoTrackerState::Running;
                            },
                            AutoTrackerCommand::Stop => {
                                connection = None;
                                self.state = AutoTrackerState::Idle;
                            }
                        }
                    } else {
                        // Control channel dropped.  We're done here.
                        return Ok(());
                    }
                },
                _ = ticker.next().fuse() => {
                    if let Some(c) = connection.as_mut() {
                        self.sample(c, &sink).await?;
                    }
                },
            };
            sink.submit_command(ENGINE_UPDATE_AUTO_TRACKER_STATE, self.state.clone(), None)
                .map_err(|e| format_err!("Failed to send state: {}", e))?;
        }
    }

    fn start<T: 'static + EventSink + Send>(mut self, sink: T) {
        thread::spawn(move || {
            task::block_on(self.auto_track(sink)).expect("oops");
        });
    }
}
