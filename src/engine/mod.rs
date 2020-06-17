use async_std::task;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

use druid::{Data, ExtEventError, Selector, Target};
use failure::{format_err, Error};
use petgraph::{algo::toposort, graph::DiGraph};

mod auto_tracker;
mod display;
pub mod expression;
pub mod module;

pub use display::{
    CheckBoxParamValue, CornerRadius, DisplayChild, DisplayState, DisplayView, DisplayViewCount,
    DisplayViewData, DisplayViewFlex, DisplayViewGrid, DisplayViewMap, DisplayViewSpacer,
    DisplayViewTabChild, DisplayViewTabs, Inset, LayoutParams, MapInfo, MapObjective, ModuleParam,
    ModuleParamValue, ThemeColor,
};
use expression::Expression;
pub use module::{
    DisplayViewInfo, DisplayViewInfoView, LayoutParamsInfo, Module, ObjectiveList,
    ObjectiveListSpecial, Param,
};

use crate::assets::{add_image_to_cache, add_objective_to_cache, IMAGES};
pub use auto_tracker::AutoTrackerState;
use auto_tracker::{AutoTracker, AutoTrackerController};

pub trait EventSink {
    fn submit_command<T: 'static + Send + Sync>(
        &self,
        sel: Selector<T>,
        obj: impl Into<Box<T>>,
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

impl ObjectiveState {
    pub fn at_least(&self, threshold: &Self) -> bool {
        self.ordinal() >= threshold.ordinal()
    }

    pub fn is(&self, threshold: &Self) -> bool {
        self == threshold
    }

    fn ordinal(&self) -> u32 {
        match self {
            ObjectiveState::Disabled => 0,
            ObjectiveState::Locked => 1,
            ObjectiveState::GlitchLocked => 2,
            ObjectiveState::Unlocked => 3,
            ObjectiveState::Complete => 4,
        }
    }
}

pub struct Engine {
    module: Module,
    popup_info: DisplayViewInfo,
    broadcast_info: Option<DisplayViewInfo>,
    objectives: HashMap<String, ObjectiveState>,
    eval_order: Vec<String>,
    auto_tracker: Option<AutoTrackerController>,

    // Active checks.  This will need to be redone for pinned objectives.
    checks: Vec<String>,
}

impl Engine {
    pub fn new<T: 'static + EventSink + Clone + Send>(
        module: Module,
        event_sink: T,
    ) -> Result<Engine, Error> {
        let mut objectives = HashMap::new();
        for (id, _) in module.objectives.iter() {
            objectives.insert(id.clone(), ObjectiveState::Disabled);
        }

        let auto_tracker = match &module.auto_track {
            Some(script) => Some(AutoTracker::new(script, event_sink.clone())?),
            None => None,
        };
        let eval_order = Self::calc_eval_order(&module)?;

        // Load all the assets into the asset store.
        IMAGES.with(|images| -> Result<(), Error> {
            let mut store = images.borrow_mut();
            for asset in &module.assets {
                let data = fs::read(&asset.path)?;
                if asset.id.starts_with("map:") {
                    // Don't cal
                    add_image_to_cache(&mut store, &asset.id, &data);
                } else {
                    add_objective_to_cache(&mut store, &asset.id, &data);
                }
            }

            Ok(())
        })?;

        let popup_info = module
            .manifest
            .layouts
            .get(&"checks".to_string())
            .ok_or(format_err!("Can't find 'checks' layout"))?
            .clone();

        let broadcast_info = module
            .manifest
            .layouts
            .get(&"boradcast".to_string())
            .map(|o| o.clone());

        let mut engine = Engine {
            module,
            popup_info: popup_info,
            broadcast_info: broadcast_info,
            objectives,
            eval_order,
            auto_tracker,
            checks: Vec::new(),
        };

        engine.eval_objectives()?;

        Ok(engine)
    }

    pub fn calc_eval_order(module: &Module) -> Result<Vec<String>, Error> {
        // `petgraph` requires indexes to be integers so we first enumerate our
        // objectives and assign the integer indexes.  We keep maps from
        // id -> index and index -> id so we can create the graph then
        // return the topological sort order by id.
        //
        // We expect the node count to be fairly low so this conversion
        // happening once at module load and updates requiring several
        // HashMap lookups.  If this becomes a performance bottleneck,
        // we can switch to storing everything in a Vec and converting
        // ids to indexes at module load and keeping the that way.
        let mut id_map = HashMap::new();
        let mut index_map = HashMap::new();
        let mut index = 0;

        for (id, _) in &module.objectives {
            id_map.insert(index, id.clone());
            index_map.insert(id.clone(), index);
            index += 1;
        }

        // Generate a list of edges a tuples of (node, dependant node).
        // Dependencies come from the objective unlocked_by and
        // enabled_by expressions.
        let mut edges = Vec::new();
        for (id, info) in &module.objectives {
            let idx = index_map.get(id).unwrap();
            let mut deps = info.enabled_by.deps();
            deps.append(&mut info.unlocked_by.deps());
            deps.append(&mut info.completed_by.deps());

            // TODO(konkers): we could de-dup these for a performance gain.
            for dep in deps {
                if let Some(dep_idx) = index_map.get(&dep) {
                    edges.push((*dep_idx, *idx));
                } else {
                    println!("unknown id {}", dep);
                }
            }
        }

        let graph = DiGraph::<u32, ()>::from_edges(&edges);

        // A topological sort gives us a static traversal order allowing
        // os to propagate objective state changes in a single pass.
        let nodes = toposort(&graph, None)
            .map_err(|e| format_err!("cycle detected in objective dependencies: {:?}", e))?;

        // Convert the eval_order back into a Vec of String ids.
        let mut eval_order = Vec::new();
        for node_index in nodes {
            eval_order.push(id_map.get(&(node_index.index() as u32)).unwrap().clone());
        }

        Ok(eval_order)
    }

    fn eval_objectives(&mut self) -> Result<(), Error> {
        for id in &self.eval_order {
            let info = self
                .module
                .objectives
                .get(id)
                .ok_or(format_err!("Can't get info for objective '{}'", id))?;

            let mut state = *self
                .objectives
                .get(id)
                .ok_or(format_err!("can't get objective state for '{}`", id))?;

            if info.enabled_by != Expression::Manual {
                let enabled = info.enabled_by.evaluate_enabled(&self.objectives)?;
                if state == ObjectiveState::Disabled && enabled {
                    state = ObjectiveState::Locked;
                }
            }

            if info.unlocked_by != Expression::Manual {
                let unlocked = info.unlocked_by.evaluate_unlocked(&self.objectives)?;
                if state == ObjectiveState::Locked && unlocked {
                    state = ObjectiveState::Unlocked;
                }
            }

            if info.completed_by != Expression::Manual {
                let completed = info.completed_by.evaluate_unlocked(&self.objectives)?;
                if completed {
                    state = ObjectiveState::Complete;
                }
                if state == ObjectiveState::Complete && !completed {
                    state = ObjectiveState::Unlocked;
                }
            }

            if info.unlocked_by != Expression::Manual {
                let unlocked = info.unlocked_by.evaluate_unlocked(&self.objectives)?;
                // Re-lock if a dependencies become locked.
                if state == ObjectiveState::Unlocked && !unlocked {
                    state = ObjectiveState::Locked;
                }
            }

            if info.enabled_by != Expression::Manual {
                let enabled = info.enabled_by.evaluate_enabled(&self.objectives)?;
                if !enabled {
                    state = ObjectiveState::Disabled;
                }
            }
            *self
                .objectives
                .get_mut(id)
                .ok_or(format_err!("can't get objective state for '{}`", id))? = state;
        }
        Ok(())
    }
    pub fn new_display_state(&self) -> DisplayState {
        let layout = DisplayView::new(
            self,
            &self
                .module
                .manifest
                .layouts
                .get(&"main".to_string())
                .unwrap(),
        );
        let mut params = Vec::new();
        for p in &self.module.manifest.params {
            let (name, value) = match p {
                Param::TextBox { name } => (name.clone(), ModuleParamValue::TextBox("".into())),
                Param::CheckBox { id, name } => (
                    name.clone(),
                    ModuleParamValue::CheckBox(CheckBoxParamValue {
                        id: id.clone(),
                        value: false,
                    }),
                ),
            };
            params.push(ModuleParam { name, value });
        }

        let popup = DisplayView::new(self, &self.popup_info);
        let broadcast = match &self.broadcast_info {
            Some(info) => DisplayView::new(self, info),
            None => Default::default(),
        };

        let mut state = DisplayState {
            layout: layout,
            popup: popup,
            broadcast,
            params: Arc::new(params),
            auto_tracker_state: AutoTrackerState::Idle,
            config_win: Arc::new(None),
            broadcast_win: Arc::new(None),
        };
        self.update_display_state(&mut state);

        state
    }

    pub fn update_display_state(&self, data: &mut DisplayState) {
        let layout = self
            .module
            .manifest
            .layouts
            .get(&"main".to_string())
            .unwrap();
        data.layout.update(self, layout);
        data.popup.update(self, &self.popup_info);
        if let Some(info) = &self.broadcast_info {
            data.broadcast.update(self, info);
        }
    }

    pub fn update_param_state(&self, data: &mut DisplayState) {
        let params = Arc::make_mut(&mut data.params).iter_mut();
        for p in params {
            if let ModuleParamValue::CheckBox(v) = &mut p.value {
                let state = match self.objectives.get(&v.id) {
                    Some(state) => state,
                    None => continue,
                };

                v.value = match state {
                    ObjectiveState::Disabled => false,
                    _ => true,
                }
            }
        }
    }

    pub fn save_param_state(&mut self, data: &mut DisplayState) -> Result<(), Error> {
        for p in &*data.params {
            if let ModuleParamValue::CheckBox(v) = &p.value {
                let new_state = if v.value {
                    ObjectiveState::Unlocked
                } else {
                    ObjectiveState::Disabled
                };
                *self
                    .objectives
                    .get_mut(&v.id)
                    .ok_or(format_err!("objective {} not found", &v.id))? = new_state;
            }
        }
        self.eval_objectives()?;
        self.update_display_state(data);

        Ok(())
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
            self.eval_objectives()?;
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

    pub fn update_state(&mut self, updates: &HashMap<String, ObjectiveState>) -> Result<(), Error> {
        for (id, state) in updates {
            self.objectives.insert(id.clone(), state.clone());
            self.eval_objectives()?;
        }
        Ok(())
    }

    pub fn build_popup(&mut self, data: &mut DisplayState, id: &String) -> Result<(), Error> {
        let obj = self
            .module
            .objectives
            .get(id)
            .ok_or(format_err!("Can't find objective {}", id))?;

        let mut checks = Vec::new();
        for check in &obj.checks {
            checks.push(check.id.clone());
        }
        self.checks = checks;

        // Recreate pop view with new checks set.
        data.popup = DisplayView::new(self, &self.popup_info);

        self.update_display_state(data);
        Ok(())
    }

    pub fn broadcast_window_size(&self) -> Option<(f64, f64)> {
        let (w, h) = if let Some(info) = &self.broadcast_info {
            (
                info.layout_params.window_width,
                info.layout_params.window_height,
            )
        } else {
            (0., 0.)
        };

        if w > 0. && h > 0. {
            Some((w, h))
        } else {
            None
        }
    }

    pub fn dump_state(&self) -> Result<(), Error> {
        for id in &self.eval_order {
            let obj = self
                .module
                .objectives
                .get(id)
                .ok_or(format_err!("Can't find objective {}", id))?;
            let state = self
                .objectives
                .get(id)
                .ok_or(format_err!("Can't find objective state {}", id))?;

            println!("{}:", id);
            println!("  state: {:?}", state);
            println!("  enabled_by: {:?}", obj.enabled_by);
            println!("  unlocked_by: {:?}", obj.unlocked_by);
            println!("  completed_by: {:?}", obj.completed_by);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestEventSink;
    impl EventSink for TestEventSink {
        fn submit_command<T: 'static + Send + Sync>(
            &self,
            _sel: Selector<T>,
            _obj: impl Into<Box<T>>,
            _target: impl Into<Option<Target>>,
        ) -> Result<(), ExtEventError> {
            Ok(())
        }
    }

    fn assert_state(engine: &Engine, id: &str, state: ObjectiveState) {
        assert_eq!(*engine.objectives.get(id).unwrap(), state);
    }

    fn update_state(engine: &mut Engine, updates: &[(&str, ObjectiveState)]) -> Result<(), Error> {
        let updates = updates.iter().map(|x| (x.0.to_string(), x.1)).collect();

        engine.update_state(&updates)
    }

    #[test]
    fn load_fe_module() -> Result<(), Error> {
        // While we are bootstrapping everything we'll be using the FE module for
        // tests.  Eventually the unique cases should be extracted into `test_data/mod`
        let module = Module::open("mods/ff4fe/manifest.json")?;
        let mut engine = Engine::new(module, TestEventSink)?;
        let _state = engine.new_display_state();

        // Make sure assets loaded.
        IMAGES.with(|images| {
            assert!(images
                .borrow()
                .get(&"objective:pan:locked".into())
                .is_some());
        });

        // Make sure we have a map.
        assert_ne!(engine.module.maps.len(), 0);

        // Depending on gating, some objectives start out unlocked and
        // others locked.
        assert_state(&engine, &"baron", ObjectiveState::Unlocked);
        assert_state(&engine, &"fabul", ObjectiveState::Unlocked);
        assert_state(&engine, &"d-castle", ObjectiveState::Locked);
        assert_state(&engine, &"bahamut-cave", ObjectiveState::Locked);

        // Dwarf Castle should still be locked if Magma Key is only Unlocked.
        update_state(&mut engine, &[("magma-key", ObjectiveState::Unlocked)])?;
        assert_state(&engine, &"d-castle", ObjectiveState::Locked);

        // Completing Magma Key now unlocks Dwarf Castle.
        update_state(&mut engine, &[("magma-key", ObjectiveState::Complete)])?;
        assert_state(&engine, &"d-castle", ObjectiveState::Unlocked);

        // Un-completing the Magma Key should re-locks Dwarf Castle.
        update_state(&mut engine, &[("magma-key", ObjectiveState::Unlocked)])?;
        assert_state(&engine, &"d-castle", ObjectiveState::Locked);

        // Unlocking Darkness Crystal is enough to unlock Moon objectives.
        update_state(
            &mut engine,
            &[("darkness-crystal", ObjectiveState::Complete)],
        )?;
        assert_state(&engine, &"bahamut-cave", ObjectiveState::Unlocked);

        // Completing D. Mist slot should complete Mist Cave.
        update_state(&mut engine, &[("mist-cave:0", ObjectiveState::Complete)])?;
        assert_state(&engine, &"mist-cave", ObjectiveState::Complete);

        // Completing all non-disabled checks should cause the location to be
        // completed.  We need to turn on Nchars to ensure the char check is
        // disabled.
        update_state(&mut engine, &[("flag-n-chars", ObjectiveState::Unlocked)])?;
        assert_state(&engine, &"mt-ordeals:0", ObjectiveState::Disabled);
        assert_state(&engine, &"mt-ordeals", ObjectiveState::Unlocked);
        update_state(
            &mut engine,
            &[
                ("mt-ordeals:1", ObjectiveState::Complete),
                ("mt-ordeals:2", ObjectiveState::Complete),
                ("mt-ordeals-key-item-check", ObjectiveState::Complete),
                ("mt-ordeals:4", ObjectiveState::Complete),
            ],
        )?;
        assert_state(&engine, &"mt-ordeals", ObjectiveState::Complete);

        // baron has 5 objectives that are not gated and 2 that are gated by
        // the baron key.  It should:
        // * start Unlocked due to the 5 non-gated checks.
        // * should transition to Locked when those are complete.
        // * should transition to Unlocked when the baron-key is unlocked.
        // * should transition to Complete once the last two checks are complete.
        assert_state(&engine, &"baron", ObjectiveState::Unlocked);
        update_state(
            &mut engine,
            &[
                ("baron:0", ObjectiveState::Complete),
                ("baron:3", ObjectiveState::Complete),
                ("baron:4", ObjectiveState::Complete),
                ("baron:5", ObjectiveState::Complete),
                ("baron-inn-key-item-check", ObjectiveState::Complete),
            ],
        )?;
        assert_state(&engine, &"baron", ObjectiveState::Locked);
        update_state(&mut engine, &[("baron-key", ObjectiveState::Unlocked)])?;
        assert_state(&engine, &"baron", ObjectiveState::Unlocked);
        update_state(
            &mut engine,
            &[
                ("baron:1", ObjectiveState::Complete),
                ("baron:2", ObjectiveState::Complete),
            ],
        )?;
        assert_state(&engine, &"baron", ObjectiveState::Complete);

        // Damncyan's character is gated by !Nchars.
        update_state(&mut engine, &[("flag-n-chars", ObjectiveState::Disabled)])?;
        assert_state(&engine, &"damcyan:0", ObjectiveState::Unlocked);
        assert_state(&engine, &"damcyan", ObjectiveState::Unlocked);
        update_state(&mut engine, &[("flag-n-chars", ObjectiveState::Unlocked)])?;
        assert_state(&engine, &"damcyan:0", ObjectiveState::Disabled);
        assert_state(&engine, &"damcyan", ObjectiveState::Disabled);

        Ok(())
    }
}
