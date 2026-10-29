//! Optional workflow selections. Commands and tool detection stay in their existing modules.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const FILE: &str = ".opi/checks.json";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub workflows: BTreeMap<String, Selection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    /// Concern names apply across all detected tools and workspace members.
    pub checks: Vec<String>,
    pub build: bool,
}

impl Selection {
    pub fn includes(&self, name: &str) -> bool {
        self.checks.iter().any(|check| check == name)
    }
}

impl Config {
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join(FILE);
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        let config: Self = serde_json::from_str(&contents)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        config
            .validate()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        let names = crate::check::Check::known_names();
        for (workflow, selection) in &self.workflows {
            if !["commit", "push", "release", "deploy"].contains(&workflow.as_str()) {
                return Err(format!("unknown workflow {workflow:?}"));
            }
            for name in &selection.checks {
                if !names.contains(name.as_str()) {
                    return Err(format!("unknown check {name:?} in {workflow}"));
                }
            }
        }
        Ok(())
    }

    pub fn save(&self, root: &Path) -> Result<PathBuf, String> {
        self.validate()?;
        prepare_dir(root).map_err(|error| error.to_string())?;
        let path = root.join(FILE);
        let contents = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        fs::write(&path, format!("{contents}\n"))
            .map_err(|error| format!("{}: {error}", path.display()))?;
        Ok(path)
    }
}

/// Keep generated artefacts ignored, but allow the policy to be shared with Git.
pub fn prepare_dir(root: &Path) -> io::Result<()> {
    let dir = root.join(".opi");
    fs::create_dir_all(&dir)?;
    let path = dir.join(".gitignore");
    let mut contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => "*\n".to_owned(),
        Err(error) => return Err(error),
    };
    for exception in ["!.gitignore", "!checks.json"] {
        if !contents.lines().any(|line| line == exception) {
            if !contents.is_empty() && !contents.ends_with('\n') {
                contents.push('\n');
            }
            contents.push_str(exception);
            contents.push('\n');
        }
    }
    fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_config_keeps_the_defaults_without_creating_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Config::load(dir.path()).unwrap().workflows.is_empty());
        assert!(!dir.path().join(".opi").exists());
    }

    #[test]
    fn selections_round_trip_and_preserve_other_workflows() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.workflows.insert(
            "push".into(),
            Selection {
                checks: vec!["Secrets".into(), "Tests".into()],
                build: true,
            },
        );
        config.workflows.insert(
            "deploy".into(),
            Selection {
                checks: vec![],
                build: false,
            },
        );
        config.save(dir.path()).unwrap();
        let loaded = Config::load(dir.path()).unwrap();
        assert_eq!(loaded.workflows, config.workflows);
        assert!(loaded.workflows["push"].includes("Secrets"));
        assert!(!loaded.workflows["push"].includes("Format"));
        assert!(loaded.workflows["deploy"].checks.is_empty());
    }

    #[test]
    fn malformed_or_unknown_configuration_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".opi")).unwrap();
        for contents in [
            "{",
            r#"{"workflows":{"psuh":{"checks":[],"build":false}}}"#,
            r#"{"workflows":{"push":{"checks":["Typo"],"build":false}}}"#,
            r#"{"workflows":{"push":{"checks":[],"biuld":true}}}"#,
        ] {
            fs::write(dir.path().join(FILE), contents).unwrap();
            assert!(Config::load(dir.path()).is_err(), "{contents}");
        }
    }

    #[test]
    fn old_ignores_are_extended_without_losing_custom_rules() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".opi")).unwrap();
        let path = dir.path().join(".opi/.gitignore");
        fs::write(&path, "*\n# custom rule").unwrap();
        prepare_dir(dir.path()).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "*\n# custom rule\n!.gitignore\n!checks.json\n");
        prepare_dir(dir.path()).unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), contents);
    }

    #[test]
    fn git_can_share_configuration_while_ignoring_generated_sboms() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            std::process::Command::new("git")
                .current_dir(dir.path())
                .args(["init", "-q"])
                .output()
                .unwrap()
                .status
                .success()
        );
        Config::default().save(dir.path()).unwrap();
        fs::create_dir(dir.path().join(".opi/sbom")).unwrap();
        fs::write(dir.path().join(".opi/sbom/test.json"), "{}").unwrap();
        let output = std::process::Command::new("git")
            .current_dir(dir.path())
            .args(["ls-files", "--others", "--exclude-standard"])
            .output()
            .unwrap();
        let listed = String::from_utf8(output.stdout).unwrap();
        assert!(listed.contains(".opi/checks.json"));
        assert!(listed.contains(".opi/.gitignore"));
        assert!(!listed.contains("sbom"));
    }
}
