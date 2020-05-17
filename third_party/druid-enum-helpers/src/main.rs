#![allow(dead_code)]
#![allow(unused_variables)]

#[allow(unused_imports)]
use match_derive::Matcher;

#[allow(unused_imports)]
use match_macro::match_widget;

use druid::widget::{Button, Flex, Label, SizedBox};
use druid::{AppLauncher, Data, Lens, Widget, WidgetExt, WindowDesc};

#[derive(Clone, Copy, Data)]
enum Event {
    Click(u32, u32),
    Key(char),
    Unknown,
}

#[derive(Clone, Data, Lens)]
struct AppState {
    event: Event,
    option: Option<u32>,
}

fn main() {
    let window = WindowDesc::new(build_ui);

    let state = AppState {
        event: Event::Key('Z'),
        option: None,
    };

    AppLauncher::with_window(window)
        .launch(state)
        .expect("Failed to launch the application");
}

fn build_ui() -> impl Widget<AppState> {
    let matcher = match_widget! { Event,
        Event::Click(u32, u32) => Label::dynamic(
            |data: &(u32, u32), _| format!("Click at x={}, y={}", data.0, data.1)
        ),
        Event::Key(char) => {
            Button::new(|data: &char, _: &_| format!("'{}' Key", data))
                .on_click(|_, _, _| println!("Key was clicked"))
        },
        Event::Unknown => SizedBox::empty(),
    };

    let matcher2 = match_widget! { Option<u32>,
        Some(u32) => Label::dynamic(|data: &u32, _| format!("Number {}", data)),
        None => Label::new("No Number"),
    };

    Flex::column()
        .with_child(
            Button::new("Next State").on_click(|_, data: &mut AppState, _| {
                data.event = match data.event {
                    Event::Click(_, _) => Event::Key('Z'),
                    Event::Key(_) => Event::Unknown,
                    Event::Unknown => Event::Click(4, 2),
                };
                data.option = match data.option {
                    Some(_) => None,
                    None => Some(42),
                };
            }),
        )
        .with_spacer(20.0)
        .with_child(matcher.lens(AppState::event))
        .with_child(matcher2.lens(AppState::option))
        .padding(10.0)
}
