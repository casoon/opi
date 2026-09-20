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
    /// Bun is listed twice: `bun.lock` is what it writes since 1.2, the binary
    /// `bun.lockb` what every project installed before that still carries.
    ///
    /// `package-lock.json` is last because other package managers are
    /// occasionally committed alongside it, while the reverse is rare.
    const LOCKFILES: [(&'static str, Self); 5] = [
        ("pnpm-lock.yaml", Self::Pnpm),
        ("bun.lock", Self::Bun),
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

    /// The argument list that runs `script`, forwarding `args` to it.
    ///
    /// Yarn takes the script name directly; the others need `run`.
    ///
    /// npm is the one package manager that needs `--` to tell its own flags
    /// from the script's, and it strips the separator before the script sees
    /// it. The others forward trailing arguments as they are, and would hand a
    /// literal `--` straight to the script.
    ///
    /// `workspace` names a member to run the script in, which every package
    /// manager spells differently and in a different position — yarn wants the
    /// package before the verb, npm a flag after the script.
    pub fn run_args(self, script: &str, workspace: Option<&str>, args: &[String]) -> Vec<String> {
        let mut argv = match (self, workspace) {
            (Self::Yarn, None) => vec![script.to_owned()],
            (Self::Yarn, Some(member)) => {
                vec![
                    "workspace".to_owned(),
                    member.to_owned(),
                    "run".to_owned(),
                    script.to_owned(),
                ]
            }
            (Self::Pnpm, Some(member)) => vec![
                "--filter".to_owned(),
                member.to_owned(),
                "run".to_owned(),
                script.to_owned(),
            ],
            (Self::Bun, Some(member)) => vec![
                "run".to_owned(),
                "--filter".to_owned(),
                member.to_owned(),
                script.to_owned(),
            ],
            (Self::Npm, Some(member)) => vec![
                "run".to_owned(),
                script.to_owned(),
                format!("--workspace={member}"),
            ],
            (_, None) => vec!["run".to_owned(), script.to_owned()],
        };

        if !args.is_empty() {
            if self == Self::Npm {
                argv.push("--".to_owned());
            }
            argv.extend_from_slice(args);
        }

        argv
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
        Self::lockfiles_in(dir).first().map(|(_, manager)| *manager)
    }

    /// Every lockfile present, in priority order.
    ///
    /// Detection only needs the first, but an action that *writes* needs to
    /// know there was a second: running `pnpm update` in a repository that
    /// actually uses npm leaves a `pnpm-lock.yaml` behind, and the next
    /// `npm ci` in CI then resolves against something the developer never saw.
    ///
    /// Bun is listed under two names and counts once — `bun.lockb` is what it
    /// wrote before 1.2 — or a Bun project mid-migration would look like a
    /// conflict with itself.
    pub fn lockfiles_in(dir: &Path) -> Vec<(&'static str, Self)> {
        let mut found: Vec<(&'static str, Self)> = Vec::new();
        for (file, manager) in Self::LOCKFILES {
            if dir.join(file).exists() && !found.iter().any(|(_, seen)| *seen == manager) {
                found.push((file, manager));
            }
        }
        found
    }

    /// Reads the `packageManager` field of the `package.json` in `dir`.
    fn from_manifest_at(dir: &Path) -> Option<Self> {
        let contents = std::fs::read_to_string(dir.join("package.json")).ok()?;
        let manifest: serde_json::Value = serde_json::from_str(&contents).ok()?;
        Self::from_corepack_field(manifest.get("packageManager")?.as_str()?)
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

/// Two lockfiles and nothing saying which one the project means.
///
/// A third state beside certain and defaulted: detected, but contradicted.
/// Measured here, 4 of 133 projects carry two lockfiles; the `packageManager`
/// field settles two of them and 89 of 133 projects have it. For the rest
/// there is no signal — both files are committed, both on the same day — and
/// reaching for a timestamp would be a heuristic that is wrong half the time
/// while looking confident.
///
/// Only actions that write ask this. The listing keeps taking the first match
/// silently: a list from the wrong manager is annoying and obvious, a lockfile
/// nobody asked for is neither.
#[derive(Debug, Clone)]
pub struct Ambiguous {
    pub lockfiles: Vec<&'static str>,
}

/// Which lockfiles contradict each other, or `None` where nothing does.
pub fn conflict(directory: &Path, detected: Detected) -> Option<Ambiguous> {
    // The field is a statement by the project; a lockfile is a side effect.
    // Where the project said it, there is nothing to resolve.
    if detected.source == Source::Manifest {
        return None;
    }
    let found = PackageManager::lockfiles_in(directory);
    (found.len() > 1).then(|| Ambiguous {
        lockfiles: found.into_iter().map(|(file, _)| file).collect(),
    })
}

/// What `opi` is looking at.
#[derive(Debug, Clone)]
pub struct Project {
    /// The `name` field. Absent in manifests that never get published.
    pub name: Option<String>,
    pub package_manager: Detected,
    /// Whether a `package.json` was found at all.
    ///
    /// `package_manager` cannot answer this: it always holds a value, because
    /// the absence of every signal still produces the npm fallback. A Rust-only
    /// project therefore looks like an npm project to it, and anything that
    /// would *run* the package manager has to ask here first.
    ///
    /// Not the same question as `Detected::is_certain`. An npm project with
    /// neither a lockfile nor a `packageManager` field is uncertain but real,
    /// and `npm audit` answers for it.
    pub npm: bool,
}

impl Project {
    /// Names the project after `name` where `package.json` does not.
    ///
    /// A Rust-only project has no npm name; its crate name beats the directory
    /// it happens to sit in.
    pub fn or_named(mut self, name: Option<String>) -> Self {
        self.name = self.name.or(name);
        self
    }

    /// Records whether a `package.json` was actually found.
    ///
    /// Only the caller that searched for it knows: `detect` is handed a
    /// manifest either way, and an empty one is indistinguishable from an empty
    /// real one.
    pub fn with_npm(mut self, npm: bool) -> Self {
        self.npm = npm;
        self
    }

    /// Detects the project in `dir` from its already-parsed manifest.
    pub fn detect(manifest: &Manifest, dir: &Path) -> Self {
        Self {
            name: manifest.name.clone().filter(|name| !name.trim().is_empty()),
            package_manager: detect_package_manager(dir, manifest.package_manager.as_deref()),
            npm: true,
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
///
/// Both are searched upwards from `dir`. In a workspace the lockfile and the
/// `packageManager` field live at the root, so a member package on its own
/// carries no evidence at all — and defaulting to npm there would run the
/// wrong package manager in a pnpm monorepo.
fn detect_package_manager(dir: &Path, field: Option<&str>) -> Detected {
    if let Some(manager) = field.and_then(PackageManager::from_corepack_field) {
        return Detected {
            manager,
            source: Source::Manifest,
        };
    }

    for ancestor in dir.ancestors() {
        if let Some(manager) = PackageManager::from_manifest_at(ancestor) {
            return Detected {
                manager,
                source: Source::Manifest,
            };
        }
        if let Some(manager) = PackageManager::from_lockfiles(ancestor) {
            return Detected {
                manager,
                source: Source::Lockfile,
            };
        }
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

    fn with_files(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        for file in files {
            std::fs::write(dir.path().join(file), "").expect("write");
        }
        dir
    }

    fn lockfile_detected(manager: PackageManager) -> Detected {
        Detected {
            manager,
            source: Source::Lockfile,
        }
    }

    #[test]
    fn two_lockfiles_and_no_field_is_a_conflict() {
        // Measured here: 4 of 133 projects carry two. Writing with the wrong
        // one leaves a lockfile the team did not ask for.
        let dir = with_files(&["pnpm-lock.yaml", "package-lock.json"]);
        let found =
            conflict(dir.path(), lockfile_detected(PackageManager::Pnpm)).expect("a conflict");
        assert_eq!(found.lockfiles, ["pnpm-lock.yaml", "package-lock.json"]);
    }

    #[test]
    fn the_package_manager_field_settles_it() {
        // The field is a statement by the project; a lockfile is a side
        // effect. Two of the four measured conflicts are answered this way.
        let dir = with_files(&["pnpm-lock.yaml", "package-lock.json"]);
        let detected = Detected {
            manager: PackageManager::Pnpm,
            source: Source::Manifest,
        };
        assert!(conflict(dir.path(), detected).is_none());
    }

    #[test]
    fn one_lockfile_is_no_conflict() {
        let dir = with_files(&["pnpm-lock.yaml"]);
        assert!(conflict(dir.path(), lockfile_detected(PackageManager::Pnpm)).is_none());
    }

    #[test]
    fn a_bun_project_mid_migration_does_not_conflict_with_itself() {
        // Both names are bun's: bun.lock since 1.2, bun.lockb before it.
        let dir = with_files(&["bun.lock", "bun.lockb"]);
        assert!(conflict(dir.path(), lockfile_detected(PackageManager::Bun)).is_none());
    }

    #[test]
    fn a_missing_package_json_is_not_an_uncertain_package_manager() {
        let dir = tempfile::tempdir().expect("temp dir");

        // No lockfile and no packageManager field: npm by fallback, but a real
        // npm project, and `npm audit` answers for it.
        let npm = Project::detect(&manifest("{}"), dir.path());
        assert!(npm.npm);
        assert!(!npm.package_manager.is_certain());

        // A project with no package.json at all reaches the same fallback.
        let rust = Project::detect(&Manifest::default(), dir.path()).with_npm(false);
        assert!(!rust.npm);
        assert_eq!(
            rust.package_manager, npm.package_manager,
            "the two are indistinguishable from the package manager alone, \
             which is the whole reason the flag exists"
        );
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
    fn evidence_at_the_workspace_root_counts_for_a_member() {
        // A member package carries neither a lockfile nor a packageManager
        // field; both live at the root. Defaulting to npm there would run the
        // wrong package manager in a pnpm monorepo.
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("pnpm-lock.yaml"), "").expect("write");
        let member = dir.path().join("apps").join("blog");
        std::fs::create_dir_all(&member).expect("mkdir");

        let project = Project::detect(&manifest("{}"), &member);
        assert_eq!(project.package_manager.manager, PackageManager::Pnpm);
        assert_eq!(project.package_manager.source, Source::Lockfile);
    }

    #[test]
    fn a_package_manager_field_at_the_root_counts_too() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"packageManager":"bun@1.2.0"}"#,
        )
        .expect("write");
        let member = dir.path().join("packages").join("ui");
        std::fs::create_dir_all(&member).expect("mkdir");

        let project = Project::detect(&manifest("{}"), &member);
        assert_eq!(project.package_manager.manager, PackageManager::Bun);
        assert_eq!(project.package_manager.source, Source::Manifest);
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
        assert_eq!(PackageManager::Yarn.run_args("dev", None, &[]), ["dev"]);
        assert_eq!(
            PackageManager::Pnpm.run_args("dev", None, &[]),
            ["run", "dev"]
        );
        assert_eq!(
            PackageManager::Npm.run_args("dev", None, &[]),
            ["run", "dev"]
        );
        assert_eq!(
            PackageManager::Bun.run_args("dev", None, &[]),
            ["run", "dev"]
        );
    }

    #[test]
    fn a_workspace_member_is_addressed_the_way_each_manager_expects() {
        let member = Some("blog");
        assert_eq!(
            PackageManager::Pnpm.run_args("dev", member, &[]),
            ["--filter", "blog", "run", "dev"]
        );
        assert_eq!(
            PackageManager::Yarn.run_args("dev", member, &[]),
            ["workspace", "blog", "run", "dev"],
            "yarn names the package before the verb"
        );
        assert_eq!(
            PackageManager::Npm.run_args("dev", member, &[]),
            ["run", "dev", "--workspace=blog"],
            "npm takes a flag after the script"
        );
        assert_eq!(
            PackageManager::Bun.run_args("dev", member, &[]),
            ["run", "--filter", "blog", "dev"]
        );
    }

    #[test]
    fn forwarded_args_still_land_last_with_a_workspace() {
        let args = ["--verbose".to_owned()];
        assert_eq!(
            PackageManager::Pnpm.run_args("build", Some("blog"), &args),
            ["--filter", "blog", "run", "build", "--verbose"]
        );
        assert_eq!(
            PackageManager::Npm.run_args("build", Some("blog"), &args),
            ["run", "build", "--workspace=blog", "--", "--verbose"]
        );
    }

    #[test]
    fn only_npm_needs_a_separator_for_forwarded_args() {
        let args = ["--verbose".to_owned()];
        assert_eq!(
            PackageManager::Npm.run_args("build", None, &args),
            ["run", "build", "--", "--verbose"]
        );
        assert_eq!(
            PackageManager::Pnpm.run_args("build", None, &args),
            ["run", "build", "--verbose"]
        );
        assert_eq!(
            PackageManager::Yarn.run_args("build", None, &args),
            ["build", "--verbose"]
        );
        assert_eq!(
            PackageManager::Bun.run_args("build", None, &args),
            ["run", "build", "--verbose"]
        );
    }

    #[test]
    fn no_separator_is_added_without_forwarded_args() {
        assert_eq!(
            PackageManager::Npm.run_args("build", None, &[]),
            ["run", "build"]
        );
    }

    #[test]
    fn forwarded_arguments_stay_single_values() {
        let args = ["--grep".to_owned(), "two words".to_owned()];
        assert_eq!(
            PackageManager::Pnpm.run_args("test", None, &args),
            ["run", "test", "--grep", "two words"]
        );
    }
}
