use std::sync::Arc;

use druid::{theme, widget::BackgroundBrush, Color, Data, Key, Lens};
use serde::Deserialize;

use crate::{
    engine::{
        module::{DisplayViewInfo, DisplayViewInfoView},
        Engine, ObjectiveState,
    },
    widget::{
        constellation::{Field, Star},
        container::ContainerParams,
        dyn_flex::{DynFlexItem, DynFlexParams},
        list_iter::ListIter,
    },
};

#[derive(Clone, Data)]
pub struct DisplayChild {
    pub id: String,
    pub ty: String,
    pub state: ObjectiveState,
}

// Data for each view type is broken out here so that we can implements
// widgets on them.
#[derive(Clone, Data, Lens)]
pub struct DisplayViewGrid {
    pub columns: usize,
    pub children: Arc<Vec<DisplayChild>>,
}

#[derive(Clone, Data, Lens)]
pub struct DisplayViewCount {
    pub found: u32,
    pub total: u32,
}

#[derive(Clone, Data, Lens)]
pub struct MapObjective {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub state: ObjectiveState,
}

impl Star for MapObjective {
    fn pos(&self) -> (f64, f64) {
        (self.x, self.y)
    }

    fn radius(&self) -> f64 {
        self.radius
    }
}

#[derive(Clone, Data, Lens)]
pub struct MapInfo {
    pub id: String,
    pub width: f64,
    pub height: f64,
    // depricated
    pub objective_radius: f64,
    pub objectives: Arc<Vec<MapObjective>>,
}

impl Field for MapInfo {
    fn size(&self) -> (f64, f64) {
        (self.width, self.height)
    }
}

impl DynFlexItem for MapInfo {
    fn flex_params(&self) -> DynFlexParams {
        return 1.0.into();
    }
}

impl ListIter<MapObjective> for MapInfo {
    fn for_each(&self, cb: impl FnMut(&MapObjective, usize)) {
        self.objectives.for_each(cb)
    }
    fn for_each_mut(&mut self, cb: impl FnMut(&mut MapObjective, usize)) {
        self.objectives.for_each_mut(cb)
    }
    fn data_len(&self) -> usize {
        self.objectives.data_len()
    }
}

#[derive(Clone, Data, Lens)]
pub struct DisplayViewMap {
    pub maps: Arc<Vec<MapInfo>>,
}

#[derive(Clone, Data, Lens)]
pub struct DisplayViewFlex {
    pub children: Arc<Vec<DisplayView>>,
}

#[derive(Clone, Data, Lens)]
pub struct DisplayViewSpacer {}

#[derive(Clone, Data, Lens)]
pub struct DisplayViewTabChild {
    pub label: String,
    pub index: usize,
    pub view: DisplayView,
}

impl DynFlexItem for DisplayViewTabChild {
    fn flex_params(&self) -> DynFlexParams {
        self.view.flex_params()
    }
}

#[derive(Clone, Data, Lens)]
pub struct DisplayViewTabs {
    pub current_tab: usize,
    pub tabs: Arc<Vec<DisplayViewTabChild>>,
}

#[derive(Clone, Data)]
pub enum DisplayViewData {
    Grid(DisplayViewGrid),
    Count(DisplayViewCount),
    Map(DisplayViewMap),
    FlexRow(DisplayViewFlex),
    FlexCol(DisplayViewFlex),
    Spacer(DisplayViewSpacer),
    Tabs(DisplayViewTabs),
    None,
}

impl Default for DisplayViewData {
    fn default() -> Self {
        DisplayViewData::None
    }
}

#[derive(Clone, Data, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeColor {
    Clear,
    BgDark,
    BgLight,
}

impl Default for ThemeColor {
    fn default() -> Self {
        return ThemeColor::Clear;
    }
}

impl ThemeColor {
    pub fn color_key(&self) -> Option<Key<Color>> {
        match self {
            ThemeColor::Clear => None,
            ThemeColor::BgLight => Some(theme::BACKGROUND_LIGHT),
            ThemeColor::BgDark => Some(theme::BACKGROUND_DARK),
        }
    }
}

#[derive(Clone, Data, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum CornerRadius {
    None,
    Small,
    Large,
}

impl Default for CornerRadius {
    fn default() -> Self {
        return CornerRadius::None;
    }
}

impl Into<f64> for CornerRadius {
    fn into(self) -> f64 {
        match self {
            CornerRadius::None => 0.,
            CornerRadius::Small => 4.,
            CornerRadius::Large => 8.,
        }
    }
}

#[derive(Clone, Data, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Inset {
    None,
    Small,
    Large,
}

impl Default for Inset {
    fn default() -> Self {
        return Inset::None;
    }
}

impl Into<f64> for Inset {
    fn into(self) -> f64 {
        match self {
            Inset::None => 0.,
            Inset::Small => 4.,
            Inset::Large => 8.,
        }
    }
}

#[derive(Clone, Debug, Data, Default)]
pub struct LayoutParams {
    pub flex: f64,
    pub background: ThemeColor,
    pub corner_radius: CornerRadius,
    pub inset: Inset,
}

#[derive(Clone, Data, Default, Lens)]
pub struct DisplayView {
    pub layout_params: LayoutParams,
    pub data: DisplayViewData,
}

impl DynFlexItem for DisplayView {
    fn flex_params(&self) -> DynFlexParams {
        return self.layout_params.flex.into();
    }
}

impl ContainerParams for DisplayView {
    fn background<T>(&self) -> Option<BackgroundBrush<T>> {
        self.layout_params
            .background
            .color_key()
            .map(|c| BackgroundBrush::ColorKey(c))
    }

    fn corner_radius(&self) -> f64 {
        self.layout_params.corner_radius.clone().into()
    }

    fn inset(&self) -> f64 {
        self.layout_params.inset.clone().into()
    }
}

impl DisplayView {
    pub fn new(engine: &Engine, info: &DisplayViewInfo) -> Self {
        let data = match &info.view {
            DisplayViewInfoView::Grid {
                columns,
                objectives,
            } => DisplayViewData::Grid(DisplayViewGrid::new(engine, *columns, objectives)),
            DisplayViewInfoView::Count { objective_type } => {
                DisplayViewData::Count(DisplayViewCount::new(engine, objective_type))
            }
            DisplayViewInfoView::Map { maps } => {
                DisplayViewData::Map(DisplayViewMap::new(engine, maps))
            }
            DisplayViewInfoView::FlexRow { children } => {
                DisplayViewData::FlexRow(DisplayViewFlex::new(engine, children))
            }
            DisplayViewInfoView::FlexCol { children } => {
                DisplayViewData::FlexCol(DisplayViewFlex::new(engine, children))
            }
            DisplayViewInfoView::Spacer {} => DisplayViewData::Spacer(DisplayViewSpacer {}),
            DisplayViewInfoView::Tabs { labels, children } => {
                DisplayViewData::Tabs(DisplayViewTabs::new(engine, labels, children))
            }
            DisplayViewInfoView::Include { path: _ } => {
                panic!("encountered unprocessed display view include");
            }
        };

        DisplayView {
            layout_params: LayoutParams {
                flex: info.layout_params.flex,
                background: info.layout_params.background.clone(),
                corner_radius: info.layout_params.corner_radius.clone(),
                inset: info.layout_params.inset.clone(),
            },
            data: data,
        }
    }

    pub fn update(&mut self, engine: &Engine, info: &DisplayViewInfo) {
        match &info.view {
            DisplayViewInfoView::Grid {
                columns,
                objectives,
            } => {
                if let DisplayViewData::Grid(g) = &mut self.data {
                    g.update(engine, *columns, objectives);
                }
            }
            DisplayViewInfoView::Count { objective_type } => {
                if let DisplayViewData::Count(c) = &mut self.data {
                    c.update(engine, objective_type);
                }
            }
            DisplayViewInfoView::Map { maps: _maps } => {
                if let DisplayViewData::Map(m) = &mut self.data {
                    m.update(engine);
                }
            }
            DisplayViewInfoView::FlexRow {
                children: children_info,
            } => {
                if let DisplayViewData::FlexRow(f) = &mut self.data {
                    f.update(engine, &children_info)
                }
            }
            DisplayViewInfoView::FlexCol {
                children: children_info,
            } => {
                if let DisplayViewData::FlexCol(f) = &mut self.data {
                    f.update(engine, &children_info)
                }
            }
            DisplayViewInfoView::Spacer {} => {}
            DisplayViewInfoView::Tabs {
                labels: _labels,
                children: children_info,
            } => {
                if let DisplayViewData::Tabs(t) = &mut self.data {
                    t.update(engine, &children_info)
                }
            }
            DisplayViewInfoView::Include { path: _ } => {
                panic!("encountered unprocessed display view include");
            }
        }
    }
}

impl DisplayViewGrid {
    fn new(engine: &Engine, columns: usize, objectives: &Vec<String>) -> Self {
        let mut children = Vec::new();
        for objective in objectives {
            let ty = if let Some(o) = engine.module.objectives.get(objective) {
                o.ty.clone()
            } else {
                "unknown".into()
            };

            // All objectives start in the Locked state.  The normal
            // app lifecycle will take care of keeping them up to date.
            children.push(DisplayChild {
                id: objective.clone(),
                ty: ty,
                state: ObjectiveState::Locked,
            });
        }
        DisplayViewGrid {
            columns: columns,
            children: Arc::new(children),
        }
    }

    fn update(&mut self, engine: &Engine, columns: usize, objectives: &Vec<String>) {
        self.columns = columns;
        let mut ids = objectives.iter();
        let children = Arc::make_mut(&mut self.children);
        for child in children {
            let id = match ids.next() {
                Some(i) => i,
                None => return,
            };

            if let Some(state) = engine.objectives.get(id) {
                child.state = *state;
            }
        }
    }
}

impl DisplayViewCount {
    fn new(_engine: &Engine, _objective_type: &String) -> Self {
        DisplayViewCount { found: 0, total: 0 }
    }

    fn update(&mut self, engine: &Engine, objective_type: &String) {
        // We're filtering the objectives every update.  If this becomes a bottleneck,
        // we can cache this filtering.
        let objectives: Vec<String> = engine
            .module
            .objectives
            .iter()
            .filter(|(_, o)| o.ty == *objective_type)
            .map(|(id, _)| id.clone())
            .collect();
        let total = objectives.len();
        let mut found = 0;
        for o in objectives {
            if let Some(state) = engine.objectives.get(&o) {
                found += match state {
                    ObjectiveState::Disabled => 0,
                    ObjectiveState::Locked => 0,
                    ObjectiveState::GlitchLocked => 0,
                    ObjectiveState::Unlocked => 1,
                    ObjectiveState::Complete => 1,
                }
            }
        }

        self.found = found as u32;
        self.total = total as u32;
    }
}

impl DisplayViewMap {
    fn new(engine: &Engine, map_ids: &Vec<String>) -> Self {
        let mut maps = Vec::new();
        for id in map_ids {
            let obj_info = engine.module.maps.get(id).unwrap();
            let mut objectives = Vec::new();

            for info in &obj_info.objectives {
                objectives.push(MapObjective {
                    id: info.id.clone(),
                    x: info.x as f64,
                    y: info.y as f64,
                    radius: obj_info.objective_radius,
                    state: ObjectiveState::Locked,
                });
            }

            maps.push(MapInfo {
                id: id.clone(),
                width: obj_info.width as f64,
                height: obj_info.height as f64,
                objective_radius: obj_info.objective_radius,
                objectives: Arc::new(objectives),
            });
        }
        DisplayViewMap {
            maps: Arc::new(maps),
        }
    }

    fn update(&mut self, engine: &Engine) {
        let maps = Arc::make_mut(&mut self.maps);
        for map in maps {
            let objectives = Arc::make_mut(&mut map.objectives);
            for mut o in objectives.iter_mut() {
                if let Some(state) = engine.objectives.get(&o.id) {
                    o.state = *state;
                }
            }
        }
    }
}

impl DisplayViewFlex {
    fn new(engine: &Engine, children: &Vec<DisplayViewInfo>) -> Self {
        let mut views = Vec::new();

        for child in children {
            let view = DisplayView::new(engine, child);
            views.push(view);
        }

        DisplayViewFlex {
            children: Arc::new(views),
        }
    }

    fn update(&mut self, engine: &Engine, children_info: &Vec<DisplayViewInfo>) {
        let views = Arc::make_mut(&mut self.children);
        let mut infos = children_info.iter();

        for view in views.iter_mut() {
            let info = match infos.next() {
                Some(i) => i,
                None => return,
            };
            view.update(engine, info);
        }
    }
}

impl DisplayViewTabs {
    fn new(engine: &Engine, labels: &Vec<String>, children: &Vec<DisplayViewInfo>) -> Self {
        let mut tabs = Vec::new();

        let mut labels = labels.iter();

        for (i, child) in children.iter().enumerate() {
            let label = match labels.next() {
                Some(l) => l,
                None => continue,
            };
            let view = DisplayView::new(engine, child);
            let tab = DisplayViewTabChild {
                label: label.clone(),
                index: i,
                view,
            };
            tabs.push(tab);
        }

        DisplayViewTabs {
            current_tab: 0,
            tabs: Arc::new(tabs),
        }
    }

    fn update(&mut self, engine: &Engine, children_info: &Vec<DisplayViewInfo>) {
        let tabs = Arc::make_mut(&mut self.tabs);
        let mut infos = children_info.iter();

        for tab in tabs.iter_mut() {
            let info = match infos.next() {
                Some(i) => i,
                None => return,
            };
            tab.view.update(engine, info);
        }
    }
}
