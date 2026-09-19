//! Named sequences of checks.
//!
//! A workflow defines no work of its own. It names checks that already exist
//! and adds the few questions about repository state that only make sense
//! before a commit or a release — whether the tree is clean, whether the
//! version has been bumped.
//!
//! These are a convenience before pushing, not a substitute for CI. What is
//! binding stays in CI, where it cannot be skipped.

use std::path::Path;
use std::process::Command;

use runemark::Verdict;

/// A question about the repository rather than about the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// Nothing uncommitted.
    WorkingTreeClean,
    /// The version in `package.json` has no tag yet.
    VersionUnreleased,
}

impl Gate {
    pub fn name(self) -> &'static str {
        match self {
            Self::WorkingTreeClean => "Working tree clean",
            Self::VersionUnreleased => "Version not yet tagged",
        }
    }

    /// Answers the question, or explains why it could not be answered.
    pub fn check(self, root: &Path, version: Option<&str>) -> GateResult {
        match self {
            Self::WorkingTreeClean => match git(root, &["status", "--porcelain"]) {
                Some(output) if output.trim().is_empty() => GateResult::pass(),
                Some(output) => {
                    let changed = output.lines().count();
                    GateResult::fail(format!("{changed} uncommitted change(s)"))
                }
                None => GateResult::skip("not a git repository"),
            },
            Self::VersionUnreleased => {
                let Some(version) = version else {
                    return GateResult::skip("package.json has no version");
                };
                match git(root, &["tag", "--list", &format!("v{version}")]) {
                    Some(output) if output.trim().is_empty() => GateResult::pass(),
                    Some(_) => GateResult::fail(format!("v{version} is already tagged")),
                    None => GateResult::skip("not a git repository"),
                }
            }
        }
    }
}

/// How a gate answered.
#[derive(Debug, Clone)]
pub struct GateResult {
    pub verdict: Verdict,
    pub detail: Option<String>,
}

impl GateResult {
    fn pass() -> Self {
        Self {
            verdict: Verdict::Passed,
            detail: None,
        }
    }

    fn fail(detail: impl Into<String>) -> Self {
        Self {
            verdict: Verdict::Failed,
            detail: Some(detail.into()),
        }
    }

    /// A question that does not apply here is not a failure.
    fn skip(reason: impl Into<String>) -> Self {
        Self {
            verdict: Verdict::Skipped,
            detail: Some(reason.into()),
        }
    }
}

/// The workflows `opi` knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Workflow {
    Commit,
    Release,
}

impl Workflow {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "commit" => Some(Self::Commit),
            "release" => Some(Self::Release),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Commit => "before commit",
            Self::Release => "before release",
        }
    }

    /// The repository questions this workflow asks.
    ///
    /// Commit asks none: staging work in progress is normal, and refusing to
    /// report on a dirty tree would make the workflow useless exactly when it
    /// is wanted.
    pub fn gates(self) -> &'static [Gate] {
        match self {
            Self::Commit => &[],
            Self::Release => &[Gate::WorkingTreeClean, Gate::VersionUnreleased],
        }
    }

    /// Whether `check` belongs to this workflow.
    ///
    /// Release runs everything. Commit leaves out the slow and the networked:
    /// a pre-commit pass people wait through is a pre-commit pass people stop
    /// running.
    pub fn includes(self, check: &str) -> bool {
        match self {
            Self::Release => true,
            Self::Commit => !matches!(check, "Tests" | "Dead code"),
        }
    }
}

/// Runs `git` in `root`, or `None` when there is no repository to ask.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflows_are_named() {
        assert_eq!(Workflow::parse("commit"), Some(Workflow::Commit));
        assert_eq!(Workflow::parse("release"), Some(Workflow::Release));
        assert_eq!(Workflow::parse("deploy"), None);
    }

    #[test]
    fn commit_leaves_out_the_slow_checks() {
        // A pre-commit pass people wait through is one people stop running.
        assert!(!Workflow::Commit.includes("Tests"));
        assert!(!Workflow::Commit.includes("Dead code"));
        assert!(Workflow::Commit.includes("Lint & format"));
        assert!(Workflow::Commit.includes("TypeScript"));
        assert!(Workflow::Commit.includes("Secrets"));
    }

    #[test]
    fn release_runs_everything() {
        for check in ["Tests", "Dead code", "TypeScript", "Secrets"] {
            assert!(Workflow::Release.includes(check));
        }
    }

    #[test]
    fn only_release_asks_about_the_repository() {
        // Staging work in progress is normal; refusing to report on a dirty
        // tree would make the commit workflow useless when it is wanted.
        assert!(Workflow::Commit.gates().is_empty());
        assert_eq!(Workflow::Release.gates().len(), 2);
    }

    #[test]
    fn a_gate_outside_a_repository_is_skipped_not_failed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let result = Gate::WorkingTreeClean.check(dir.path(), None);
        assert_eq!(result.verdict, Verdict::Skipped);
    }

    #[test]
    fn a_missing_version_skips_the_tag_question() {
        let dir = tempfile::tempdir().expect("temp dir");
        let result = Gate::VersionUnreleased.check(dir.path(), None);
        assert_eq!(result.verdict, Verdict::Skipped);
        assert_eq!(
            result.detail.as_deref(),
            Some("package.json has no version")
        );
    }

    #[test]
    fn a_dirty_tree_fails_and_says_how_dirty() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(
            git(dir.path(), &["init"]).is_some(),
            "git must be available"
        );
        std::fs::write(dir.path().join("new.txt"), "x").expect("write");

        let result = Gate::WorkingTreeClean.check(dir.path(), None);
        assert_eq!(result.verdict, Verdict::Failed);
        assert!(result.detail.expect("detail").contains("1 uncommitted"));
    }

    #[test]
    fn a_clean_tree_passes() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(git(dir.path(), &["init"]).is_some());
        assert_eq!(
            Gate::WorkingTreeClean.check(dir.path(), None).verdict,
            Verdict::Passed
        );
    }
}
