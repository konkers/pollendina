use std::collections::HashMap;
use std::convert::AsRef;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use failure::{format_err, Error};
use serde::{Deserialize, Serialize};

use super::expression::Expression;

#[derive(Debug, Deserialize, PartialEq)]
pub struct ObjectiveInfoLoc {
    #[serde(rename = "type")]
    ty: String,
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
    pub display: Vec<DisplayViewInfo>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ObjectiveCheck {
    #[serde(default, rename = "type")]
    pub ty: String,
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
    #[serde(default)]
    pub checks: Vec<ObjectiveCheck>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum DisplayViewInfo {
    Grid {
        columns: usize,
        objectives: Vec<String>,
    },
    Count {
        objective_type: String,
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

        let mut objectives = HashMap::new();

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

        for loc in &manifest.objectives {
            let obj_path = base_path.join(&loc.path);
            let obj_str = std::fs::read_to_string(&obj_path)
                .map_err(|e| format_err!("Failed to open {}: {}", obj_path.display(), e))?;
            let objs: Vec<ObjectiveInfo> = serde_json::from_str(&obj_str)
                .map_err(|e| format_err!("Failed to parse {}: {}", obj_path.display(), e))?;
            for o in objs {
                let mut obj = o.clone();
                obj.ty = loc.ty.clone();
                if objectives.contains_key(&obj.id) {
                    return Err(format_err!(
                        "Duplicate id {} found in {}.",
                        &obj.id,
                        obj_path.display()
                    ));
                }
                objectives.insert(obj.id.clone(), obj);
            }
        }
        // Traverse `assets` directory looking for PNGs.
        let assets_path = base_path.join("assets");
        let mut assets = Vec::new();
        Self::visit_asset_dir(&assets_path, &assets_path, &mut assets)?;

        println!("assets:");
        for a in &assets {
            println!("{:?}", &a);
        }
        // TODO(konkers): verify module integrity
        //  All id references should resolve (display and elsewhere)
        Ok(Module {
            manifest,
            objectives,
            auto_track,
            assets,
        })
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
                checks: vec![ObjectiveCheck {
                    ty: "key-item".to_string(),
                    name: "".to_string(),
                    enabled_by: Expression::True,
                    unlocked_by: Expression::True,
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
