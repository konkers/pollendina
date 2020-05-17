# Druid enum helpers

In this repo I work on implementing the ideas mentioned in [druid#789](https://github.com/xi-editor/druid/issues/789)

The two sub-crates implement the macros while the main crate is the testing ground.

## match-macro

This works already!

Type inference is kinda broken :(
Also, using a `{}` macro inside the ui declaration seems to disable formatting.

Error messages are still a bit crappy when messing up types.

```rust
#[derive(Clone, Data)]
enum Event {
    Click(u32, u32),
    Key(char),
}

fn event_widget() -> druid::WidgetMatcher<Event> {
    match_widget! { Event,
        Event::Click(u32, u32) => Label::dynamic(|data, _| {
            format!("x: {}, y: {}", data.0, data.1)
        }),
        Event::Key(char) => Label::dynamic(|data, _| format!("key: {}", data))),
    }
}

fn event_widget() -> impl Widget<Event> {
    match_widget! { Event,
        Event::Click(u32, u32) => Label::dynamic(|data, _| {
            format!("x: {}, y: {}", data.0, data.1)
        }),
        _ => Label::dynamic(|data: &(), _| format!("key: unhandled"))),
    }
}
```

## match-derive

```rust
#[derive(Clone, Data, Match)]
enum Event { .. }

fn event_widget() -> impl Widget<Event> {
    Event::matcher()
        .click(Label::dynamic(|data, _| {
            format!("x: {}, y: {}", data.0, data.1)
        ))
        .key(Label::dynamic(|data, _| {
            format!("key: {}", data))
        })
}

fn event_widget() -> impl Widget<Event> {
    Event::matcher()
        .key(Label::dynamic(|data, _| {
            format!("key: {}", data))
        })
        .default(Label::new("Unhandled Event"))
    }
}

fn event_widget() -> impl Widget<Event> {
    // Will emit warning for missing variant
    // Event::Click at runtime
    Event::matcher()
        .key(Label::dynamic(|data, _| {
            format!("key: {}", data))
        })
    }
}
```

