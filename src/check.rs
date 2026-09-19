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
    /// The executable in `node_modules/.bin`.
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
    pub fn detect_all(manifest: &Manifest, members: &[Member], root: &Path) -> Vec<Self> {
        let mut checks = Self::detect_in(manifest, root, None);
        for member in members {
            checks.extend(Self::detect_in(
                &member.manifest,
                &member.path,
                Some(member.name.clone()),
            ));
        }
        checks
    }

    /// The checks that apply to one package, rooted at `dir`.
    ///
    /// A tool the package does not depend on, or whose binary is not
    /// installed, produces no check at all — an absent check is honest, a
    /// failing one would not be.
    fn detect_in(manifest: &Manifest, dir: &Path, scope: Option<String>) -> Vec<Self> {
        let mut checks: Vec<Self> = Vec::new();

        for candidate in CANDIDATES {
            if checks.iter().any(|check| check.name == candidate.name) {
                continue;
            }
            if !manifest.depends_on(candidate.package) {
                continue;
            }
            let Some(program) = binary(dir, candidate.tool) else {
                continue;
            };
            checks.push(Self {
                name: candidate.name,
                tool: candidate.tool,
                scope: scope.clone(),
                program,
                args: candidate.args.to_vec(),
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
