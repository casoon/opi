//! Project detection.
//!
//! Establishes what `opi` is looking at before anything is rendered: the
//! project name, the package manager its scripts must be run with, and the Node
//! runtime available.
//!
//! Detection never guesses silently. Where the package manager cannot be
//! established from the project itself, that is a distinguishable state
//! ([`Source::Fallback`]) rather than an unmarked default — running scripts
//! through the wrong package manager starts the wrong process.

use std::fmt;
use std::path::Path;

use crate::manifest::Manifest;

/// The package manager a project's scripts are run with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl PackageManager {
    /// Lockfiles that identify a package manager, most specific first.
    ///
    /// `package-lock.json` is last because other package managers are
    /// occasionally committed alongside it, while the reverse is rare.
    const LOCKFILES: [(&'static str, Self); 4] = [
        ("pnpm-lock.yaml", Self::Pnpm),
        ("bun.lockb", Self::Bun),
        ("yarn.lock", Self::Yarn),
        ("package-lock.json", Self::Npm),
    ];

    /// The executable name.
    pub fn program(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Bun => "bun",
        }
    }

    /// The argument list that runs `script`.
    ///
    /// Yarn takes the script name directly; the others need `run`.
    // Covered by tests; the executor that calls it arrives with the script list.
    #[allow(dead_code)]
    pub fn run_args(self, script: &str) -> Vec<String> {
        match self {
            Self::Yarn => vec![script.to_owned()],
            _ => vec!["run".to_owned(), script.to_owned()],
        }
    }

    /// Parses the name out of a `packageManager` field such as
    /// `pnpm@10.4.1+sha512.abc`.
    fn from_corepack_field(value: &str) -> Option<Self> {
        let name = value.split('@').next()?.trim();
        match name {
            "npm" => Some(Self::Npm),
            "pnpm" => Some(Self::Pnpm),
            "yarn" => Some(Self::Yarn),
            "bun" => Some(Self::Bun),
            _ => None,
        }
    }

    fn from_lockfiles(dir: &Path) -> Option<Self> {
        Self::LOCKFILES
            .iter()
            .find(|(file, _)| dir.join(file).exists())
            .map(|(_, manager)| *manager)
    }
}

impl fmt::Display for PackageManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.program())
    }
}

/// Where a detected package manager came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The `packageManager` field in `package.json`.
    Manifest,
    /// A lockfile in the project directory.
    Lockfile,
    /// Neither was present. The value is a default, not a detection.
    Fallback,
}

/// A package manager together with the evidence for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Detected {
    pub manager: PackageManager,
    pub source: Source,
}

impl Detected {
    /// Whether this was established from the project, rather than defaulted.
    pub fn is_certain(self) -> bool {
        self.source != Source::Fallback
    }
}

/// What `opi` is looking at.
#[derive(Debug, Clone)]
pub struct Project {
    /// The `name` field. Absent in manifests that never get published.
    pub name: Option<String>,
    pub package_manager: Detected,
}

impl Project {
    /// Detects the project in `dir` from its already-parsed manifest.
    pub fn detect(manifest: &Manifest, dir: &Path) -> Self {
        Self {
            name: manifest.name.clone().filter(|name| !name.trim().is_empty()),
            package_manager: detect_package_manager(dir, manifest.package_manager.as_deref()),
        }
    }

    /// The name to show, falling back to the directory name.
    pub fn display_name(&self, dir: &Path) -> String {
        self.name
            .clone()
            .or_else(|| {
                dir.canonicalize()
                    .ok()?
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "project".to_owned())
    }
}

/// The `packageManager` field wins over lockfiles: it is an explicit statement
/// by the project, while a lockfile is a side effect that can be left behind
/// after switching package managers.
fn detect_package_manager(dir: &Path, field: Option<&str>) -> Detected {
    if let Some(manager) = field.and_then(PackageManager::from_corepack_field) {
        return Detected {
            manager,
            source: Source::Manifest,
        };
    }

    if let Some(manager) = PackageManager::from_lockfiles(dir) {
        return Detected {
            manager,
            source: Source::Lockfile,
        };
    }

    Detected {
        manager: PackageManager::Npm,
        source: Source::Fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(json: &str) -> Manifest {
        serde_json::from_str(json).expect("parse fixture")
    }

    fn detect(json: &str, lockfiles: &[&str]) -> (tempfile::TempDir, Project) {
        let dir = tempfile::tempdir().expect("temp dir");
        for lockfile in lockfiles {
            std::fs::write(dir.path().join(lockfile), "").expect("write lockfile");
        }
        let project = Project::detect(&manifest(json), dir.path());
        (dir, project)
    }

    #[test]
    fn reads_name_from_manifest() {
        let (_dir, project) = detect(r#"{"name":"casoon.dev"}"#, &[]);
        assert_eq!(project.name.as_deref(), Some("casoon.dev"));
    }

    #[test]
    fn lockfile_identifies_package_manager() {
        for (file, expected) in PackageManager::LOCKFILES {
            let (_dir, project) = detect("{}", &[file]);
            assert_eq!(project.package_manager.manager, expected, "for {file}");
            assert_eq!(project.package_manager.source, Source::Lockfile);
        }
    }

    #[test]
    fn manifest_field_beats_lockfile() {
        let (_dir, project) = detect(
            r#"{"packageManager":"pnpm@10.4.1"}"#,
            &["package-lock.json"],
        );
        assert_eq!(project.package_manager.manager, PackageManager::Pnpm);
        assert_eq!(project.package_manager.source, Source::Manifest);
    }

    #[test]
    fn corepack_field_tolerates_hash_suffix() {
        assert_eq!(
            PackageManager::from_corepack_field("yarn@4.1.0+sha512.deadbeef"),
            Some(PackageManager::Yarn)
        );
    }

    #[test]
    fn unknown_corepack_field_falls_through_to_lockfile() {
        let (_dir, project) = detect(r#"{"packageManager":"cnpm@1.0.0"}"#, &["pnpm-lock.yaml"]);
        assert_eq!(project.package_manager.manager, PackageManager::Pnpm);
        assert_eq!(project.package_manager.source, Source::Lockfile);
    }

    #[test]
    fn no_evidence_is_marked_as_a_fallback() {
        let (_dir, project) = detect("{}", &[]);
        assert_eq!(project.package_manager.manager, PackageManager::Npm);
        assert_eq!(project.package_manager.source, Source::Fallback);
        assert!(!project.package_manager.is_certain());
    }

    #[test]
    fn blank_name_is_treated_as_absent() {
        let (_dir, project) = detect(r#"{"name":"  "}"#, &[]);
        assert!(project.name.is_none());
    }

    #[test]
    fn display_name_falls_back_to_directory() {
        let (dir, project) = detect("{}", &[]);
        let expected = dir
            .path()
            .canonicalize()
            .expect("canonicalize")
            .file_name()
            .expect("file name")
            .to_string_lossy()
            .into_owned();
        assert_eq!(project.display_name(dir.path()), expected);
    }

    #[test]
    fn run_args_follow_each_package_manager() {
        assert_eq!(PackageManager::Yarn.run_args("dev"), ["dev"]);
        assert_eq!(PackageManager::Pnpm.run_args("dev"), ["run", "dev"]);
        assert_eq!(PackageManager::Npm.run_args("dev"), ["run", "dev"]);
        assert_eq!(PackageManager::Bun.run_args("dev"), ["run", "dev"]);
    }
}
