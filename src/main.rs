#![recursion_limit = "256"]
#![windows_subsystem = "windows"]

use std::collections::HashMap;
use std::sync::Arc;

use druid::widget::{Button, Checkbox, Flex, Label, List, Padding, TextBox};
use druid::{
    platform_menus, AppDelegate, AppLauncher, Command, Data, DelegateCtx, Env, ExtEventError,
    ExtEventSink, LensExt, LocalizedString, MenuDesc, MouseEvent, Point, Selector, Target, Widget,
    WidgetExt, WindowDesc, WindowId,
};
use failure::Error;
use match_macro::match_widget;

mod assets;
mod engine;
mod widget;

use engine::{
    AutoTrackerState, CheckBoxParamValue, DisplayChild, DisplayState, DisplayView,
    DisplayViewCount, DisplayViewData, DisplayViewFlex, DisplayViewGrid, DisplayViewMap, Engine,
    EventSink, Module, ModuleParam, ModuleParamValue, ObjectiveState,
};
use widget::{
    Asset, ClickExt, Constellation, Container, DynFlex, Grid, MapObjective, ModalHost, Objective,
    Stack,
};

pub(crate) const UI_OPEN_CONFIG: Selector<()> = Selector::new("ui:open_config");
pub(crate) const UI_CANCEL_CONFIG: Selector<()> = Selector::new("ui:cancel_config");
pub(crate) const UI_APPLY_CONFIG: Selector<()> = Selector::new("ui:update_config");
const UI_OPEN_POPUP: Selector<((f64, f64), String)> = Selector::new("ui:open_popup");

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
        if cmd.is(UI_OPEN_CONFIG) {
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

fn main() -> Result<(), Error> {
    let main_window = WindowDesc::new(ui_builder)
        .menu(app_menu())
        .show_titlebar(false);
    let app = AppLauncher::with_window(main_window);

    let module = Module::open("mods/ff4fe/manifest.json")?;
    let engine = Engine::new(module, ExtEventSinkProxy(app.get_external_handle()))?;
    let data = engine.new_display_state();

    //    let auto_tracker = AutoTracker::new(ki_info, app.get_external_handle());

    app.delegate(Delegate { engine })
        .launch(data)
        .expect("launch failed");
    println!("done");

    Ok(())
}

fn grid_widget() -> impl Widget<DisplayViewGrid> {
    Grid::new(|| {
        Padding::new(
            2.0,
            Objective::new().on_click(|ctx, data: &mut DisplayChild, _env| {
                let cmd = Command::new(ENGINE_TOGGLE_STATE, data.id.clone());
                ctx.submit_command(cmd, None);
            }),
        )
    })
}

fn count_widget() -> impl Widget<DisplayViewCount> {
    Label::new(|data: &DisplayViewCount, _env: &_| format!("{} / {}", data.found, data.total))
}

fn map_widget() -> impl Widget<DisplayViewMap> {
    DynFlex::column(|| {
        Padding::new(
            8.0,
            Stack::new()
                .with_child(
                    Asset::new().lens(
                        engine::MapInfo::id.map(|id| format!("map:{}", id), |_id, _new_id| {}),
                    ),
                )
                .with_child(Constellation::new(|| {
                    MapObjective::new()
                        .lens(engine::MapObjective::state)
                        .on_left_click(
                            |ctx, event: &MouseEvent, data: &mut engine::MapObjective, _env| {
                                // We're sending window based position here and the
                                // modal host uses widget local coordinates.  This
                                // works out only because it's placed at the window
                                // origin.
                                let id = data.id.clone();
                                /*let cmd = ModalHost::make_modal_command(event.window_pos, || {
                                    modal_builder(id)
                                });
                                */
                                let pos = event.window_pos;
                                let cmd = UI_OPEN_POPUP.with(((pos.x, pos.y), id));
                                ctx.submit_command(cmd, None);
                            },
                        )
                })),
        )
    })
    .lens(DisplayViewMap::maps)
}

fn flex_row_widget() -> impl Widget<DisplayViewFlex> {
    DynFlex::row(|| display_widget()).lens(DisplayViewFlex::children)
}

fn flex_col_widget() -> impl Widget<DisplayViewFlex> {
    DynFlex::column(|| display_widget()).lens(DisplayViewFlex::children)
}

fn display_widget() -> impl Widget<DisplayView> {
    Container::new(
        (match_widget! { DisplayViewData,
            DisplayViewData::Grid(_) => grid_widget(),
            DisplayViewData::Count(_) => count_widget(),
            DisplayViewData::Map(_) => map_widget(),
            DisplayViewData::FlexRow(_) => flex_row_widget(),
            DisplayViewData::FlexCol(_) => flex_col_widget(),
            DisplayViewData::Spacer(_) => Label::new(""),
            DisplayViewData::None => Label::new(""),
        })
        .lens(DisplayView::data),
    )
}

fn modal_builder() -> impl Widget<DisplayState> {
    display_widget().lens(DisplayState::popup)
}

fn ui_builder() -> impl Widget<DisplayState> {
    let mut root = Flex::column();
    root.add_flex_child(
        ModalHost::new(display_widget().lens(DisplayState::layout)),
        1.0,
    );

    let mut bot = Flex::row();
    bot.add_child(
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
    bot.add_child(
        Label::new(|data: &AutoTrackerState, _env: &_| format!("{:?}", data))
            .lens(DisplayState::auto_tracker_state),
    );
    bot.add_flex_spacer(1.0);
    bot.add_child(Button::new("Dump").on_click(|ctx, _data, _env| {
        ctx.submit_command(Command::new(ENGINE_DUMP_STATE, ()), None);
    }));
    bot.add_child(Button::new("Config").on_click(|ctx, _data, _env| {
        ctx.submit_command(Command::new(UI_OPEN_CONFIG, ()), None);
    }));
    root.add_child(Padding::new(8.0, bot));
    //root.debug_paint_layout()
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

#[allow(unused_mut)]
pub(crate) fn app_menu() -> MenuDesc<DisplayState> {
    let mut menu = MenuDesc::empty();
    #[cfg(target_os = "macos")]
    {
        menu = menu.append(platform_menus::mac::application::default());
    }

    menu.append(edit_menu())
}

fn edit_menu<T: Data>() -> MenuDesc<T> {
    MenuDesc::new(LocalizedString::new("common-menu-edit-menu"))
        .append(platform_menus::common::undo())
        .append(platform_menus::common::redo())
        .append_separator()
        .append(platform_menus::common::cut().disabled())
        .append(platform_menus::common::copy())
        .append(platform_menus::common::paste())
}
