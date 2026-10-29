//! Project checks.
//!
//! Detects project tools and reports their results. TypeScript errors come
//! from `tsc`, lint findings from Biome or ESLint. Secrets use a project scanner
//! or the bundled nosecrets engine; commit workflows also inspect Git's index.
//!
//! Which tools exist here was measured rather than assumed: across 133 real
//! projects Biome appeared in 51%, `tsc` in 41% and the secret scanner in 22%,
//! while Knip — prominent in the original plan — appeared in one. The command
//! each check runs is the one those projects already write in their scripts,
//! which is how `vitest run` got picked over a bare `vitest` that would sit in
//! watch mode forever.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use runemark::Verdict;

use crate::manifest::Manifest;
use crate::project::PackageManager;
use crate::workspace::Member;

/// One thing that can be checked, and the tool that checks it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// What is being checked, in the user's terms.
    pub name: &'static str,
    /// Which tool answers for it. Shown, so a result is never anonymous.
    pub tool: &'static str,
    /// Which package it belongs to, in a workspace. `None` is the root.
    pub scope: Option<String>,
    /// The external executable or a focused library/Git adapter.
    program: Program,
    args: Vec<&'static str>,
    /// Where the tool runs. A member's checks run in the member, which is the
    /// only place its config and its sources make sense.
    dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Program {
    External(PathBuf),
    Secrets { staged: bool },
    StagedFiles,
}

impl Check {
    /// The command to run to see the tool's full output.
    ///
    /// Health shows an excerpt; this is how to get the rest without having to
    /// work out which tool ran and with what.
    pub fn command_line(&self) -> String {
        let program = match &self.program {
            Program::External(program) => program,
            Program::Secrets { staged: true } | Program::StagedFiles => {
                return "opi --check commit".to_owned();
            }
            Program::Secrets { staged: false } => return "opi --security".to_owned(),
        };
        let program = program.file_name().map_or_else(
            || self.tool.to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        std::iter::once(program)
            .chain(self.args.iter().map(|arg| (*arg).to_owned()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// What to call this check on screen.
    pub fn label(&self) -> String {
        match &self.scope {
            Some(scope) => format!("{} ({scope})", self.name),
            None => self.name.to_owned(),
        }
    }
}

/// How a check turned out.
#[derive(Debug, Clone)]
pub struct Report {
    pub check: Check,
    pub verdict: Verdict,
    pub duration: Duration,
    /// Everything the tool wrote, kept whole.
    ///
    /// A parser that guesses at a tool's format is a parser that breaks on the
    /// next release of it; the raw output is what a developer needs anyway.
    pub output: String,
}

impl Report {
    /// Whether nothing failed. A warning passes: it is shown, never blocking.
    pub fn passed(&self) -> bool {
        matches!(self.verdict, Verdict::Passed | Verdict::Warning)
    }

    pub fn warned(&self) -> bool {
        self.verdict == Verdict::Warning
    }

    /// Whether the tool itself could not be started.
    pub fn unusable(&self) -> bool {
        self.verdict == Verdict::ActionRequired
    }
}

/// A check's tool, and the arguments that make it report without changing
/// anything.
struct Candidate {
    name: &'static str,
    tool: &'static str,
    /// Package that has to be a dependency for this tool to apply.
    package: &'static str,
    /// File the tool needs in the package directory, where it has nothing to
    /// check without one.
    requires: Option<&'static str>,
    args: &'static [&'static str],
}

/// Ordered within each concern, most specific first.
///
/// Only the first candidate of a concern runs. A project with both Biome and
/// Prettier gets one formatting answer, from the tool named in the result,
/// rather than two that can disagree.
const CANDIDATES: &[Candidate] = &[
    // `astro check` understands .astro files that `tsc` alone cannot.
    Candidate {
        name: "TypeScript",
        tool: "astro",
        package: "@astrojs/check",
        requires: None,
        args: &["check"],
    },
    Candidate {
        name: "TypeScript",
        tool: "tsc",
        package: "typescript",
        // Without a tsconfig.json, `tsc --noEmit` prints its help and fails.
        requires: Some("tsconfig.json"),
        args: &["--noEmit"],
    },
    // Biome checks lint and formatting in one pass.
    Candidate {
        name: "Lint & format",
        tool: "biome",
        package: "@biomejs/biome",
        requires: None,
        args: &["check", "."],
    },
    Candidate {
        name: "Lint",
        tool: "eslint",
        package: "eslint",
        requires: None,
        args: &["."],
    },
    Candidate {
        name: "Format",
        tool: "prettier",
        package: "prettier",
        requires: None,
        args: &["--check", "."],
    },
    // A bare `vitest` watches; health has to terminate.
    Candidate {
        name: "Tests",
        tool: "vitest",
        package: "vitest",
        requires: None,
        args: &["run"],
    },
    Candidate {
        name: "Tests",
        tool: "jest",
        package: "jest",
        requires: None,
        args: &[],
    },
    Candidate {
        name: "Dead code",
        tool: "fallow",
        package: "fallow",
        requires: None,
        args: &["dead-code"],
    },
    Candidate {
        name: "Dead code",
        tool: "knip",
        package: "knip",
        requires: None,
        args: &[],
    },
    Candidate {
        name: "Secrets",
        tool: "nosecrets",
        package: "@casoon/nosecrets",
        requires: None,
        args: &["scan", "."],
    },
    Candidate {
        name: "Secrets",
        tool: "nosecrets",
        package: "nosecrets",
        requires: None,
        args: &["scan", "."],
    },
    Candidate {
        name: "Secrets",
        tool: "secretlint",
        package: "secretlint",
        requires: None,
        args: &["**/*"],
    },
];

impl Check {
    pub fn known_names() -> std::collections::BTreeSet<&'static str> {
        CANDIDATES
            .iter()
            .map(|candidate| candidate.name)
            .chain(crate::cargo::CHECKS.iter().map(|(name, _)| *name))
            .chain(crate::cargo::FEATURE_CHECKS.iter().map(|(name, _)| *name))
            .chain(
                crate::cargo::OPTIONAL_CHECKS
                    .iter()
                    .map(|(name, _, _)| *name),
            )
            .chain(["Lockfile", "Staged files"])
            .collect()
    }

    /// Every check that applies, for the project and its workspace members.
    ///
    /// Members are checked in place. A monorepo keeps its TypeScript and its
    /// test runner in the packages rather than at the root — measured on a real
    /// workspace, the root declared only Biome, the secret scanner and a dead
    /// code tool, while `typescript` and `@astrojs/check` lived in both apps.
    /// Checking only the root would have reported two checks and missed four.
    pub fn detect_all(
        manifest: &Manifest,
        members: &[Member],
        rust_root: Option<&Path>,
        root: &Path,
        manager: PackageManager,
    ) -> Vec<Self> {
        let mut checks = Self::detect_in(manifest, root, None, manager);
        checks.extend(Self::lockfile(root, manager));
        for member in members {
            checks.extend(Self::detect_in(
                &member.manifest,
                &member.path,
                Some(member.name.clone()),
                manager,
            ));
        }
        if let Some(rust_root) = rust_root {
            checks.extend(Self::cargo(rust_root));
        }
        if !checks
            .iter()
            .any(|check| check.name == "Secrets" && check.scope.is_none())
        {
            checks.push(Self {
                name: "Secrets",
                tool: "nosecrets",
                scope: None,
                program: Program::Secrets { staged: false },
                args: Vec::new(),
                dir: root.to_path_buf(),
            });
        }
        checks
    }

    /// Commit-only checks read the index, including partially staged files.
    pub fn for_commit(mut checks: Vec<Self>, root: &Path) -> Vec<Self> {
        if crate::staged::repository(root).is_none() {
            return checks;
        }
        for check in &mut checks {
            if let Program::Secrets { staged } = &mut check.program {
                *staged = true;
            } else if check.name == "Secrets" && check.tool == "nosecrets" {
                check.args.retain(|arg| *arg != ".");
                check.args.push("--staged");
            }
        }
        checks.push(Self {
            name: "Staged files",
            tool: "git",
            scope: None,
            program: Program::StagedFiles,
            args: Vec::new(),
            dir: root.to_path_buf(),
        });
        checks
    }

    /// The checks a Rust project answers.
    ///
    /// Scoped as "rust" even in a Rust-only project: in the twelve
    /// repositories carrying both manifests, "Tests" would otherwise mean two
    /// different things on two lines.
    fn cargo(root: &Path) -> Vec<Self> {
        let build = |name: &'static str, args: Vec<&'static str>| Self {
            name,
            tool: "cargo",
            scope: Some("rust".to_owned()),
            program: Program::External(PathBuf::from("cargo")),
            args,
            dir: root.to_path_buf(),
        };

        // A library that commits no lockfile has none to be out of step with,
        // and `--locked` would fail for want of one rather than for drift.
        let lockfile = root
            .join("Cargo.lock")
            .exists()
            .then(|| build("Lockfile", crate::cargo::LOCKFILE_CHECK.to_vec()));

        // `cargo rustdoc` cannot start at a virtual manifest, so Docs there is
        // one run per member instead; see `cargo::virtual_members`.
        let members = crate::cargo::virtual_members(root);
        // A package with a library and a binary needs its target named; see
        // `cargo::library_packages`.
        let libraries = crate::cargo::library_packages(root);
        let root_library = crate::cargo::discover(root)
            .and_then(|(project, _)| project.name)
            .is_some_and(|name| libraries.contains(&name));
        let rustdoc = |args: &[&'static str], library: bool| {
            let mut args = args.to_vec();
            if library && args.first() == Some(&"rustdoc") {
                args.insert(1, "--lib");
            }
            args
        };
        let per_member = members.iter().map(|member| {
            let mut args = vec![
                "rustdoc",
                "-p",
                // Leaked: arguments are `&'static str`, and the few member
                // names live as long as the process anyway.
                Box::leak(member.clone().into_boxed_str()),
            ];
            if libraries.contains(member) {
                args.push("--lib");
            }
            args.extend(["--all-features", "--", "-D", "warnings"]);
            Self {
                name: "Docs",
                tool: "cargo",
                scope: Some(member.clone()),
                program: Program::External(PathBuf::from("cargo")),
                args,
                dir: root.to_path_buf(),
            }
        });

        crate::cargo::CHECKS
            .iter()
            .filter(|(name, _)| members.is_empty() || *name != "Docs")
            .map(|(name, args)| build(name, rustdoc(args, root_library)))
            // Feature-dependent: a second documentation run, offered where the
            // manifest has a feature to turn on and absent where both runs
            // would compile the same crate.
            .chain(
                crate::cargo::FEATURE_CHECKS
                    .iter()
                    .filter(|_| crate::cargo::has_optional_features(root))
                    .map(|(name, args)| build(name, rustdoc(args, root_library))),
            )
            .chain(per_member)
            .chain(lockfile)
            // A package no workspace flag reaches is checked where it lives.
            // Tests only: that is the gap CI found, and a second copy of every
            // Rust check per package would bury the project's own results.
            .chain(crate::cargo::siblings(root).into_iter().map(|sibling| {
                let (name, args) = crate::cargo::CHECKS
                    .iter()
                    .find(|(name, _)| *name == "Tests")
                    .expect("Tests is one of the Rust checks");
                Self {
                    name,
                    tool: "cargo",
                    scope: Some(sibling.name),
                    program: Program::External(PathBuf::from("cargo")),
                    args: args.to_vec(),
                    dir: sibling.path,
                }
            }))
            // The optional ones are separate binaries, so they are offered
            // where they are installed and absent otherwise — the shape
            // `cargo audit` and `cargo outdated` already have.
            .chain(
                crate::cargo::OPTIONAL_CHECKS
                    .iter()
                    .filter(|(_, subcommand, _)| crate::cargo::has_subcommand(subcommand))
                    .map(|(name, _, args)| build(name, args.to_vec())),
            )
            .collect()
    }

    /// Whether the lockfile still matches `package.json`, asked of the package
    /// manager that wrote it.
    ///
    /// The one project prerequisite worth a check. Measured across the
    /// repositories here, a declared `packageManager` version, `engines.node`
    /// and `rust-toolchain.toml` never disagreed with what ran — pnpm and
    /// rustup switch versions themselves, and every `engines` range was a
    /// lower bound the installed Node cleared — while 72 of 104 pnpm lockfiles
    /// were out of step with their manifest: a dependency added to
    /// `package.json` and never locked. Every CI that installs with a frozen
    /// lockfile stops there.
    ///
    /// Each command was measured to change nothing and to exit non-zero only
    /// on drift. pnpm's is `--offline` so the commit workflow never waits on
    /// the registry; it spots a mismatched specifier before resolving
    /// anything. yarn has no answer here: `yarn install --immutable` is a full
    /// install, and a check that installs is not a check.
    fn lockfile(root: &Path, manager: PackageManager) -> Option<Self> {
        let args: &[&'static str] = match manager {
            PackageManager::Pnpm => &[
                "install",
                "--frozen-lockfile",
                "--lockfile-only",
                "--ignore-scripts",
                "--offline",
            ],
            PackageManager::Npm => &["ci", "--dry-run", "--ignore-scripts"],
            PackageManager::Bun => &["install", "--frozen-lockfile", "--dry-run"],
            PackageManager::Yarn => return None,
        };
        // Only the manager's own lockfile counts: without one there is
        // nothing to be out of step, and `npm ci` would fail for its absence.
        PackageManager::lockfiles_in(root)
            .iter()
            .any(|(_, found)| *found == manager)
            .then(|| Self {
                name: "Lockfile",
                tool: manager.program(),
                scope: None,
                program: Program::External(PathBuf::from(manager.program())),
                args: args.to_vec(),
                dir: root.to_path_buf(),
            })
    }

    /// What the push and release workflows build, once every other check is
    /// done.
    ///
    /// Orchestrated, not reimplemented: the project's own `build` script, run
    /// by its package manager. A workspace root with a `build` script is
    /// trusted to build its members; without one, each member that has a
    /// `build` script is built in place. Rust builds with `--locked`, so a
    /// build that only works by rewriting `Cargo.lock` fails here, as it
    /// would in CI.
    pub fn builds(
        manifest: &Manifest,
        members: &[Member],
        rust_root: Option<&Path>,
        root: &Path,
        manager: PackageManager,
    ) -> Vec<Self> {
        let script = |dir: &Path, scope: Option<String>| Self {
            name: "Build",
            tool: manager.program(),
            scope,
            program: Program::External(PathBuf::from(manager.program())),
            args: vec!["run", "build"],
            dir: dir.to_path_buf(),
        };

        let mut builds = Vec::new();
        if manifest.scripts.contains_key("build") {
            builds.push(script(root, None));
        } else {
            builds.extend(
                members
                    .iter()
                    .filter(|member| member.manifest.scripts.contains_key("build"))
                    .map(|member| script(&member.path, Some(member.name.clone()))),
            );
        }

        if let Some(rust_root) = rust_root {
            // As with the lockfile check: without a Cargo.lock, `--locked`
            // would fail for want of one rather than for drift.
            let mut args = vec!["build", "--workspace"];
            if rust_root.join("Cargo.lock").exists() {
                args.push("--locked");
            }
            builds.push(Self {
                name: "Build",
                tool: "cargo",
                scope: Some("rust".to_owned()),
                program: Program::External(PathBuf::from("cargo")),
                args,
                dir: rust_root.to_path_buf(),
            });
        }
        builds
    }

    /// The checks that apply to one package, rooted at `dir`.
    ///
    /// A tool the package does not depend on produces no check at all — an
    /// absent check is honest, a failing one would not be. Whether the tool
    /// is actually installed is, for Yarn, not something a filesystem probe
    /// can answer (see [`resolve`]); there, a missing tool surfaces through
    /// `Check::run` as `ActionRequired` instead of being left out here.
    fn detect_in(
        manifest: &Manifest,
        dir: &Path,
        scope: Option<String>,
        manager: PackageManager,
    ) -> Vec<Self> {
        let mut checks: Vec<Self> = Vec::new();

        for candidate in CANDIDATES {
            if checks.iter().any(|check| check.name == candidate.name) {
                continue;
            }
            if !manifest.depends_on(candidate.package) {
                continue;
            }
            if candidate
                .requires
                .is_some_and(|file| !dir.join(file).exists())
            {
                continue;
            }
            let Some((program, prefix)) = resolve(dir, candidate.tool, manager) else {
                continue;
            };
            checks.push(Self {
                name: candidate.name,
                tool: candidate.tool,
                scope: scope.clone(),
                program: Program::External(program),
                args: prefix
                    .into_iter()
                    .chain(candidate.args.iter().copied())
                    .collect(),
                dir: dir.to_path_buf(),
            });
        }

        checks
    }

    /// Runs the tool and reports what it said.
    pub fn run(self) -> Report {
        let started = Instant::now();
        let (verdict, output) = match &self.program {
            Program::Secrets { staged } => crate::secrets::scan(&self.dir, *staged),
            Program::StagedFiles => crate::staged::check(&self.dir),
            Program::External(program) => {
                Self::run_external(program, &self.args, &self.dir, self.tool)
            }
        };
        Report {
            check: self,
            verdict,
            duration: started.elapsed(),
            output: output.trim_end().to_owned(),
        }
    }

    fn run_external(program: &Path, args: &[&str], dir: &Path, tool: &str) -> (Verdict, String) {
        let result = Command::new(program).args(args).current_dir(dir).output();
        match result {
            Ok(output) => {
                let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
                text.push_str(&String::from_utf8_lossy(&output.stderr));
                // A non-zero exit here means the tool has findings, which is a
                // successful run with a result — not a broken tool.
                let verdict = if output.status.success() {
                    Verdict::Passed
                } else {
                    Verdict::Failed
                };
                (verdict, text)
            }
            // A tool that will not start is a broken installation, not a check
            // with findings; the remedy is different and so is the mark.
            Err(error) => (
                Verdict::ActionRequired,
                format!("could not run {tool}: {error}"),
            ),
        }
    }
}

/// Finds `tool` in a `node_modules/.bin` at or above `root`.
///
/// Workspaces hoist their binaries to the root, so a member package has none
/// of its own.
fn binary(root: &Path, tool: &str) -> Option<PathBuf> {
    root.ancestors()
        .map(|dir| dir.join("node_modules").join(".bin").join(tool))
        .find(|path| path.exists())
}

/// How to start `tool`: a binary path, or — for Yarn — `yarn run <tool>`.
///
/// Yarn's default linker (Plug'n'Play, Yarn ≥ 2) never creates
/// `node_modules`, so [`binary`] alone finds nothing installed under an
/// ordinary Yarn 4 project even though the tool is there. `yarn run <tool>`
/// resolves a binary exposed by a dependency the same way a declared script
/// would, PnP or not, and works for Yarn Classic too — so the filesystem
/// probe is tried first and only Yarn falls back to it. If the tool is
/// genuinely not installed, that surfaces once the check runs, as the same
/// `ActionRequired` a missing binary already produces elsewhere.
fn resolve(
    dir: &Path,
    tool: &'static str,
    manager: PackageManager,
) -> Option<(PathBuf, Vec<&'static str>)> {
    if let Some(program) = binary(dir, tool) {
        return Some((program, Vec::new()));
    }
    (manager == PackageManager::Yarn).then(|| (PathBuf::from("yarn"), vec!["run", tool]))
}

/// Runs `checks` concurrently, handing each report back as it finishes.
///
/// Concurrency is bounded: six Node processes at once can be slower than three
/// on a laptop, and a health check that pins the machine is worse than one that
/// takes a moment longer.
pub fn run_all(checks: Vec<Check>, mut on_report: impl FnMut(&Report)) -> Vec<Report> {
    use std::sync::mpsc;

    let limit = std::thread::available_parallelism()
        .map_or(2, std::num::NonZero::get)
        .clamp(1, 4);

    let mut reports = Vec::with_capacity(checks.len());
    let mut queue = checks.into_iter();
    let (sender, receiver) = mpsc::channel();
    let mut running = 0usize;

    std::thread::scope(|scope| {
        loop {
            while running < limit {
                let Some(check) = queue.next() else { break };
                let sender = sender.clone();
                running += 1;
                scope.spawn(move || {
                    let _ = sender.send(check.run());
                });
            }

            if running == 0 {
                break;
            }

            let Ok(report) = receiver.recv() else { break };
            running -= 1;
            on_report(&report);
            reports.push(report);
        }
    });

    // Slowest last would order by accident; by label it is the same every run.
    reports.sort_by_key(|report| report.check.label());
    reports
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn projects_without_a_scanner_get_the_bundled_engine() {
        let dir = tempfile::tempdir().expect("temp dir");
        let checks = Check::detect_all(
            &Manifest::default(),
            &[],
            None,
            dir.path(),
            PackageManager::Npm,
        );
        let secrets = checks
            .iter()
            .find(|check| check.name == "Secrets")
            .expect("default scanner");
        assert_eq!(secrets.program, Program::Secrets { staged: false });
        assert_eq!(secrets.command_line(), "opi --security");
    }

    #[test]
    fn every_detected_check_can_be_saved_in_a_selection() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("package-lock.json"), "{}").unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"app\"\n").unwrap();
        fs::write(dir.path().join("Cargo.lock"), "").unwrap();
        let checks = Check::detect_all(
            &Manifest::default(),
            &[],
            Some(dir.path()),
            dir.path(),
            PackageManager::Npm,
        );
        let known = Check::known_names();
        for check in Check::for_commit(checks, dir.path()) {
            assert!(
                known.contains(check.name),
                "{} is not selectable",
                check.name
            );
        }
    }

    #[test]
    fn a_project_scanner_takes_precedence_over_the_bundled_engine() {
        let dir = tempfile::tempdir().expect("temp dir");
        let manifest: Manifest =
            serde_json::from_str(r#"{"devDependencies":{"secretlint":"10"}}"#).unwrap();
        let checks = Check::detect_all(&manifest, &[], None, dir.path(), PackageManager::Yarn);
        let scanners: Vec<_> = checks
            .iter()
            .filter(|check| check.name == "Secrets")
            .collect();
        assert_eq!(scanners.len(), 1);
        assert_eq!(scanners[0].tool, "secretlint");
    }

    #[test]
    fn resolve_prefers_a_filesystem_binary_over_yarn() {
        let dir = tempfile::tempdir().expect("temp dir");
        let bin = dir.path().join("node_modules").join(".bin");
        fs::create_dir_all(&bin).expect("mkdir");
        fs::write(bin.join("eslint"), "").expect("write");

        let (program, prefix) =
            resolve(dir.path(), "eslint", PackageManager::Yarn).expect("found on disk");
        assert_eq!(program, bin.join("eslint"));
        assert!(prefix.is_empty(), "a real binary needs no yarn prefix");
    }

    #[test]
    fn resolve_falls_back_to_yarn_run_when_nothing_is_on_disk() {
        // Yarn's PnP linker never writes node_modules, so this is the normal
        // case for a Yarn >= 2 project, not an edge case.
        let dir = tempfile::tempdir().expect("temp dir");
        let (program, prefix) =
            resolve(dir.path(), "eslint", PackageManager::Yarn).expect("resolved through yarn");
        assert_eq!(program, PathBuf::from("yarn"));
        assert_eq!(prefix, ["run", "eslint"]);
    }

    #[test]
    fn resolve_does_not_guess_for_other_managers() {
        let dir = tempfile::tempdir().expect("temp dir");
        for manager in [
            PackageManager::Npm,
            PackageManager::Pnpm,
            PackageManager::Bun,
        ] {
            assert!(
                resolve(dir.path(), "eslint", manager).is_none(),
                "{manager:?} has no node_modules here and no yarn fallback to try"
            );
        }
    }

    #[test]
    fn a_yarn_pnp_project_still_gets_its_checks() {
        // The bug this pins: a Yarn 4 project using Plug'n'Play has no
        // node_modules/.bin, so the old binary-only lookup found nothing and
        // Health reported zero checks even though `yarn eslint` runs fine.
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(
            dir.path().join("package.json"),
            r#"{"name":"pnp","devDependencies":{"eslint":"9.0.0"}}"#,
        )
        .expect("write");
        let manifest = Manifest::load(dir.path()).expect("manifest");

        let checks = Check::detect_in(&manifest, dir.path(), None, PackageManager::Yarn);
        let lint = checks
            .iter()
            .find(|check| check.name == "Lint")
            .expect("eslint check is offered under Yarn PnP");
        assert_eq!(lint.command_line(), "yarn run eslint .");
    }

    #[test]
    fn tsc_needs_a_tsconfig_to_check_anything() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(
            dir.path().join("package.json"),
            r#"{"name":"t","devDependencies":{"typescript":"6.0.3"}}"#,
        )
        .expect("write");
        fs::create_dir_all(dir.path().join("node_modules/.bin")).expect("mkdir");
        fs::write(dir.path().join("node_modules/.bin/tsc"), "").expect("write");
        let manifest = Manifest::load(dir.path()).expect("manifest");
        let typescript = |dir: &Path| {
            Check::detect_in(&manifest, dir, None, PackageManager::Npm)
                .into_iter()
                .any(|check| check.name == "TypeScript")
        };

        assert!(
            !typescript(dir.path()),
            "tsc without a tsconfig prints its help"
        );
        fs::write(dir.path().join("tsconfig.json"), "{}").expect("write");
        assert!(typescript(dir.path()));
    }

    #[test]
    fn a_lockfile_is_checked_by_the_manager_that_wrote_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("pnpm-lock.yaml"), "").expect("write");

        let check = Check::lockfile(dir.path(), PackageManager::Pnpm).expect("checked");
        assert_eq!(
            check.command_line(),
            "pnpm install --frozen-lockfile --lockfile-only --ignore-scripts --offline"
        );
    }

    #[test]
    fn no_lockfile_means_no_lockfile_check() {
        // `npm ci` without a package-lock.json fails for its absence, which
        // is not drift.
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(Check::lockfile(dir.path(), PackageManager::Npm).is_none());
    }

    #[test]
    fn another_managers_lockfile_is_not_this_ones() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("package-lock.json"), "{}").expect("write");
        assert!(Check::lockfile(dir.path(), PackageManager::Pnpm).is_none());
    }

    #[test]
    fn yarn_gets_no_lockfile_check() {
        // Its only strict form is a full install, and a check that installs
        // is not a check.
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("yarn.lock"), "").expect("write");
        assert!(Check::lockfile(dir.path(), PackageManager::Yarn).is_none());
    }

    #[test]
    fn a_cargo_lockfile_is_checked_only_where_one_is_committed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let named = |checks: Vec<Check>| checks.iter().any(|check| check.name == "Lockfile");

        assert!(
            !named(Check::cargo(dir.path())),
            "a library without Cargo.lock"
        );

        fs::write(dir.path().join("Cargo.lock"), "").expect("write");
        let checks = Check::cargo(dir.path());
        let lockfile = checks
            .iter()
            .find(|check| check.name == "Lockfile")
            .expect("checked");
        assert_eq!(lockfile.label(), "Lockfile (rust)");
        assert_eq!(
            lockfile.command_line(),
            "cargo metadata --locked --format-version 1"
        );
    }

    #[test]
    fn the_same_pnp_project_offers_nothing_under_npm() {
        // Without a real node_modules, npm has no fallback to reach for —
        // the Yarn path must not leak into the other managers.
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(
            dir.path().join("package.json"),
            r#"{"name":"solo","devDependencies":{"eslint":"9.0.0"}}"#,
        )
        .expect("write");
        let manifest = Manifest::load(dir.path()).expect("manifest");

        let checks = Check::detect_in(&manifest, dir.path(), None, PackageManager::Npm);
        assert!(
            checks.is_empty(),
            "no installed eslint, no yarn to fall back to"
        );
    }
}
