//! Reading `package.json`.
//!
//! The manifest is the only project interface `opi` has. Everything downstream —
//! project detection, the task list — is derived from what this module returns,
//! so parsing happens once and the result is shared.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};

/// The parts of `package.json` that `opi` uses.
///
/// Unknown fields are ignored, so any real-world manifest parses.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub name: Option<String>,
    pub version: Option<String>,
    pub package_manager: Option<String>,
    #[serde(default)]
    pub scripts: BTreeMap<String, String>,
    /// Descriptions, keyed by script name. Deliberately the `scripts-info`
    /// field `nr` already uses, so projects that maintain it benefit unchanged.
    #[serde(default, rename = "scripts-info")]
    pub scripts_info: BTreeMap<String, String>,
    /// Workspace member patterns. npm, yarn and bun declare these here; pnpm
    /// uses `pnpm-workspace.yaml` instead.
    #[serde(default, deserialize_with = "workspace_patterns")]
    pub workspaces: Vec<String>,
    /// The `opi` block, kept unparsed on purpose.
    ///
    /// Typed straight into a struct, a field of the wrong type would fail the
    /// whole `package.json` parse and make `opi` useless in a project whose
    /// `scripts` are perfectly fine. Reading it leniently keeps a typo here
    /// from costing everything else.
    #[serde(default)]
    opi: serde_json::Value,
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
    #[serde(default, rename = "devDependencies")]
    dev_dependencies: BTreeMap<String, String>,
}

/// Accepts both shapes the `workspaces` field takes: a bare list, or an object
/// with a `packages` list. yarn introduced the second for its `nohoist` option
/// and real manifests still use it.
fn workspace_patterns<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Field {
        List(Vec<String>),
        Object {
            #[serde(default)]
            packages: Vec<String>,
        },
    }

    Ok(match Field::deserialize(deserializer)? {
        Field::List(patterns) | Field::Object { packages: patterns } => patterns,
    })
}

impl Manifest {
    /// Finds the nearest `package.json`, searching `dir` and then its parents.
    ///
    /// Running `opi` from somewhere inside a project should work the way `npm`
    /// does, rather than only from the directory holding the manifest.
    ///
    /// Returns the directory it was found in, which is the project root for
    /// everything downstream — workspace patterns resolve against it, and it
    /// names the project when the manifest does not.
    pub fn discover(dir: &Path) -> Result<(Self, PathBuf), ManifestError> {
        for candidate in dir.ancestors() {
            match Self::load(candidate) {
                Ok(manifest) => return Ok((manifest, candidate.to_path_buf())),
                Err(ManifestError::Missing { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        Err(ManifestError::Missing {
            directory: dir.to_path_buf(),
        })
    }

    /// Loads and parses `package.json` from `dir`.
    pub fn load(dir: &Path) -> Result<Self, ManifestError> {
        let path = dir.join("package.json");
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(ManifestError::Missing {
                    directory: dir.to_path_buf(),
                });
            }
            Err(error) => return Err(ManifestError::Unreadable { path, error }),
        };

        serde_json::from_str(&contents).map_err(|error| ManifestError::Malformed { path, error })
    }

    /// The strings under `opi.<key>`, ignoring anything of another shape.
    pub fn opi_list(&self, key: &str) -> Vec<String> {
        self.opi
            .get(key)
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Whether the project depends on `package`, in either dependency set.
    ///
    /// Which one it is in does not matter here: a tool is available to run
    /// either way.
    pub fn depends_on(&self, package: &str) -> bool {
        self.dependencies.contains_key(package) || self.dev_dependencies.contains_key(package)
    }

    /// The description for `script`, if the project provides one.
    ///
    /// A blank entry counts as absent — an empty description column is better
    /// than a column of whitespace.
    pub fn description(&self, script: &str) -> Option<&str> {
        self.scripts_info
            .get(script)
            .map(|text| text.trim())
            .filter(|text| !text.is_empty())
    }
}

/// Why `package.json` could not be turned into a [`Manifest`].
///
/// The three cases stay distinct because they need different remedies.
#[derive(Debug)]
pub enum ManifestError {
    Missing {
        directory: PathBuf,
    },
    Unreadable {
        path: PathBuf,
        error: std::io::Error,
    },
    Malformed {
        path: PathBuf,
        error: serde_json::Error,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { directory } => {
                write!(f, "No package.json in {}", directory.display())
            }
            Self::Unreadable { path, error } => {
                write!(f, "Cannot read {}: {error}", path.display())
            }
            Self::Malformed { path, error } => {
                write!(f, "Cannot parse {}: {error}", path.display())
            }
        }
    }
}

impl std::error::Error for ManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Missing { .. } => None,
            Self::Unreadable { error, .. } => Some(error),
            Self::Malformed { error, .. } => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn load(contents: &str) -> Result<Manifest, ManifestError> {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("package.json"), contents).expect("write");
        Manifest::load(dir.path())
    }

    #[test]
    fn reads_scripts_and_descriptions() {
        let manifest = load(r#"{"scripts":{"dev":"astro dev"},"scripts-info":{"dev":"Start"}}"#)
            .expect("parse");
        assert_eq!(
            manifest.scripts.get("dev").map(String::as_str),
            Some("astro dev")
        );
        assert_eq!(manifest.description("dev"), Some("Start"));
    }

    #[test]
    fn missing_sections_are_empty_not_an_error() {
        let manifest = load(r#"{"name":"x"}"#).expect("parse");
        assert!(manifest.scripts.is_empty());
        assert!(manifest.scripts_info.is_empty());
    }

    #[test]
    fn blank_description_counts_as_absent() {
        let manifest =
            load(r#"{"scripts":{"dev":"x"},"scripts-info":{"dev":"   "}}"#).expect("parse");
        assert_eq!(manifest.description("dev"), None);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let manifest = load(r#"{"dependencies":{"astro":"^6"},"type":"module"}"#).expect("parse");
        assert!(manifest.name.is_none());
    }

    #[test]
    fn workspaces_accepts_a_bare_list() {
        let manifest = load(r#"{"workspaces":["packages/*","apps/*"]}"#).expect("parse");
        assert_eq!(manifest.workspaces, ["packages/*", "apps/*"]);
    }

    #[test]
    fn workspaces_accepts_the_object_form() {
        let manifest = load(r#"{"workspaces":{"packages":["packages/*"],"nohoist":["**/x"]}}"#)
            .expect("parse");
        assert_eq!(manifest.workspaces, ["packages/*"]);
    }

    #[test]
    fn no_workspaces_field_is_empty() {
        assert!(
            load(r#"{"name":"x"}"#)
                .expect("parse")
                .workspaces
                .is_empty()
        );
    }

    #[test]
    fn discover_walks_up_to_the_nearest_manifest() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("package.json"), r#"{"name":"root"}"#).expect("write");
        let deep = dir.path().join("src").join("components");
        fs::create_dir_all(&deep).expect("mkdir");

        let (manifest, root) = Manifest::discover(&deep).expect("discover");
        assert_eq!(manifest.name.as_deref(), Some("root"));
        assert_eq!(
            root.canonicalize().expect("canonicalize"),
            dir.path().canonicalize().expect("canonicalize")
        );
    }

    #[test]
    fn discover_stops_at_the_closest_manifest() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("package.json"), r#"{"name":"root"}"#).expect("write");
        let inner = dir.path().join("apps").join("blog");
        fs::create_dir_all(&inner).expect("mkdir");
        fs::write(inner.join("package.json"), r#"{"name":"blog"}"#).expect("write");

        let (manifest, _) = Manifest::discover(&inner).expect("discover");
        assert_eq!(manifest.name.as_deref(), Some("blog"));
    }

    #[test]
    fn the_opi_block_is_read_leniently() {
        let manifest =
            load(r#"{"scripts":{"dev":"x"},"opi":{"clean":["dist",".astro"]}}"#).expect("parse");
        assert_eq!(manifest.opi_list("clean"), ["dist", ".astro"]);
        assert!(manifest.opi_list("health").is_empty());
    }

    #[test]
    fn a_malformed_opi_block_does_not_cost_the_scripts() {
        // Typed into a struct, a wrong type here would fail the whole parse and
        // make opi useless in a project whose scripts are fine.
        for broken in [
            r#"{"scripts":{"dev":"x"},"opi":"not an object"}"#,
            r#"{"scripts":{"dev":"x"},"opi":{"clean":"not a list"}}"#,
            r#"{"scripts":{"dev":"x"},"opi":{"clean":[1,2,{"a":"b"}]}}"#,
            r#"{"scripts":{"dev":"x"},"opi":42}"#,
        ] {
            let manifest = load(broken).unwrap_or_else(|_| panic!("should parse: {broken}"));
            assert_eq!(manifest.scripts.len(), 1, "for {broken}");
            assert!(manifest.opi_list("clean").is_empty(), "for {broken}");
        }
    }

    #[test]
    fn dependencies_are_found_in_either_set() {
        let manifest =
            load(r#"{"dependencies":{"astro":"^7"},"devDependencies":{"@biomejs/biome":"^2"}}"#)
                .expect("parse");
        assert!(manifest.depends_on("astro"));
        assert!(manifest.depends_on("@biomejs/biome"));
        assert!(!manifest.depends_on("eslint"));
    }

    #[test]
    fn missing_file_is_its_own_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(matches!(
            Manifest::load(dir.path()),
            Err(ManifestError::Missing { .. })
        ));
    }

    #[test]
    fn malformed_json_is_reported() {
        assert!(matches!(
            load("{ not json"),
            Err(ManifestError::Malformed { .. })
        ));
    }
}
