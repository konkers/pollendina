use std::sync::Arc;

use super::AutoTrackerState;
use druid::{Data, Lens, WindowId};

mod view;
pub use view::{
    CornerRadius, DisplayChild, DisplayView, DisplayViewCount, DisplayViewData, DisplayViewFlex,
    DisplayViewGrid, DisplayViewMap, DisplayViewSpacer, DisplayViewTabChild, DisplayViewTabs,
    Inset, LayoutParams, MapInfo, MapObjective, ThemeColor,
};

#[derive(Clone, Data, Lens, PartialEq)]
pub struct CheckBoxParamValue {
    pub id: String,
    pub value: bool,
}

#[derive(Clone, Data, PartialEq)]
pub enum ModuleParamValue {
    TextBox(String),
    CheckBox(CheckBoxParamValue),
}

#[derive(Clone, Data, Lens, PartialEq)]
pub struct ModuleParam {
    pub name: String,
    pub value: ModuleParamValue,
}

// DisplayState is owned by the UI and should contain all the information
// it needs to function.
#[derive(Clone, Data, Lens)]
pub struct DisplayState {
    pub layout: DisplayView,
    pub popup: DisplayView,
    pub broadcast: DisplayView,
    pub params: Arc<Vec<ModuleParam>>,
    pub auto_tracker_state: AutoTrackerState,
    pub config_win: Arc<Option<WindowId>>,
    pub broadcast_win: Arc<Option<WindowId>>,
}
