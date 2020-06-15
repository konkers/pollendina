use std::sync::Arc;

use druid::widget::{Button, Flex, Label, Padding, ViewSwitcher};
use druid::{lens, Command, LensExt, MouseEvent, Widget, WidgetExt};

use match_macro::match_widget;

use crate::{
    engine::{
        self, DisplayChild, DisplayView, DisplayViewCount, DisplayViewData, DisplayViewFlex,
        DisplayViewGrid, DisplayViewMap, DisplayViewTabChild, DisplayViewTabs, MapInfo,
    },
    widget::{
        dyn_flex::CrossAxisAlignment, Asset, ClickExt, Constellation, Container, DynFlex, Grid,
        MapObjective, Objective, Stack,
    },
    ENGINE_TOGGLE_STATE, UI_OPEN_POPUP,
};

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
                    Asset::new()
                        .lens(MapInfo::id.map(|id| format!("map:{}", id), |_id, _new_id| {})),
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
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .must_fill_minor_axis(true)
    .lens(DisplayViewMap::maps)
}

fn flex_row_widget() -> impl Widget<DisplayViewFlex> {
    DynFlex::row(|| display_widget()).lens(DisplayViewFlex::children)
}

fn flex_col_widget() -> impl Widget<DisplayViewFlex> {
    DynFlex::column(|| display_widget()).lens(DisplayViewFlex::children)
}

fn tabs_widget() -> impl Widget<DisplayViewTabs> {
    let mut w = Flex::column();
    w.add_child(
        DynFlex::row(|| {
            Button::new(|(_, data): &(usize, DisplayViewTabChild), _env: &_| data.label.clone())
                .on_click(
                    |_ctx, (current_tab, data): &mut (usize, DisplayViewTabChild), _env| {
                        *current_tab = data.index;
                    },
                )
        })
        .lens(lens::Id.map(
            // This mapping allows display the tab buttons to change the parent `current_tab`.
            |t: &DisplayViewTabs| (t.current_tab, t.tabs.clone()),
            |t: &mut DisplayViewTabs, data: (usize, Arc<Vec<DisplayViewTabChild>>)| {
                t.current_tab = data.0;
            },
        )),
    );
    w.add_flex_child(
        ViewSwitcher::new(
            |data: &DisplayViewTabs, _env| data.current_tab,
            |selector, _data, _env| {
                Box::new(
                    display_widget()
                        .lens(DisplayViewTabChild::view)
                        .lens(lens::Id.index(*selector).in_arc())
                        .lens(DisplayViewTabs::tabs),
                )
            },
        ),
        1.0,
    );

    w
}

pub fn display_widget() -> impl Widget<DisplayView> {
    Container::new(
        (match_widget! { DisplayViewData,
            DisplayViewData::Grid(_) => grid_widget(),
            DisplayViewData::Count(_) => count_widget(),
            DisplayViewData::Map(_) => map_widget(),
            DisplayViewData::FlexRow(_) => flex_row_widget(),
            DisplayViewData::FlexCol(_) => flex_col_widget(),
            DisplayViewData::Spacer(_) => Label::new(""),
            DisplayViewData::None => Label::new(""),
            DisplayViewData::Tabs(_) => tabs_widget(),
        })
        .lens(DisplayView::data),
    )
}
