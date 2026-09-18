//! Reading `package.json`.
//!
//! The manifest is the only project interface `opi` has. Everything downstream —
//! project detection, the task list — is derived from what this module returns,
//! so parsing happens once and the result is shared.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The parts of `package.json` that `opi` uses.
///
/// Unknown fields are ignored, so any real-world manifest parses.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub name: Option<String>,
    pub package_manager: Option<String>,
    #[serde(default)]
    pub scripts: BTreeMap<String, String>,
    /// Descriptions, keyed by script name. Deliberately the `scripts-info`
    /// field `nr` already uses, so projects that maintain it benefit unchanged.
    #[serde(default, rename = "scripts-info")]
    pub scripts_info: BTreeMap<String, String>,
}

impl Manifest {
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
