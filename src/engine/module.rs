use std::collections::HashMap;
use std::convert::AsRef;
use std::fs;
use std::path::{Path, PathBuf};

use failure::{format_err, Error};
use path_slash::PathBufExt;
use serde::Deserialize;

use super::expression::Expression;
use super::{CornerRadius, Inset, ThemeColor};

#[derive(Debug, Deserialize, PartialEq)]
pub struct NodeInfoLoc {
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
    CheckBox { id: String, name: String },
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
    pub nodes: Vec<NodeInfoLoc>,
    #[serde(default)]
    pub maps: Vec<MapInfoLoc>,
    pub layouts: HashMap<String, DisplayViewInfo>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct NodeCheck {
    #[serde(default, rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "enabled-by")]
    pub enabled_by: Expression,
    #[serde(default, rename = "unlocked-by")]
    pub unlocked_by: Expression,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct NodeInfo {
    pub id: String,
    #[serde(default, rename = "type")]
    pub ty: String,
    pub name: String,
    #[serde(skip)]
    pub completed_by: Expression,
    #[serde(default, rename = "enabled-by")]
    pub enabled_by: Expression,
    #[serde(default, rename = "unlocked-by")]
    pub unlocked_by: Expression,
    #[serde(default)]
    pub checks: Vec<NodeCheck>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MapNode {
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
    #[serde(rename = "node-radius")]
    pub node_radius: f64,
    #[serde(default)]
    pub nodes: Vec<MapNode>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum NodeListSpecial {
    Checks,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
#[serde(rename_all = "kebab-case")]
pub enum NodeList {
    List(Vec<String>),
    Special(NodeListSpecial),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum DisplayViewInfoView {
    Grid {
        columns: usize,
        nodes: NodeList,
    },
    Count {
        node_type: String,
    },
    Map {
        maps: Vec<String>,
    },
    FlexRow {
        children: Vec<DisplayViewInfo>,
    },
    FlexCol {
        children: Vec<DisplayViewInfo>,
    },
    Spacer {},
    Tabs {
        labels: Vec<String>,
        children: Vec<DisplayViewInfo>,
    },
    Include {
        path: String,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct LayoutParamsInfo {
    #[serde(default)]
    pub flex: f64,

    #[serde(default)]
    pub background: ThemeColor,

    #[serde(default)]
    pub corner_radius: CornerRadius,

    #[serde(default)]
    pub inset: Inset,

    // window_height and window_width only apply to the root view of
    // a window.
    #[serde(default)]
    pub window_height: f64,

    #[serde(default)]
    pub window_width: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct DisplayViewInfo {
    #[serde(flatten)]
    pub layout_params: LayoutParamsInfo,

    #[serde(flatten)]
    pub view: DisplayViewInfoView,
}

#[derive(Debug)]
pub struct AssetInfo {
    pub path: PathBuf,
    pub id: String,
}

pub struct Module {
    pub manifest: Manifest,
    pub nodes: HashMap<String, NodeInfo>,
    pub maps: HashMap<String, MapInfo>,
    pub auto_track: Option<String>,
    pub assets: Vec<AssetInfo>,
}

impl Module {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Module, Error> {
        let path = path.as_ref().canonicalize()?;

        let manifest_str = std::fs::read_to_string(&path)
            .map_err(|e| format_err!("Failed to open {}: {}", path.display(), e))?;
        let mut manifest: Manifest = serde_json::from_str(&manifest_str)
            .map_err(|e| format_err!("Failed to parse {}: {}", path.display(), e))?;

        let base_path = path.parent().ok_or(format_err!(
            "Can't get parent directory of {}",
            path.display()
        ))?;

        if !manifest.layouts.contains_key(&"main".to_string()) {
            return Err(format_err!("manifest does not contain 'main' layout."));
        }
        if !manifest.layouts.contains_key(&"checks".to_string()) {
            return Err(format_err!("manifest does not contain 'checks' layout."));
        }

        for (_, layout) in manifest.layouts.iter_mut() {
            Self::process_display_includes(base_path, layout)?;
        }

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
            nodes: HashMap::new(),
            maps: HashMap::new(),
            auto_track,
            assets: Vec::new(),
        };

        module.import_nods(&base_path)?;

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

    fn process_display_includes(base_path: &Path, info: &mut DisplayViewInfo) -> Result<(), Error> {
        match &mut info.view {
            // Views with no children require no processing.
            DisplayViewInfoView::Grid {
                columns: _,
                nodes: _,
            }
            | DisplayViewInfoView::Count { node_type: _ }
            | DisplayViewInfoView::Map { maps: _ }
            | DisplayViewInfoView::Spacer {} => (),

            // Views will children need to recurse.
            DisplayViewInfoView::FlexRow { children }
            | DisplayViewInfoView::FlexCol { children }
            | DisplayViewInfoView::Tabs {
                labels: _,
                children,
            } => {
                for child in children.iter_mut() {
                    Self::process_display_includes(base_path, child)?;
                }
            }

            DisplayViewInfoView::Include { path } => {
                *info = Self::open_display_include(base_path, &path)?;
            }
        }

        Ok(())
    }

    fn open_display_include(base_path: &Path, path: &String) -> Result<DisplayViewInfo, Error> {
        let path = base_path.join(PathBuf::from_slash(path));
        let layout_str = std::fs::read_to_string(&path)
            .map_err(|e| format_err!("Failed to open {}: {}", path.display(), e))?;
        let mut info: DisplayViewInfo = serde_json::from_str(&layout_str)
            .map_err(|e| format_err!("Failed to parse {}: {}", path.display(), e))?;

        Self::process_display_includes(base_path, &mut info)?;
        Ok(info)
    }

    fn import_nods(&mut self, base_path: &Path) -> Result<(), Error> {
        for loc in &self.manifest.nodes {
            let path = base_path.join(PathBuf::from_slash(&loc.path));
            let obj_str = std::fs::read_to_string(&path)
                .map_err(|e| format_err!("Failed to open {}: {}", path.display(), e))?;
            let objs: Vec<NodeInfo> = serde_json::from_str(&obj_str)
                .map_err(|e| format_err!("Failed to parse {}: {}", path.display(), e))?;
            for o in objs {
                let mut obj = o.clone();
                self.check_for_unique_id(&obj.id, &path)?;
                obj.ty = loc.ty.clone();

                let mut checks_enabled_by = Expression::False;
                let mut checks_unlocked_by = Expression::False;
                let mut checks_completed_by = Expression::True;
                // Create nodes for each check.
                for (i, check) in obj.checks.iter_mut().enumerate() {
                    // If an ID is not givin. Assign one of the form `node_id:index`.
                    let id = if check.id == "" {
                        format!("{}:{}", &o.id, i)
                    } else {
                        check.id.clone()
                    };
                    self.check_for_unique_id(&id, &path)?;

                    check.id = id.clone();

                    // Expression defaults for checks should be True
                    let enabled_by = check.enabled_by.clone().eval_default(Expression::True);
                    let unlocked_by = check.unlocked_by.clone().eval_default(Expression::True);

                    // Add check conditions to parent node.
                    checks_enabled_by = checks_enabled_by.or(Expression::Node(id.clone()));
                    checks_unlocked_by =
                        checks_unlocked_by.or(Expression::NodeUnlocked(id.clone()));

                    // Node is complete if all non-disabled checks are complete.
                    checks_completed_by = checks_completed_by.and(Expression::Or(
                        Box::new(Expression::NodeComplete(id.clone())),
                        Box::new(Expression::NodeDisabled(id.clone())),
                    ));

                    self.nodes.insert(
                        id.clone(),
                        NodeInfo {
                            id,
                            ty: check.ty.clone(),
                            name: check.name.clone(),
                            unlocked_by: unlocked_by,
                            enabled_by: enabled_by,
                            completed_by: Expression::Manual,
                            checks: vec![],
                        },
                    );
                }

                if o.checks.len() == 0 {
                    // Nodes with no checks are enabled by default and
                    // unlocked manually.
                    obj.enabled_by = obj.enabled_by.eval_default(Expression::True);
                    obj.unlocked_by = obj.unlocked_by.eval_default(Expression::Manual);
                    obj.completed_by = obj.completed_by.eval_default(Expression::Manual);
                } else {
                    // Nodes with checks have their enabled_by/unlocked_by
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

                    // No support for explicit `completed_by` expressions.
                    obj.completed_by = checks_completed_by;
                }

                self.nodes.insert(obj.id.clone(), obj);
            }
        }
        Ok(())
    }

    fn check_for_unique_id(&self, id: &String, path: &Path) -> Result<(), Error> {
        if self.nodes.contains_key(id) {
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
    fn node_info_encoding() -> Result<(), Error> {
        // Test for type, children, and deps defaults.
        test_json_object(
            r#"{
    "id": "test",
    "name": "Test Node"
}"#,
            &NodeInfo {
                id: "test".to_string(),
                ty: "".to_string(),
                name: "Test Node".to_string(),
                enabled_by: Expression::default(),
                unlocked_by: Expression::default(),
                completed_by: Expression::default(),
                checks: vec![],
            },
        )
        .expect("decoding error");

        // Test for type, children, and deps specified.
        test_json_object(
            r#"{
    "id": "test",
    "type": "location",
    "name": "Test Node",
    "checks": [{"type": "key-item"}]
}"#,
            &NodeInfo {
                id: "test".to_string(),
                ty: "location".to_string(),
                name: "Test Node".to_string(),
                enabled_by: Expression::default(),
                unlocked_by: Expression::default(),
                completed_by: Expression::default(),
                checks: vec![NodeCheck {
                    ty: "key-item".to_string(),
                    id: "".to_string(),
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
    fn node_list_encoding() -> Result<(), Error> {
        test_json_object(
            r#"["a", "b"]"#,
            &NodeList::List(vec!["a".to_string(), "b".to_string()]),
        )
        .expect("decoding error");

        test_json_object(r#""checks""#, &NodeList::Special(NodeListSpecial::Checks))
            .expect("decoding error");

        Ok(())
    }

    #[test]
    fn load_module() -> Result<(), Error> {
        Module::open("src/engine/test_data/mod/manifest.json")?;
        Ok(())
    }
}
