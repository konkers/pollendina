use async_std::task;
use std::collections::HashMap;
use std::sync::Arc;

use druid::{Data, ExtEventError, Lens, Selector, Target, WindowId};
use failure::{format_err, Error};

mod auto_tracker;
pub mod module;

pub use module::{DisplayViewInfo, Module, Param};

pub use auto_tracker::AutoTrackerState;
use auto_tracker::{AutoTracker, AutoTrackerController};

pub trait EventSink {
    fn submit_command<T: 'static + Send>(
        &self,
        sel: Selector,
        obj: impl Into<Option<T>>,
        target: impl Into<Option<Target>>,
    ) -> Result<(), ExtEventError>;
}

#[derive(Clone, Copy, Data, Debug, PartialEq)]
pub enum ObjectiveState {
    Disabled,
    Locked,
    GlitchLocked,
    Unlocked,
    Complete,
}

#[derive(Clone, Data)]
pub struct DisplayChild {
    pub id: String,
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

#[derive(Clone, Data)]
pub enum DisplayView {
    Grid(DisplayViewGrid),
    Count(DisplayViewCount),
}

#[derive(Clone, Data, PartialEq)]
pub enum ModuleParamValue {
    TextBox(String),
}

#[derive(Clone, Data, Lens, PartialEq)]
pub struct ModuleParam {
    name: String,
    value: ModuleParamValue,
}

// DisplayState is owned by the UI and should contain all the information
// it needs to function.
#[derive(Clone, Data, Lens)]
pub struct DisplayState {
    pub views: Arc<Vec<DisplayView>>,
    pub params: Arc<Vec<ModuleParam>>,
    pub auto_tracker_state: AutoTrackerState,
    pub config_win: Arc<Option<WindowId>>,
}

pub struct Engine {
    module: Module,
    objectives: HashMap<String, ObjectiveState>,
    auto_tracker: Option<AutoTrackerController>,
}

impl Engine {
    pub fn new<T: 'static + EventSink + Clone + Send>(
        module: Module,
        event_sink: T,
    ) -> Result<Engine, Error> {
        let mut objectives = HashMap::new();
        for (id, _) in module.objectives.iter() {
            objectives.insert(id.clone(), ObjectiveState::Locked);
        }

        let auto_tracker = match &module.auto_track {
            Some(script) => Some(AutoTracker::new(script, event_sink.clone())?),
            None => None,
        };

        Ok(Engine {
            module,
            objectives,
            auto_tracker,
        })
    }

    pub fn new_display_state(&self) -> DisplayState {
        let mut views = Vec::new();

        for info in &self.module.manifest.display {
            let view = match info {
                DisplayViewInfo::Grid {
                    columns,
                    objectives,
                } => {
                    let mut children = Vec::new();
                    for objective in objectives {
                        // All objectives start in the Locked state.  The normal
                        // app lifecycle will take care of keeping them up to date.
                        children.push(DisplayChild {
                            id: objective.clone(),
                            state: ObjectiveState::Locked,
                        });
                    }
                    DisplayView::Grid(DisplayViewGrid {
                        columns: *columns,
                        children: Arc::new(children),
                    })
                }
                DisplayViewInfo::Count {
                    objective_type: _objective_type,
                } => DisplayView::Count(DisplayViewCount { found: 0, total: 0 }),
            };
            views.push(view);
        }
        let mut params = Vec::new();
        for p in &self.module.manifest.params {
            let (name, value) = match p {
                Param::TextBox { name } => (name.clone(), ModuleParamValue::TextBox("".into())),
            };
            params.push(ModuleParam { name, value });
        }

        let mut state = DisplayState {
            views: Arc::new(views),
            params: Arc::new(params),
            auto_tracker_state: AutoTrackerState::Idle,
            config_win: Arc::new(None),
        };
        self.update_display_state(&mut state);

        state
    }

    fn update_grid_state(
        &self,
        view: &mut DisplayViewGrid,
        columns: &usize,
        objectives: &Vec<String>,
    ) {
        view.columns = *columns;
        let mut ids = objectives.iter();
        let children = Arc::make_mut(&mut view.children);
        for child in children {
            let id = match ids.next() {
                Some(i) => i,
                None => return,
            };

            if let Some(state) = self.objectives.get(id) {
                child.state = *state;
            }
        }
    }

    fn update_count_state(&self, view: &mut DisplayViewCount, objective_type: &String) {
        // We're filtering the objectives every update.  If this becomes a bottleneck,
        // we can cache this filtering.
        let objectives: Vec<String> = self
            .module
            .objectives
            .iter()
            .filter(|(_, o)| o.ty == *objective_type)
            .map(|(id, _)| id.clone())
            .collect();
        let total = objectives.len();
        let mut found = 0;
        for o in objectives {
            if let Some(state) = self.objectives.get(&o) {
                found += match state {
                    ObjectiveState::Disabled => 0,
                    ObjectiveState::Locked => 0,
                    ObjectiveState::GlitchLocked => 0,
                    ObjectiveState::Unlocked => 1,
                    ObjectiveState::Complete => 1,
                }
            }
        }

        view.found = found as u32;
        view.total = total as u32;
    }

    pub fn update_display_state(&self, data: &mut DisplayState) {
        let views = Arc::make_mut(&mut data.views);
        let mut infos = self.module.manifest.display.iter();
        for view in views.iter_mut() {
            let info = match infos.next() {
                Some(i) => i,
                None => return,
            };

            match info {
                DisplayViewInfo::Grid {
                    columns,
                    objectives,
                } => {
                    if let DisplayView::Grid(g) = view {
                        self.update_grid_state(g, columns, objectives);
                    }
                }
                DisplayViewInfo::Count { objective_type } => {
                    if let DisplayView::Count(c) = view {
                        self.update_count_state(c, objective_type);
                    }
                }
            }
        }
    }

    pub fn toggle_state(&mut self, id: &String) -> Result<(), Error> {
        if let Some(o) = self.objectives.get_mut(id) {
            let new_state = match *o {
                ObjectiveState::Disabled => ObjectiveState::Disabled,
                ObjectiveState::Locked => ObjectiveState::Unlocked,
                ObjectiveState::GlitchLocked => ObjectiveState::Unlocked,
                ObjectiveState::Unlocked => ObjectiveState::Complete,
                ObjectiveState::Complete => ObjectiveState::Locked,
            };
            *o = new_state;
            Ok(())
        } else {
            Err(format_err!("toggle_state: id {} not found", &id))
        }
    }

    pub fn start_auto_tracking(&mut self) -> Result<(), Error> {
        if let Some(tracker) = &mut self.auto_tracker {
            println!("starting");
            task::block_on(tracker.start())
                .map_err(|e| format_err!("could not send start tracker message: {}", e))
        } else {
            println!("no auto tracker");
            Err(format_err!("no auto tracker support in this module"))
        }
    }

    pub fn stop_auto_tracking(&mut self) -> Result<(), Error> {
        if let Some(tracker) = &mut self.auto_tracker {
            task::block_on(tracker.stop())
                .map_err(|e| format_err!("could not send stop tracker message: {}", e))
        } else {
            Err(format_err!("no auto tracker support in this module"))
        }
    }

    pub fn update_state(&mut self, updates: &HashMap<String, ObjectiveState>) {
        for (id, state) in updates {
            self.objectives.insert(id.clone(), state.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn load_fe_module() -> Result<(), Error> {
        // While we are bootstrapping everything we'll be using the FE module for
        // tests.  Eventually the unique cases should be extracted into `test_data/mod`
        Module::open("mods/ff4fe/manifest.json")?;
        Ok(())
    }
}
