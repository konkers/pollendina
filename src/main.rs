#![recursion_limit = "256"]

use druid::widget::{Button, Flex, Label, List, Padding};
use druid::{AppLauncher, Command, Widget, WidgetExt, WindowDesc};
use failure::Error;
use match_macro::match_widget;

mod assets;
//mod auto_tracker;
mod engine;
mod widget;

//use auto_tracker::AutoTracker;
use engine::{
    AutoTrackerState, DisplayChild, DisplayState, DisplayView, DisplayViewCount, Engine, Module,
    ENGINE_START_AUTO_TRACKING, ENGINE_STOP_AUTO_TRACKING, ENGINE_TOGGLE_STATE,
};

use widget::{Grid, Objective};
fn main() -> Result<(), Error> {
    let main_window = WindowDesc::new(ui_builder);
    let app = AppLauncher::with_window(main_window);

    let module = Module::open("mods/ff4fe/manifest.json")?;
    let engine = Engine::new(module, app.get_external_handle())?;
    let data = engine.new_display_state();

    //    let auto_tracker = AutoTracker::new(ki_info, app.get_external_handle());

    app.delegate(engine).launch(data).expect("launch failed");
    println!("done");

    Ok(())
}

fn ui_builder() -> impl Widget<DisplayState> {
    println!("building");
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
    root.add_child(Padding::new(
        (0.0, 20.0, 0.0, 0.0),
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
    ));
    root.add_child(
        Label::new(|data: &AutoTrackerState, _env: &_| format!("{:?}", data))
            .lens(DisplayState::auto_tracker_state),
    );
    root.add_spacer(8.0);
    root
}
