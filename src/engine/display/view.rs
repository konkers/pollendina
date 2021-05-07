use std::sync::Arc;

use druid::{theme, widget::BackgroundBrush, Color, Data, Key, Lens};
use serde::Deserialize;

use crate::{
    engine::{
        module::{DisplayViewInfo, DisplayViewInfoView, NodeList, NodeListSpecial},
        Engine, NodeState,
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
    pub state: NodeState,
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
pub struct MapNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub state: NodeState,
}

impl Star for MapNode {
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
    pub nodes: Arc<Vec<MapNode>>,
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

impl ListIter<MapNode> for MapInfo {
    fn for_each(&self, cb: impl FnMut(&MapNode, usize)) {
        self.nodes.for_each(cb)
    }
    fn for_each_mut(&mut self, cb: impl FnMut(&mut MapNode, usize)) {
        self.nodes.for_each_mut(cb)
    }
    fn data_len(&self) -> usize {
        self.nodes.data_len()
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
            DisplayViewInfoView::Grid { columns, nodes } => {
                DisplayViewData::Grid(DisplayViewGrid::new(engine, *columns, nodes))
            }
            DisplayViewInfoView::Count { node_type } => {
                DisplayViewData::Count(DisplayViewCount::new(engine, node_type))
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
            DisplayViewInfoView::Grid { columns, nodes } => {
                if let DisplayViewData::Grid(g) = &mut self.data {
                    g.update(engine, *columns, nodes);
                }
            }
            DisplayViewInfoView::Count { node_type } => {
                if let DisplayViewData::Count(c) = &mut self.data {
                    c.update(engine, node_type);
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
    fn deref_nodes<'a>(engine: &'a Engine, nodes: &'a NodeList) -> &'a Vec<String> {
        match nodes {
            NodeList::List(nodes) => nodes,
            NodeList::Special(NodeListSpecial::Checks) => &engine.checks,
        }
    }

    fn new(engine: &Engine, columns: usize, nodes: &NodeList) -> Self {
        let mut children = Vec::new();
        let nodes = Self::deref_nodes(engine, nodes);
        for node in nodes {
            let ty = if let Some(o) = engine.module.nodes.get(node) {
                o.ty.clone()
            } else {
                "unknown".into()
            };

            // All nodes start in the Locked state.  The normal
            // app lifecycle will take care of keeping them up to date.
            children.push(DisplayChild {
                id: node.clone(),
                ty: ty,
                state: NodeState::Locked,
            });
        }
        DisplayViewGrid {
            columns: columns,
            children: Arc::new(children),
        }
    }

    fn update(&mut self, engine: &Engine, columns: usize, nodes: &NodeList) {
        self.columns = columns;
        let nodes = Self::deref_nodes(engine, nodes);
        let mut ids = nodes.iter();
        let children = Arc::make_mut(&mut self.children);
        for child in children {
            let id = match ids.next() {
                Some(i) => i,
                None => return,
            };

            if let Some(state) = engine.nodes.get(id) {
                child.state = *state;
            }
        }
    }
}

impl DisplayViewCount {
    fn new(_engine: &Engine, _node_type: &String) -> Self {
        DisplayViewCount { found: 0, total: 0 }
    }

    fn update(&mut self, engine: &Engine, node: &String) {
        // We're filtering the nodes every update.  If this becomes a bottleneck,
        // we can cache this filtering.
        let nodes: Vec<String> = engine
            .module
            .nodes
            .iter()
            .filter(|(_, o)| o.ty == *node)
            .map(|(id, _)| id.clone())
            .collect();
        let total = nodes.len();
        let mut found = 0;
        for o in nodes {
            if let Some(state) = engine.nodes.get(&o) {
                found += match state {
                    NodeState::Disabled => 0,
                    NodeState::Locked => 0,
                    NodeState::GlitchLocked => 0,
                    NodeState::Unlocked => 1,
                    NodeState::Complete => 1,
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
            let mut nodes = Vec::new();

            for info in &obj_info.nodes {
                nodes.push(MapNode {
                    id: info.id.clone(),
                    x: info.x as f64,
                    y: info.y as f64,
                    radius: obj_info.node_radius,
                    state: NodeState::Locked,
                });
            }

            maps.push(MapInfo {
                id: id.clone(),
                width: obj_info.width as f64,
                height: obj_info.height as f64,
                nodes: Arc::new(nodes),
            });
        }
        DisplayViewMap {
            maps: Arc::new(maps),
        }
    }

    fn update(&mut self, engine: &Engine) {
        let maps = Arc::make_mut(&mut self.maps);
        for map in maps {
            let nodes = Arc::make_mut(&mut map.nodes);
            for mut o in nodes.iter_mut() {
                if let Some(state) = engine.nodes.get(&o.id) {
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
