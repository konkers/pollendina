use std::collections::HashMap;
use std::convert::AsRef;
use std::fs;
use std::path::{Path, PathBuf};

use failure::{format_err, Error};
use path_slash::PathBufExt;
use serde::Deserialize;

use super::expression::Expression;

#[derive(Debug, Deserialize, PartialEq)]
pub struct ObjectiveInfoLoc {
    #[serde(rename = "type")]
    ty: String,
    path: String,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct MapInfoLoc {
    id: String,
    path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Param {
    TextBox { name: String },
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub authors: Vec<String>,
    #[serde(default, rename = "game-url")]
    pub game_url: String,
    #[serde(default, rename = "auto-track")]
    pub auto_track: Option<String>,
    #[serde(default)]
    pub params: Vec<Param>,
    pub objectives: Vec<ObjectiveInfoLoc>,
    #[serde(default)]
    pub maps: Vec<MapInfoLoc>,
    pub layout: DisplayViewInfo,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ObjectiveCheck {
    #[serde(default, rename = "type")]
    pub ty: String,
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "enabled-by")]
    pub enabled_by: Expression,
    #[serde(default, rename = "unlocked-by")]
    pub unlocked_by: Expression,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ObjectiveInfo {
    pub id: String,
    #[serde(default, rename = "type")]
    pub ty: String,
    pub name: String,
    #[serde(default, rename = "enabled-by")]
    pub enabled_by: Expression,
    #[serde(default, rename = "unlocked-by")]
    pub unlocked_by: Expression,
    #[serde(default)]
    pub checks: Vec<ObjectiveCheck>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MapObjective {
    pub id: String,
    pub x: u64,
    pub y: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MapInfo {
    pub id: String,
    pub name: String,
    pub width: u64,
    pub height: u64,
    #[serde(rename = "objective-radius")]
    pub objective_radius: f64,
    #[serde(default)]
    pub objectives: Vec<MapObjective>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum DisplayViewInfo {
    Grid {
        columns: usize,
        objectives: Vec<String>,
        #[serde(default)]
        flex: f64,
    },
    Count {
        objective_type: String,
        #[serde(default)]
        flex: f64,
    },
    Map {
        maps: Vec<String>,
        #[serde(default)]
        flex: f64,
    },
    FlexRow {
        children: Vec<DisplayViewInfo>,
        #[serde(default)]
        flex: f64,
    },
    FlexCol {
        children: Vec<DisplayViewInfo>,
        #[serde(default)]
        flex: f64,
    },
}

#[derive(Debug)]
pub struct AssetInfo {
    pub path: PathBuf,
    pub id: String,
}

pub struct Module {
    pub manifest: Manifest,
    pub objectives: HashMap<String, ObjectiveInfo>,
    pub maps: HashMap<String, MapInfo>,
    pub auto_track: Option<String>,
    pub assets: Vec<AssetInfo>,
}

impl Module {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Module, Error> {
        let path = path.as_ref().canonicalize()?;

        let manifest_str = std::fs::read_to_string(&path)
            .map_err(|e| format_err!("Failed to open {}: {}", path.display(), e))?;
        let manifest: Manifest = serde_json::from_str(&manifest_str)
            .map_err(|e| format_err!("Failed to parse {}: {}", path.display(), e))?;

        let base_path = path.parent().ok_or(format_err!(
            "Can't get parent directory of {}",
            path.display()
        ))?;

        let auto_track = match &manifest.auto_track {
            Some(path) => {
                let path = base_path.join(&path);
                let script_str = std::fs::read_to_string(&path)
                    .map_err(|e| format_err!("Failed to open {}: {}", path.display(), e))?;
                Some(script_str)
            }
            None => None,
        };

        let mut module = Module {
            manifest,
            objectives: HashMap::new(),
            maps: HashMap::new(),
            auto_track,
            assets: Vec::new(),
        };

        module.import_objectives(&base_path)?;

        for loc in &module.manifest.maps {
            let map_path = base_path.join(PathBuf::from_slash(&loc.path));
            let map_str = std::fs::read_to_string(&map_path)
                .map_err(|e| format_err!("Failed to open {}: {}", map_path.display(), e))?;
            let map: MapInfo = serde_json::from_str(&map_str)
                .map_err(|e| format_err!("Failed to parse {}: {}", map_path.display(), e))?;
            module.maps.insert(map.id.clone(), map);
        }

        // Traverse `assets` directory looking for PNGs.
        let assets_path = base_path.join("assets");
        Self::visit_asset_dir(&assets_path, &assets_path, &mut module.assets)?;

        // TODO(konkers): verify module integrity
        //  All id references should resolve (display and elsewhere)
        Ok(module)
    }

    fn import_objectives(&mut self, base_path: &Path) -> Result<(), Error> {
        for loc in &self.manifest.objectives {
            let path = base_path.join(PathBuf::from_slash(&loc.path));
            let obj_str = std::fs::read_to_string(&path)
                .map_err(|e| format_err!("Failed to open {}: {}", path.display(), e))?;
            let objs: Vec<ObjectiveInfo> = serde_json::from_str(&obj_str)
                .map_err(|e| format_err!("Failed to parse {}: {}", path.display(), e))?;
            for o in objs {
                let mut obj = o.clone();
                self.check_for_unique_id(&obj.id, &path)?;
                obj.ty = loc.ty.clone();

                let mut checks_enabled_by = Expression::False;
                let mut checks_unlocked_by = Expression::False;
                // Create objectives for each check.
                for (i, check) in o.checks.iter().enumerate() {
                    // If an ID is not givin. Assign one of the form `objective_id:index`.
                    let id = check.id.clone().unwrap_or(format!("{}:{}", &o.id, i));
                    self.check_for_unique_id(&id, &path)?;

                    // Expression defaults for checks should be True
                    let enabled_by = check.enabled_by.clone().eval_default(Expression::True);
                    let unlocked_by = check.unlocked_by.clone().eval_default(Expression::True);

                    // Add check conditions to parent objective.
                    checks_enabled_by = checks_enabled_by.or(Expression::Objective(id.clone()));
                    checks_unlocked_by = checks_unlocked_by.or(Expression::Objective(id.clone()));

                    self.objectives.insert(
                        id.clone(),
                        ObjectiveInfo {
                            id,
                            ty: "__CHECK__".into(),
                            name: check.name.clone(),
                            unlocked_by: unlocked_by,
                            enabled_by: enabled_by,
                            checks: vec![],
                        },
                    );
                }

                if o.checks.len() == 0 {
                    // Objectives with no checks are enabled by default and
                    // unlocked manually.
                    obj.enabled_by = obj.enabled_by.eval_default(Expression::True);
                    obj.unlocked_by = obj.unlocked_by.eval_default(Expression::Manual);
                } else {
                    // Objectives with checks have their enabled_by/unlocked_by
                    // ORed with their checks.  The default is False to short circuit
                    // with the checks expression.
                    obj.enabled_by = obj
                        .enabled_by
                        .eval_default(Expression::False)
                        .or(checks_enabled_by);
                    obj.unlocked_by = obj
                        .unlocked_by
                        .eval_default(Expression::False)
                        .or(checks_unlocked_by);
                }

                self.objectives.insert(obj.id.clone(), obj);
            }
        }
        Ok(())
    }

    fn check_for_unique_id(&self, id: &String, path: &Path) -> Result<(), Error> {
        if self.objectives.contains_key(id) {
            Err(format_err!(
                "Duplicate id {} found in {}.",
                id,
                path.display()
            ))
        } else {
            Ok(())
        }
    }

    fn visit_asset_dir(
        base_dir: &Path,
        dir: &Path,
        paths: &mut Vec<AssetInfo>,
    ) -> Result<(), Error> {
        if !dir.is_dir() {
            return Err(format_err!("{} is not a directory.", dir.to_string_lossy()));
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                Self::visit_asset_dir(base_dir, &path, paths)?;
            } else {
                if let Some(extension) = path.extension() {
                    if extension == "png" {
                        // Create `id` by stripping off the asset directory prefix,
                        // converting path separators to ':', and stripping the
                        // .png extension.  This creates a platform agnostic id
                        // based on the asset's path.
                        let mut id = path
                            .strip_prefix(&base_dir)?
                            .iter()
                            .map(|c| c.to_string_lossy().into_owned())
                            .collect::<Vec<String>>()
                            .join(":");
                        // Trims extension and the '.' preceding it.
                        id.truncate(id.len() - extension.len() - 1);

                        paths.push(AssetInfo {
                            path: path.to_path_buf(),
                            id,
                        });
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_json_object<'a, T>(s: &'a str, o: &T) -> Result<(), Error>
    where
        T: Deserialize<'a> + std::fmt::Debug + PartialEq,
    {
        let decoded: T = serde_json::from_str(s)?;
        assert_eq!(decoded, *o);

        Ok(())
    }

    #[test]
    fn objective_info_encoding() -> Result<(), Error> {
        // Test for type, children, and deps defaults.
        test_json_object(
            r#"{
    "id": "test",
    "name": "Test Objective"
}"#,
            &ObjectiveInfo {
                id: "test".to_string(),
                ty: "".to_string(),
                name: "Test Objective".to_string(),
                enabled_by: Expression::default(),
                unlocked_by: Expression::default(),
                checks: vec![],
            },
        )
        .expect("decoding error");

        // Test for type, children, and deps specified.
        test_json_object(
            r#"{
    "id": "test",
    "type": "location",
    "name": "Test Objective",
    "checks": [{"type": "key-item"}]
}"#,
            &ObjectiveInfo {
                id: "test".to_string(),
                ty: "location".to_string(),
                name: "Test Objective".to_string(),
                enabled_by: Expression::default(),
                unlocked_by: Expression::default(),
                checks: vec![ObjectiveCheck {
                    ty: "key-item".to_string(),
                    id: None,
                    name: "".to_string(),
                    enabled_by: Expression::default(),
                    unlocked_by: Expression::default(),
                }],
            },
        )
        .expect("decoding error");

        Ok(())
    }

    #[test]
    fn load_module() -> Result<(), Error> {
        Module::open("src/engine/test_data/mod/manifest.json")?;
        Ok(())
    }
}
