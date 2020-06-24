#![recursion_limit = "256"]
#![windows_subsystem = "windows"]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use druid::widget::{Button, Checkbox, Flex, Label, List, Padding, TextBox};
use druid::{
    platform_menus, AppDelegate, AppLauncher, Command, Data, DelegateCtx, Env, ExtEventError,
    ExtEventSink, LocalizedString, MenuDesc, Point, Selector, Target, Widget, WidgetExt,
    WindowDesc, WindowId,
};
use failure::{format_err, Error};
use match_macro::match_widget;

mod assets;
mod engine;
mod views;
mod widget;

use engine::{
    AutoTrackerState, CheckBoxParamValue, DisplayState, Engine, EventSink, Module, ModuleParam,
    ModuleParamValue, ObjectiveState,
};
use views::display_widget;
use widget::ModalHost;

pub(crate) const UI_OPEN_CONFIG: Selector<()> = Selector::new("ui:open_config");
pub(crate) const UI_CANCEL_CONFIG: Selector<()> = Selector::new("ui:cancel_config");
pub(crate) const UI_APPLY_CONFIG: Selector<()> = Selector::new("ui:update_config");
const UI_OPEN_POPUP: Selector<((f64, f64), String)> = Selector::new("ui:open_popup");

pub(crate) const UI_OPEN_BROADCAST: Selector<()> = Selector::new("ui:open_broadcast");

pub(crate) const ENGINE_TOGGLE_STATE: Selector<String> = Selector::new("engine:toggle_state");
pub(crate) const ENGINE_UPDATE_STATE: Selector<HashMap<String, ObjectiveState>> =
    Selector::new("engine:update_state");
pub(crate) const ENGINE_DUMP_STATE: Selector<()> = Selector::new("engine:dump_state");

pub(crate) const ENGINE_UPDATE_AUTO_TRACKER_STATE: Selector<AutoTrackerState> =
    Selector::new("engine:update_auto_tracker_state");
pub(crate) const ENGINE_START_AUTO_TRACKING: Selector<()> =
    Selector::new("engine:start_auto_tracking");
pub(crate) const ENGINE_STOP_AUTO_TRACKING: Selector<()> =
    Selector::new("engine:stop_auto_tracking");

#[derive(Clone)]
struct ExtEventSinkProxy(ExtEventSink);

impl EventSink for ExtEventSinkProxy {
    fn submit_command<T: 'static + Send + Sync>(
        &self,
        sel: Selector<T>,
        obj: impl Into<Box<T>>,
        target: impl Into<Option<Target>>,
    ) -> Result<(), ExtEventError> {
        self.0.submit_command(sel, obj, target)
    }
}

struct Delegate {
    engine: Engine,
}

impl Delegate {
    fn close_config_window(&self, data: &mut DisplayState, ctx: &mut DelegateCtx) {
        match *data.config_win {
            Some(id) => {
                let command = Command::new(druid::commands::CLOSE_WINDOW, ());
                ctx.submit_command(command, id);
            }
            None => println!("tried closing config window when not open"),
        }
    }
}

impl AppDelegate<DisplayState> for Delegate {
    fn command(
        &mut self,
        ctx: &mut DelegateCtx,
        _target: Target,
        cmd: &Command,
        data: &mut DisplayState,
        _env: &Env,
    ) -> bool {
        if cmd.is(UI_OPEN_BROADCAST) {
            match *data.broadcast_win {
                Some(id) => {
                    let command = Command::new(druid::commands::SHOW_WINDOW, ());
                    ctx.submit_command(command, id);
                }
                None => {
                    self.engine.update_param_state(data);
                    let mut window = WindowDesc::new(broadcast_ui_builder).title("Broadcast View");

                    if let Some(size) = self.engine.broadcast_window_size() {
                        window = window.window_size(size).resizable(false);
                    }
                    let win_id = window.id;
                    ctx.new_window(window);
                    *Arc::make_mut(&mut data.broadcast_win) = Some(win_id);
                }
            };
            false
        } else if cmd.is(UI_OPEN_CONFIG) {
            match *data.config_win {
                Some(id) => {
                    let command = Command::new(druid::commands::SHOW_WINDOW, ());
                    ctx.submit_command(command, id);
                }
                None => {
                    self.engine.update_param_state(data);
                    let window = WindowDesc::new(config_ui_builder).menu(app_menu());
                    let win_id = window.id;
                    ctx.new_window(window);
                    *Arc::make_mut(&mut data.config_win) = Some(win_id);
                }
            };
            false
        } else if cmd.is(UI_CANCEL_CONFIG) {
            println!("canceling config changes");
            self.close_config_window(data, ctx);
            false
        } else if cmd.is(UI_APPLY_CONFIG) {
            println!("applying config changes");
            if let Err(e) = self.engine.save_param_state(data) {
                println!("error saving config changes: {}", e);
            }
            self.close_config_window(data, ctx);
            false
        } else if let Some(payload) = cmd.get(UI_OPEN_POPUP) {
            if let Err(e) = self.engine.build_popup(data, &payload.1) {
                println!("error building popup: {}", e);
            } else {
                let pos = Point::new((payload.0).0, (payload.0).1);
                let cmd = ModalHost::make_modal_command(pos, modal_builder);
                ctx.submit_command(cmd, None);
            }
            false
        } else if let Some(id) = cmd.get(ENGINE_TOGGLE_STATE) {
            if let Err(e) = self.engine.toggle_state(&id) {
                println!("error toggling state: {}", e);
            } else {
                self.engine.update_display_state(data);
            }
            true
        } else if cmd.is(ENGINE_START_AUTO_TRACKING) {
            if let Err(e) = self.engine.start_auto_tracking() {
                println!("error starting auto tracking: {}", e);
            }
            true
        } else if cmd.is(ENGINE_STOP_AUTO_TRACKING) {
            if let Err(e) = self.engine.stop_auto_tracking() {
                println!("error stopping auto tracking: {}", e);
            }
            true
        } else if let Some(state) = cmd.get(ENGINE_UPDATE_AUTO_TRACKER_STATE) {
            data.auto_tracker_state = state.clone();
            true
        } else if let Some(updates) = cmd.get(ENGINE_UPDATE_STATE) {
            if let Err(e) = self.engine.update_state(updates) {
                println!("error updating state: {}", e);
            } else {
                self.engine.update_display_state(data);
            }
            true
        } else if cmd.is(ENGINE_DUMP_STATE) {
            if let Err(e) = self.engine.dump_state() {
                println!("Error dumping state: {}", e);
            }
            true
        } else {
            true
        }
    }
    fn window_removed(
        &mut self,
        id: WindowId,
        data: &mut DisplayState,
        _env: &Env,
        _ctx: &mut DelegateCtx,
    ) {
        if let Some(config_win_id) = *data.config_win {
            if id == config_win_id {
                *Arc::make_mut(&mut data.config_win) = None;
            }
        }
    }
}

fn get_exe_dir() -> Result<PathBuf, Error> {
    let mut p = std::env::current_exe()?;

    p.pop();

    Ok(p)
}

fn get_dev_path() -> Result<PathBuf, Error> {
    // For development with `cargo run` we end up in target/<buildtype>/<exe>
    let mut p = get_exe_dir()?;
    p.pop(); // pop <buildtype>
    p.pop(); // pop target

    Ok(p)
}

#[cfg(target_os = "macos")]
fn get_pkg_path() -> Result<PathBuf, Error> {
    // In a Mac app the executable lives in Contents/MacOS/<exe> and
    // resources live in Contents/Resources.
    let mut p = get_exe_dir()?;
    p.pop(); // pop MacOS
    p.push("Resources");

    Ok(p)
}

#[cfg(target_os = "windows")]
fn get_pkg_path() -> Result<PathBuf, Error> {
    // On Windows we're installed in bin/pollendina.exe and mods are in mods.
    let mut p = get_exe_dir()?;
    p.pop(); // pop bin
    Ok(p)
}

#[cfg(target_os = "linux")]
fn get_pkg_path() -> Result<PathBuf, Error> {
    Ok("/usr/lib/pollendina".into())
}

fn get_mod_paths() -> Vec<PathBuf> {
    [get_dev_path, get_pkg_path, get_exe_dir, || {
        std::env::current_dir().map_err(From::from)
    }]
    .iter()
    .filter_map(|f| f().ok())
    .collect()
}

fn resolve_module_path<P: AsRef<Path>>(path: P) -> Result<PathBuf, Error> {
    let path = path.as_ref();
    let mut dirs = get_mod_paths();

    for dir in &mut dirs {
        dir.push(&path);
        if dir.exists() {
            println!("found {}", dir.to_string_lossy());
            return Ok(dir.clone());
        }
    }

    Err(format_err!(
        "Can't find {:?} in {:?}",
        path.to_string_lossy(),
        &dirs
    ))
}

fn main() -> Result<(), Error> {
    let main_window = WindowDesc::new(ui_builder)
        .menu(app_menu())
        .title("Pollendina")
        .window_size((650., 500.))
        .with_min_size((650., 500.));
    let app = AppLauncher::with_window(main_window);

    println!("{:?}", std::env::current_exe());
    let module_path = resolve_module_path("mods/ff4fe/manifest.json")?;
    let module = Module::open(&module_path)?;
    let engine = Engine::new(module, ExtEventSinkProxy(app.get_external_handle()))?;
    let data = engine.new_display_state();

    //    let auto_tracker = AutoTracker::new(ki_info, app.get_external_handle());

    app.delegate(Delegate { engine })
        .launch(data)
        .expect("launch failed");
    println!("done");

    Ok(())
}

fn modal_builder() -> impl Widget<DisplayState> {
    display_widget().lens(DisplayState::popup)
}

fn ui_builder() -> impl Widget<DisplayState> {
    let mut root = Flex::column();

    let mut top = Flex::row();
    top.add_child(
        Button::new(|data: &AutoTrackerState, _env: &_| {
            if *data == AutoTrackerState::Idle {
                "Start auto tracking".into()
            } else {
                "Stop auto tracking".into()
            }
        })
        .on_click(|ctx, data: &mut AutoTrackerState, _env| {
            let cmd = if *data == AutoTrackerState::Idle {
                Command::new(ENGINE_START_AUTO_TRACKING, ())
            } else {
                Command::new(ENGINE_STOP_AUTO_TRACKING, ())
            };
            ctx.submit_command(cmd, None);
        })
        .lens(DisplayState::auto_tracker_state),
    );
    top.add_child(
        Label::new(|data: &AutoTrackerState, _env: &_| format!("{:?}", data))
            .lens(DisplayState::auto_tracker_state),
    );
    top.add_flex_spacer(1.0);
    top.add_child(Button::new("Dump").on_click(|ctx, _data, _env| {
        ctx.submit_command(Command::new(ENGINE_DUMP_STATE, ()), None);
    }));
    top.add_child(Button::new("Broadcast View").on_click(|ctx, _data, _env| {
        ctx.submit_command(Command::new(UI_OPEN_BROADCAST, ()), None);
    }));
    top.add_child(Button::new("Config").on_click(|ctx, _data, _env| {
        ctx.submit_command(Command::new(UI_OPEN_CONFIG, ()), None);
    }));
    root.add_child(Padding::new(8.0, top));

    root.add_flex_child(display_widget().lens(DisplayState::layout), 1.0);

    let root = ModalHost::new(root);
    // root.debug_paint_layout()
    root
}

fn config_ui_builder() -> impl Widget<DisplayState> {
    let mut root = Flex::column();

    root.add_child(
        List::new(|| {
            let mut row = Flex::row();
            row.add_child(
                Label::new(|data: &String, _env: &_| format!("{}:", data)).lens(ModuleParam::name),
            );
            row.add_flex_child(
                match_widget! { ModuleParamValue,
                    ModuleParamValue::TextBox(_) => TextBox::new(),
                    ModuleParamValue::CheckBox(_) => Checkbox::new("").lens(CheckBoxParamValue::value),
                }
                .expand_width()
                .lens(ModuleParam::value),
                1.0,
            );
            row
        })
        .lens(DisplayState::params),
    );

    root.add_flex_spacer(1.0);
    root.add_child(
        Flex::row()
            .with_flex_spacer(1.0)
            .with_child(
                Button::new("Ok").on_click(|ctx, _data: &mut DisplayState, _env| {
                    let cmd = Command::new(UI_APPLY_CONFIG, ());
                    ctx.submit_command(cmd, None);
                }),
            )
            .with_child(
                Button::new("Cancel").on_click(|ctx, _data: &mut DisplayState, _env| {
                    let cmd = Command::new(UI_CANCEL_CONFIG, ());
                    ctx.submit_command(cmd, None);
                }),
            ),
    );

    //root.debug_paint_layout()
    root.padding(8.0)
}

fn broadcast_ui_builder() -> impl Widget<DisplayState> {
    display_widget().lens(DisplayState::broadcast)
    /*
    ViewSwitcher::new(
        |data, _env| data.broadcast,
        |selector, data, env| {
            match selector {
                Some(w)
    Either::new(
        |data: &DisplayState, _env| -> bool {
            match data.broadcast {
                Some(_) => true,
                None => false,
            }
        },
        display_widget()
            .lens(lens::Id.map(|x: &Option<DisplayView>| x.unwrap(), |x, y| {}))
            .lens(DisplayState::broadcast),
        Label::new(""),
    )
    */
}

#[allow(unused_mut)]
pub(crate) fn app_menu() -> MenuDesc<DisplayState> {
    let mut menu = MenuDesc::empty();
    #[cfg(target_os = "macos")]
    {
        menu = menu.append(platform_menus::mac::application::default());
        menu = menu.append(edit_menu());
    }

    menu
}

#[allow(unused)]
fn edit_menu<T: Data>() -> MenuDesc<T> {
    MenuDesc::new(LocalizedString::new("common-menu-edit-menu"))
        .append(platform_menus::common::undo())
        .append(platform_menus::common::redo())
        .append_separator()
        .append(platform_menus::common::cut().disabled())
        .append(platform_menus::common::copy())
        .append(platform_menus::common::paste())
}
