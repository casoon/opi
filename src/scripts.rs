//! Dependencies that run code when they are installed.
//!
//! A `preinstall`, `install` or `postinstall` script runs on every machine that
//! installs the package, which is how most npm supply-chain attacks execute.
//! The package managers differ in who may run one: pnpm and bun run only what
//! the project allows, npm runs everything unless `ignore-scripts` is set.
//!
//! The packages are found by reading the installed `package.json` files, and
//! the allow list from the project's own configuration. Nothing is installed or
//! run.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::project::PackageManager;

const LIFECYCLE: [&str; 3] = ["preinstall", "install", "postinstall"];

/// What happens to a dependency's install script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The project allows it, or the package manager's own rules let it run.
    Allowed,
    /// The project turned it off, by name or with `ignore-scripts`.
    Ignored,
    /// Neither allowed nor ignored: pnpm skips it silently, npm runs it, bun
    /// skips it.
    Unreviewed,
}

/// The installed dependencies with an install script, and what the project
/// says about each.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct InstallScripts {
    /// `(name, status)`, by name.
    pub packages: Vec<(String, Status)>,
}

/// Lists the installed dependencies with install scripts, with their status.
///
/// `None` where the package manager has no such policy to report (Yarn) or
/// nothing is installed yet.
pub fn install_scripts(manager: PackageManager, root: &Path) -> Option<InstallScripts> {
    if manager == PackageManager::Yarn {
        return None;
    }
    let modules = root.join("node_modules");
    if !modules.is_dir() {
        return None;
    }
    let mut names = BTreeSet::new();
    scan(&modules, &mut names);

    let package_json = read_json(&root.join("package.json"));
    let policy = match manager {
        PackageManager::Pnpm => Policy::Lists {
            allowed: pnpm_list(root, &package_json, "onlyBuiltDependencies"),
            ignored: pnpm_list(root, &package_json, "ignoredBuiltDependencies"),
        },
        PackageManager::Bun => Policy::Lists {
            allowed: strings(package_json.get("trustedDependencies")),
            ignored: Vec::new(),
        },
        _ => Policy::IgnoreScripts(npm_ignore_scripts(root)),
    };
    Some(InstallScripts {
        packages: names
            .into_iter()
            .map(|name| {
                let status = policy.status(&name);
                (name, status)
            })
            .collect(),
    })
}

enum Policy {
    Lists {
        allowed: Vec<String>,
        ignored: Vec<String>,
    },
    IgnoreScripts(bool),
}

impl Policy {
    fn status(&self, name: &str) -> Status {
        match self {
            Self::Lists { allowed, ignored } => {
                if listed(allowed, name) {
                    Status::Allowed
                } else if listed(ignored, name) {
                    Status::Ignored
                } else {
                    Status::Unreviewed
                }
            }
            Self::IgnoreScripts(true) => Status::Ignored,
            Self::IgnoreScripts(false) => Status::Unreviewed,
        }
    }
}

/// Whether a list names the package, bare or pinned as `name@version`.
fn listed(list: &[String], name: &str) -> bool {
    list.iter()
        .any(|entry| entry == name || entry.rsplit_once('@').is_some_and(|(n, _)| n == name))
}

/// Collects the names of the packages below a `node_modules` that declare an
/// install script, descending into nested `node_modules` and the stores of
/// pnpm (`.pnpm`) and bun (`.bun`). Symlinks are skipped: they point at
/// something that is scanned where it lives.
fn scan(modules: &Path, names: &mut BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(modules) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let file = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type().map_or(true, |kind| !kind.is_dir()) {
            continue;
        }
        if file == ".pnpm" || file == ".bun" {
            if let Ok(store) = fs::read_dir(&path) {
                for package in store.flatten() {
                    scan(&package.path().join("node_modules"), names);
                }
            }
        } else if file.starts_with('@') {
            scan(&path, names);
        } else if !file.starts_with('.') {
            if let Some(name) = declares_script(&path) {
                names.insert(name);
            }
            scan(&path.join("node_modules"), names);
        }
    }
}

fn declares_script(package: &Path) -> Option<String> {
    let json = read_json(&package.join("package.json"));
    let scripts = json.get("scripts")?.as_object()?;
    LIFECYCLE
        .iter()
        .any(|hook| scripts.get(*hook).is_some_and(Value::is_string))
        .then(|| json.get("name")?.as_str().map(str::to_owned))
        .flatten()
}

fn read_json(path: &Path) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null)
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.as_str().map(str::to_owned))
        .collect()
}

/// A pnpm setting that is a list of package names, from `package.json`'s
/// `pnpm` field and from `pnpm-workspace.yaml`, where it lives since pnpm 10.5.
fn pnpm_list(root: &Path, package_json: &Value, key: &str) -> Vec<String> {
    let mut list = strings(package_json.get("pnpm").and_then(|pnpm| pnpm.get(key)));
    if let Ok(yaml) = fs::read_to_string(root.join("pnpm-workspace.yaml")) {
        list.extend(yaml_list(&yaml, key));
    }
    list
}

/// The items of a top-level YAML list, block (`- a`) or flow (`[a, b]`).
/// Enough for the one shape these settings take; not a YAML parser.
fn yaml_list(yaml: &str, key: &str) -> Vec<String> {
    let clean = |item: &str| {
        item.trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .to_owned()
    };
    let mut items = Vec::new();
    let mut inside = false;
    for line in yaml.lines() {
        let content = line.split(" #").next().unwrap_or(line);
        if content.trim().is_empty() || content.trim_start().starts_with('#') {
            continue;
        }
        if !line.starts_with(char::is_whitespace) && !line.starts_with('-') {
            inside = false;
            if let Some(rest) = line.strip_prefix(key).and_then(|r| r.strip_prefix(':')) {
                inside = true;
                let rest = content[content.len() - rest.len()..].trim();
                if let Some(flow) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                    items.extend(flow.split(',').map(clean).filter(|i| !i.is_empty()));
                }
            }
        } else if inside {
            if let Some(item) = content.trim_start().strip_prefix('-') {
                items.push(clean(item));
            }
        }
    }
    items
}

/// Whether npm's effective configuration has `ignore-scripts` on, asked of npm
/// because it can sit in a project, user or global `.npmrc`.
fn npm_ignore_scripts(root: &Path) -> bool {
    Command::new("npm")
        .args(["config", "get", "ignore-scripts"])
        .current_dir(root)
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "true")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(root: &Path, relative: &str, name: &str, scripts: &str) {
        let dir = root.join(relative);
        fs::create_dir_all(&dir).expect("dir");
        fs::write(
            dir.join("package.json"),
            format!(r#"{{"name": "{name}", "scripts": {scripts}}}"#),
        )
        .expect("package.json");
    }

    fn status(found: &InstallScripts, name: &str) -> Status {
        found
            .packages
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("{name} not found in {found:?}"))
            .1
    }

    #[test]
    fn pnpm_compares_the_store_with_the_workspace_lists() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        fs::write(
            root.join("pnpm-workspace.yaml"),
            "packages:\n  - app\nonlyBuiltDependencies:\n  - esbuild\n  - \"sharp@0.33.0\"\nignoredBuiltDependencies:\n  - core-js # old\n",
        )
        .expect("workspace");
        fs::write(root.join("package.json"), "{}").expect("package.json");
        let store = "node_modules/.pnpm";
        package(
            root,
            &format!("{store}/esbuild@0.21.0/node_modules/esbuild"),
            "esbuild",
            r#"{"postinstall": "node install.js"}"#,
        );
        package(
            root,
            &format!("{store}/sharp@0.33.0/node_modules/sharp"),
            "sharp",
            r#"{"install": "node check.js"}"#,
        );
        package(
            root,
            &format!("{store}/core-js@3.0.0/node_modules/core-js"),
            "core-js",
            r#"{"postinstall": "node x.js"}"#,
        );
        package(
            root,
            &format!("{store}/fsevents@2.0.0/node_modules/fsevents"),
            "fsevents",
            r#"{"install": "node-gyp rebuild"}"#,
        );
        package(
            root,
            &format!("{store}/plain@1.0.0/node_modules/plain"),
            "plain",
            r#"{"test": "x"}"#,
        );

        let found = install_scripts(PackageManager::Pnpm, root).expect("report");
        assert_eq!(found.packages.len(), 4);
        assert_eq!(status(&found, "esbuild"), Status::Allowed);
        assert_eq!(status(&found, "sharp"), Status::Allowed);
        assert_eq!(status(&found, "core-js"), Status::Ignored);
        assert_eq!(status(&found, "fsevents"), Status::Unreviewed);
    }

    #[test]
    fn pnpm_reads_the_lists_from_package_json_too() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        fs::write(
            root.join("package.json"),
            r#"{"pnpm": {"onlyBuiltDependencies": ["@scope/native"]}}"#,
        )
        .expect("package.json");
        package(
            root,
            "node_modules/.pnpm/n@1.0.0/node_modules/@scope/native",
            "@scope/native",
            r#"{"preinstall": "x"}"#,
        );

        let found = install_scripts(PackageManager::Pnpm, root).expect("report");
        assert_eq!(status(&found, "@scope/native"), Status::Allowed);
    }

    #[test]
    fn bun_allows_only_trusted_dependencies() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        fs::write(
            root.join("package.json"),
            r#"{"trustedDependencies": ["esbuild"]}"#,
        )
        .expect("package.json");
        package(
            root,
            "node_modules/esbuild",
            "esbuild",
            r#"{"postinstall": "x"}"#,
        );
        package(root, "node_modules/other", "other", r#"{"install": "x"}"#);

        let found = install_scripts(PackageManager::Bun, root).expect("report");
        assert_eq!(status(&found, "esbuild"), Status::Allowed);
        assert_eq!(status(&found, "other"), Status::Unreviewed);
    }

    #[test]
    fn npm_finds_nested_packages_and_scoped_ones() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        fs::write(root.join("package.json"), "{}").expect("package.json");
        package(root, "node_modules/outer", "outer", "{}");
        package(
            root,
            "node_modules/outer/node_modules/inner",
            "inner",
            r#"{"postinstall": "x"}"#,
        );
        package(root, "node_modules/@s/top", "@s/top", r#"{"install": "x"}"#);

        let found = install_scripts(PackageManager::Npm, root).expect("report");
        let names: Vec<&str> = found.packages.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["@s/top", "inner"]);
    }

    #[test]
    fn nothing_installed_is_no_report() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(install_scripts(PackageManager::Pnpm, dir.path()), None);
        assert_eq!(install_scripts(PackageManager::Yarn, dir.path()), None);
    }

    #[test]
    fn yaml_lists_read_block_and_flow_form() {
        let yaml = "a:\n  - x\nonlyBuiltDependencies: [esbuild, 'sharp']\nb:\n  - y\n";
        assert_eq!(
            yaml_list(yaml, "onlyBuiltDependencies"),
            ["esbuild", "sharp"]
        );
        assert_eq!(yaml_list(yaml, "a"), ["x"]);
        assert!(yaml_list(yaml, "missing").is_empty());
    }

    #[test]
    fn a_pinned_entry_names_its_package() {
        let list = vec!["sharp@0.33.0".to_owned(), "@s/p@1.0.0".to_owned()];
        assert!(listed(&list, "sharp"));
        assert!(listed(&list, "@s/p"));
        assert!(!listed(&list, "shar"));
    }
}
