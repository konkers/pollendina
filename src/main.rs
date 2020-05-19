#![recursion_limit = "256"]

use std::sync::Arc;

use druid::widget::{Button, Flex, Label, List, Padding, TextBox};
use druid::{
    platform_menus, AppDelegate, AppLauncher, Command, Data, DelegateCtx, Env, LocalizedString,
    MenuDesc, MenuItem, Selector, Target, Widget, WidgetExt, WindowDesc, WindowId,
};
use failure::Error;
use match_macro::match_widget;

mod assets;
mod engine;
mod widget;

use engine::{
    AutoTrackerState, DisplayChild, DisplayState, DisplayView, DisplayViewCount, Engine, Module,
    ModuleParam, ModuleParamValue, ENGINE_START_AUTO_TRACKING, ENGINE_STOP_AUTO_TRACKING,
    ENGINE_TOGGLE_STATE,
};
use widget::{Grid, Objective};

pub(crate) const UI_OPEN_CONFIG: Selector = Selector::new("ui:open_config");
pub(crate) const UI_CANCEL_CONFIG: Selector = Selector::new("ui:cancel_config");
pub(crate) const UI_APPLY_CONFIG: Selector = Selector::new("ui:update_config");

struct Delegate {
    engine: Engine,
}

impl Delegate {
    fn close_config_window(&self, data: &mut DisplayState, ctx: &mut DelegateCtx) {
        match *data.config_win {
            Some(id) => {
                let command = Command::new(druid::commands::CLOSE_WINDOW, id);
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
        target: Target,
        cmd: &Command,
        data: &mut DisplayState,
        env: &Env,
    ) -> bool {
        match cmd.selector {
            UI_OPEN_CONFIG => {
                match *data.config_win {
                    Some(id) => {
                        let command = Command::new(druid::commands::SHOW_WINDOW, id);
                        ctx.submit_command(command, id);
                    }
                    None => {
                        let window = WindowDesc::new(config_ui_builder).menu(app_menu());
                        let win_id = window.id;
                        ctx.new_window(window);
                        *Arc::make_mut(&mut data.config_win) = Some(win_id);
                    }
                };
                false
            }
            UI_CANCEL_CONFIG => {
                println!("canceling config changes");
                self.close_config_window(data, ctx);
                false
            }
            UI_APPLY_CONFIG => {
                println!("applying config changes");
                self.close_config_window(data, ctx);
                false
            }
            _ => self.engine.command(ctx, target, cmd, data, env),
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
    let main_window = WindowDesc::new(ui_builder).menu(app_menu());
    let app = AppLauncher::with_window(main_window);

    let module = Module::open("mods/ff4fe/manifest.json")?;
    let engine = Engine::new(module, app.get_external_handle())?;
    let data = engine.new_display_state();

    //    let auto_tracker = AutoTracker::new(ki_info, app.get_external_handle());

    app.delegate(Delegate { engine })
        .launch(data)
        .expect("launch failed");
    println!("done");

    Ok(())
}

fn ui_builder() -> impl Widget<DisplayState> {
    let mut root = Flex::column();
    root.add_child(
        Padding::new(
            8.0,
            List::new(|| {
                match_widget! { DisplayView,
                        DisplayView::Grid(_) =>
                    Grid::new(|| {
                        Padding::new(
                            2.0,
                            Objective::new().on_click(|ctx, data: &mut DisplayChild, _env| {
                                let cmd = Command::new(ENGINE_TOGGLE_STATE, data.id.clone());
                                ctx.submit_command(cmd, None);
                            }),
                        )
                    }),
                    DisplayView::Count(_) => Label::new(|data: &DisplayViewCount, _env: &_| {
                        format!("{} / {}", data.found, data.total)
                    })

                }
            }),
        )
        .lens(DisplayState::views),
    );
    root.add_flex_spacer(1.0);
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
                Command::new(ENGINE_START_AUTO_TRACKING, 0)
            } else {
                Command::new(ENGINE_STOP_AUTO_TRACKING, 0)
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
    bot.add_child(Button::new("Config").on_click(|ctx, _data, _env| {
        ctx.submit_command(Command::new(UI_OPEN_CONFIG, 0), None);
    }));
    root.add_child(bot);
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
                    let cmd = Command::new(UI_APPLY_CONFIG, 0);
                    ctx.submit_command(cmd, None);
                }),
            )
            .with_child(
                Button::new("Cancel").on_click(|ctx, _data: &mut DisplayState, _env| {
                    let cmd = Command::new(UI_CANCEL_CONFIG, 0);
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
