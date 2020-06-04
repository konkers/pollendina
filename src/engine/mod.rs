use async_std::task;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

use druid::{Data, ExtEventError, Lens, Selector, Target, WindowId};
use failure::{format_err, Error};
use petgraph::{algo::toposort, graph::DiGraph};

mod auto_tracker;
pub mod expression;
pub mod module;

use expression::Expression;
pub use module::{DisplayViewInfo, Module, Param};

use crate::assets::{add_image_to_cache, add_objective_to_cache, IMAGES};
use crate::widget::dyn_flex::{DynFlexItem, DynFlexParams};

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
    pub fn is(&self, threshold: &Self) -> bool {
        self.ordinal() >= threshold.ordinal()
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
    pub flex: f64,
}

#[derive(Clone, Data, Lens)]
pub struct DisplayViewCount {
    pub found: u32,
    pub total: u32,
    pub flex: f64,
}

#[derive(Clone, Data, Lens)]
pub struct MapObjective {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub state: ObjectiveState,
}

#[derive(Clone, Data, Lens)]
pub struct MapInfo {
    pub id: String,
    pub objective_radius: f64,
    pub objectives: Arc<Vec<MapObjective>>,
}

impl DynFlexItem for MapInfo {
    fn flex_params(&self) -> DynFlexParams {
        return 1.0.into();
    }
}

#[derive(Clone, Data, Lens)]
pub struct DisplayViewMap {
    pub maps: Arc<Vec<MapInfo>>,
    pub flex: f64,
}

#[derive(Clone, Data, Lens)]
pub struct DisplayViewFlex {
    pub children: Arc<Vec<DisplayView>>,
    pub flex: f64,
}

#[derive(Clone, Data)]
pub enum DisplayView {
    Grid(DisplayViewGrid),
    Count(DisplayViewCount),
    Map(DisplayViewMap),
    FlexRow(DisplayViewFlex),
    FlexCol(DisplayViewFlex),
}

impl DynFlexItem for DisplayView {
    fn flex_params(&self) -> DynFlexParams {
        match self {
            DisplayView::Grid(g) => g.flex,
            DisplayView::Count(c) => c.flex,
            DisplayView::Map(m) => m.flex,
            DisplayView::FlexRow(f) => f.flex,
            DisplayView::FlexCol(f) => f.flex,
        }
        .into()
    }
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
    pub layout: DisplayView,
    pub params: Arc<Vec<ModuleParam>>,
    pub auto_tracker_state: AutoTrackerState,
    pub config_win: Arc<Option<WindowId>>,
}

pub struct Engine {
    module: Module,
    objectives: HashMap<String, ObjectiveState>,
    eval_order: Vec<String>,
    auto_tracker: Option<AutoTrackerController>,
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
                if asset.id.starts_with("objective:") {
                    add_objective_to_cache(&mut store, &asset.id, &data);
                } else {
                    add_image_to_cache(&mut store, &asset.id, &data);
                }
            }

            Ok(())
        })?;

        let mut engine = Engine {
            module,
            objectives,
            eval_order,
            auto_tracker,
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
                if state == ObjectiveState::Locked && !enabled {
                    state = ObjectiveState::Disabled;
                }
            }

            if info.unlocked_by != Expression::Manual {
                let unlocked = info.unlocked_by.evaluate_unlocked(&self.objectives)?;
                if state == ObjectiveState::Locked && unlocked {
                    state = ObjectiveState::Unlocked;
                }
                // Re-lock if a dependencies become locked.
                if state == ObjectiveState::Unlocked && !unlocked {
                    state = ObjectiveState::Locked;
                }
            }

            println!(" => {:?}", &state);
            *self
                .objectives
                .get_mut(id)
                .ok_or(format_err!("can't get objective state for '{}`", id))? = state;
        }
        Ok(())
    }

    fn new_view(&self, info: &DisplayViewInfo) -> DisplayView {
        match info {
            DisplayViewInfo::Grid {
                columns,
                objectives,
                flex,
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
                    flex: *flex,
                })
            }
            DisplayViewInfo::Count {
                objective_type: _objective_type,
                flex,
            } => DisplayView::Count(DisplayViewCount {
                found: 0,
                total: 0,
                flex: *flex,
            }),
            DisplayViewInfo::Map {
                maps: map_ids,
                flex,
            } => {
                let mut maps = Vec::new();
                for id in map_ids {
                    let obj_info = self.module.maps.get(id).unwrap();
                    let mut objectives = Vec::new();

                    for info in &obj_info.objectives {
                        objectives.push(MapObjective {
                            id: info.id.clone(),
                            x: info.x as f64,
                            y: info.y as f64,
                            state: ObjectiveState::Locked,
                        });
                    }

                    maps.push(MapInfo {
                        id: id.clone(),
                        objective_radius: obj_info.objective_radius,
                        objectives: Arc::new(objectives),
                    });
                }
                DisplayView::Map(DisplayViewMap {
                    maps: Arc::new(maps),
                    flex: *flex,
                })
            }
            DisplayViewInfo::FlexRow { children, flex } => DisplayView::FlexRow(DisplayViewFlex {
                children: Arc::new(self.new_sub_layout(children)),
                flex: *flex,
            }),
            DisplayViewInfo::FlexCol { children, flex } => DisplayView::FlexCol(DisplayViewFlex {
                children: Arc::new(self.new_sub_layout(children)),
                flex: *flex,
            }),
        }
    }

    fn new_sub_layout(&self, infos: &Vec<DisplayViewInfo>) -> Vec<DisplayView> {
        let mut views = Vec::new();

        for info in infos {
            let view = self.new_view(&info);
            views.push(view);
        }

        views
    }

    pub fn new_display_state(&self) -> DisplayState {
        let layout = self.new_view(&self.module.manifest.layout);
        let mut params = Vec::new();
        for p in &self.module.manifest.params {
            let (name, value) = match p {
                Param::TextBox { name } => (name.clone(), ModuleParamValue::TextBox("".into())),
            };
            params.push(ModuleParam { name, value });
        }

        let mut state = DisplayState {
            layout: layout,
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

    fn update_map_state(&self, view: &mut DisplayViewMap) {
        let maps = Arc::make_mut(&mut view.maps);
        for map in maps {
            let objectives = Arc::make_mut(&mut map.objectives);
            for mut o in objectives.iter_mut() {
                if let Some(state) = self.objectives.get(&o.id) {
                    o.state = *state;
                }
            }
        }
    }

    fn update_view(&self, view: &mut DisplayView, info: &DisplayViewInfo) {
        match info {
            DisplayViewInfo::Grid {
                columns,
                objectives,
                flex: _flex,
            } => {
                if let DisplayView::Grid(g) = view {
                    self.update_grid_state(g, columns, objectives);
                }
            }
            DisplayViewInfo::Count {
                objective_type,
                flex: _flex,
            } => {
                if let DisplayView::Count(c) = view {
                    self.update_count_state(c, objective_type);
                }
            }
            DisplayViewInfo::Map {
                maps: _maps,
                flex: _flex,
            } => {
                if let DisplayView::Map(m) = view {
                    self.update_map_state(m);
                }
            }
            DisplayViewInfo::FlexRow {
                children: children_info,
                flex: _flex,
            } => {
                if let DisplayView::FlexRow(f) = view {
                    self.update_sub_layout(&mut f.children, &children_info)
                }
            }
            DisplayViewInfo::FlexCol {
                children: children_info,
                flex: _flex,
            } => {
                if let DisplayView::FlexCol(f) = view {
                    self.update_sub_layout(&mut f.children, &children_info)
                }
            }
        }
    }

    fn update_sub_layout(&self, views: &mut Arc<Vec<DisplayView>>, infos: &Vec<DisplayViewInfo>) {
        let views = Arc::make_mut(views);
        let mut infos = infos.iter();

        for view in views.iter_mut() {
            let info = match infos.next() {
                Some(i) => i,
                None => return,
            };
            self.update_view(view, info);
        }
    }

    pub fn update_display_state(&self, data: &mut DisplayState) {
        self.update_view(&mut data.layout, &self.module.manifest.layout);
    }

    pub fn toggle_state(&mut self, id: &String) -> Result<(), Error> {
        println!("toggle_state {}", id);
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

    #[test]
    fn load_fe_module() -> Result<(), Error> {
        // While we are bootstrapping everything we'll be using the FE module for
        // tests.  Eventually the unique cases should be extracted into `test_data/mod`
        let module = Module::open("mods/ff4fe/manifest.json")?;
        let mut engine = Engine::new(module, TestEventSink)?;

        assert_state(&engine, &"baron", ObjectiveState::Unlocked);
        assert_state(&engine, &"fabul", ObjectiveState::Unlocked);
        assert_state(&engine, &"d-castle", ObjectiveState::Locked);
        assert_state(&engine, &"bahamut-cave", ObjectiveState::Locked);

        // Dwarf Castle should still be locked if Magma Key is only Unlocked.
        let updates = [("magma-key".to_string(), ObjectiveState::Unlocked)]
            .iter()
            .cloned()
            .collect();
        engine.update_state(&updates)?;
        assert_state(&engine, &"d-castle", ObjectiveState::Locked);

        // Completing Magma Key now unlocks Dwarf Castle.
        let updates = [("magma-key".to_string(), ObjectiveState::Complete)]
            .iter()
            .cloned()
            .collect();
        engine.update_state(&updates)?;
        assert_state(&engine, &"d-castle", ObjectiveState::Unlocked);

        // Un-completing the Magma Key should re-locks Dwarf Castle.
        let updates = [("magma-key".to_string(), ObjectiveState::Unlocked)]
            .iter()
            .cloned()
            .collect();
        engine.update_state(&updates)?;
        assert_state(&engine, &"d-castle", ObjectiveState::Locked);

        // Unlocking Darkness Crystal is enough to unlock Moon objectives.
        let updates = [("darkness-crystal".to_string(), ObjectiveState::Complete)]
            .iter()
            .cloned()
            .collect();
        engine.update_state(&updates)?;
        assert_state(&engine, &"bahamut-cave", ObjectiveState::Unlocked);
        Ok(())
    }
}
