//! Project checks.
//!
//! `opi` implements none of these. It detects which tool a project uses, runs
//! it, and reports what came back. TypeScript errors come from `tsc`, lint
//! findings from Biome or ESLint, secrets from the scanner the project already
//! depends on.
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
    /// The executable in `node_modules/.bin`, or `yarn` when resolved through
    /// it instead — see [`resolve`].
    program: PathBuf,
    args: Vec<&'static str>,
    /// Where the tool runs. A member's checks run in the member, which is the
    /// only place its config and its sources make sense.
    dir: PathBuf,
}

impl Check {
    /// The command to run to see the tool's full output.
    ///
    /// Health shows an excerpt; this is how to get the rest without having to
    /// work out which tool ran and with what.
    pub fn command_line(&self) -> String {
        let program = self.program.file_name().map_or_else(
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
    pub fn passed(&self) -> bool {
        self.verdict == Verdict::Passed
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
        args: &["check"],
    },
    Candidate {
        name: "TypeScript",
        tool: "tsc",
        package: "typescript",
        args: &["--noEmit"],
    },
    // Biome checks lint and formatting in one pass.
    Candidate {
        name: "Lint & format",
        tool: "biome",
        package: "@biomejs/biome",
        args: &["check", "."],
    },
    Candidate {
        name: "Lint",
        tool: "eslint",
        package: "eslint",
        args: &["."],
    },
    Candidate {
        name: "Format",
        tool: "prettier",
        package: "prettier",
        args: &["--check", "."],
    },
    // A bare `vitest` watches; health has to terminate.
    Candidate {
        name: "Tests",
        tool: "vitest",
        package: "vitest",
        args: &["run"],
    },
    Candidate {
        name: "Tests",
        tool: "jest",
        package: "jest",
        args: &[],
    },
    Candidate {
        name: "Dead code",
        tool: "fallow",
        package: "fallow",
        args: &["dead-code"],
    },
    Candidate {
        name: "Dead code",
        tool: "knip",
        package: "knip",
        args: &[],
    },
    Candidate {
        name: "Secrets",
        tool: "nosecrets",
        package: "@casoon/nosecrets",
        args: &["scan", "."],
    },
    Candidate {
        name: "Secrets",
        tool: "nosecrets",
        package: "nosecrets",
        args: &["scan", "."],
    },
    Candidate {
        name: "Secrets",
        tool: "secretlint",
        package: "secretlint",
        args: &["**/*"],
    },
];

impl Check {
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
            program: PathBuf::from("cargo"),
            args,
            dir: root.to_path_buf(),
        };

        crate::cargo::CHECKS
            .iter()
            .map(|(name, args)| build(name, args.to_vec()))
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
            let Some((program, prefix)) = resolve(dir, candidate.tool, manager) else {
                continue;
            };
            checks.push(Self {
                name: candidate.name,
                tool: candidate.tool,
                scope: scope.clone(),
                program,
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
        let result = Command::new(&self.program)
            .args(&self.args)
            .current_dir(&self.dir)
            .output();

        let (verdict, output) = match result {
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
                format!("could not run {}: {error}", self.tool),
            ),
        };

        Report {
            check: self,
            verdict,
            duration: started.elapsed(),
            output: output.trim_end().to_owned(),
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
