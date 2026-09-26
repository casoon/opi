//! Named sequences of checks.
//!
//! A workflow defines no work of its own. It names checks that already exist
//! and adds the few questions about repository state that only make sense
//! before a commit or a release — whether the tree is clean, whether the
//! version has been bumped.
//!
//! `push` is the one meant to be binding: installed as a pre-push hook with
//! `opi --hooks`, it runs what a CI would — every check, then the build — so a
//! repository without CI still cannot push a red state by accident. The escape
//! hatch is git's own, `git push --no-verify`.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use runemark::Verdict;

/// A question about the repository rather than about the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// Nothing uncommitted.
    WorkingTreeClean,
    /// The version in `package.json` has no tag yet.
    VersionUnreleased,
    /// The upstream branch has nothing this one lacks.
    NotBehindUpstream,
}

impl Gate {
    pub fn name(self) -> &'static str {
        match self {
            Self::WorkingTreeClean => "Working tree clean",
            Self::VersionUnreleased => "Version not yet tagged",
            Self::NotBehindUpstream => "Not behind upstream",
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
            // Against the last fetched state, not the remote itself: a hook
            // that waits on the network is a hook people disable. A push that
            // would be rejected anyway is caught before minutes of checks.
            Self::NotBehindUpstream => {
                match git(root, &["rev-list", "--count", "HEAD..@{upstream}"]) {
                    Some(output) if output.trim() == "0" => GateResult::pass(),
                    Some(output) => {
                        GateResult::fail(format!("{} commit(s) behind — pull first", output.trim()))
                    }
                    None => GateResult::skip("no upstream branch"),
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
    Push,
    Release,
}

impl Workflow {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "commit" => Some(Self::Commit),
            "push" => Some(Self::Push),
            "release" => Some(Self::Release),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Commit => "before commit",
            Self::Push => "before push",
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
            // Clean, because the checks run on the working tree and only what
            // is committed gets pushed.
            Self::Push => &[Gate::WorkingTreeClean, Gate::NotBehindUpstream],
            Self::Release => &[Gate::WorkingTreeClean, Gate::VersionUnreleased],
        }
    }

    /// Whether the project is built after the checks.
    ///
    /// Commit does not: a build is the slowest step there is, and the point
    /// of the commit workflow is to be fast enough to keep running.
    pub fn builds(self) -> bool {
        !matches!(self, Self::Commit)
    }

    /// Whether `check` belongs to this workflow.
    ///
    /// Release runs everything. Commit leaves out the slow and the networked:
    /// a pre-commit pass people wait through is a pre-commit pass people stop
    /// running.
    pub fn includes(self, check: &str) -> bool {
        match self {
            Self::Push | Self::Release => true,
            Self::Commit => !matches!(check, "Tests" | "Dead code"),
        }
    }
}

/// What the pre-push hook runs.
const HOOK_COMMAND: &str = "opi --check push";

/// What `opi --hooks` did.
#[derive(Debug, PartialEq, Eq)]
pub enum HookInstall {
    /// A new hook file, holding only the opi line.
    Created(PathBuf),
    /// An existing hook, with the opi line added after its own.
    Appended(PathBuf),
    /// The hook already runs opi; nothing was written.
    Present(PathBuf),
}

/// Why the hook could not be installed.
#[derive(Debug)]
pub enum HookError {
    NotARepository,
    Io(io::Error),
}

impl From<io::Error> for HookError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Installs the push workflow as the repository's pre-push hook.
///
/// A husky directory wins over `.git/hooks`: that is where the project keeps
/// its hooks, committed and shared, and a line in `.git/hooks` would reach this
/// clone only. Without husky, git is asked where hooks live, which honours
/// `core.hooksPath` and worktrees alike.
///
/// Idempotent, and never destructive: an existing hook keeps every line it
/// had, and a hook that already runs opi is left untouched.
pub fn install_hook(root: &Path) -> Result<HookInstall, HookError> {
    let toplevel = git(root, &["rev-parse", "--show-toplevel"])
        .map(|output| PathBuf::from(output.trim()))
        .ok_or(HookError::NotARepository)?;

    let husky = toplevel.join(".husky");
    let dir = if husky.is_dir() {
        husky
    } else {
        git(
            root,
            &["rev-parse", "--path-format=absolute", "--git-path", "hooks"],
        )
        .map(|output| PathBuf::from(output.trim()))
        .ok_or(HookError::NotARepository)?
    };
    let hook = dir.join("pre-push");

    // Git runs hooks from the top of the repository. A project further down
    // has to be stepped into, or opi would find no project there.
    let project = root.canonicalize()?;
    let line = match project.strip_prefix(toplevel.canonicalize()?) {
        Ok(relative) if !relative.as_os_str().is_empty() => {
            format!("(cd '{}' && {HOOK_COMMAND})", relative.display())
        }
        _ => HOOK_COMMAND.to_owned(),
    };

    if hook.exists() {
        let mut contents = fs::read_to_string(&hook)?;
        if contents.contains(HOOK_COMMAND) {
            return Ok(HookInstall::Present(hook));
        }
        if !contents.is_empty() && !contents.ends_with('\n') {
            contents.push('\n');
        }
        contents.push_str(&line);
        contents.push('\n');
        fs::write(&hook, contents)?;
        return Ok(HookInstall::Appended(hook));
    }

    fs::create_dir_all(&dir)?;
    fs::write(&hook, format!("#!/bin/sh\n{line}\n"))?;
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
    Ok(HookInstall::Created(hook))
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
        assert_eq!(Workflow::parse("push"), Some(Workflow::Push));
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
    fn push_and_release_run_everything_and_build() {
        for workflow in [Workflow::Push, Workflow::Release] {
            for check in ["Tests", "Dead code", "TypeScript", "Secrets"] {
                assert!(workflow.includes(check));
            }
            assert!(workflow.builds());
        }
        assert!(!Workflow::Commit.builds());
    }

    #[test]
    fn commit_asks_nothing_about_the_repository() {
        // Staging work in progress is normal; refusing to report on a dirty
        // tree would make the commit workflow useless when it is wanted.
        assert!(Workflow::Commit.gates().is_empty());
        assert_eq!(Workflow::Push.gates().len(), 2);
        assert_eq!(Workflow::Release.gates().len(), 2);
    }

    #[test]
    fn a_branch_without_upstream_skips_the_behind_question() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(git(dir.path(), &["init"]).is_some());
        let result = Gate::NotBehindUpstream.check(dir.path(), None);
        assert_eq!(result.verdict, Verdict::Skipped);
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

    #[test]
    fn the_hook_is_created_once_and_then_left_alone() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(git(dir.path(), &["init"]).is_some());

        let HookInstall::Created(hook) = install_hook(dir.path()).expect("installed") else {
            panic!("a fresh repository has no hook");
        };
        assert!(hook.ends_with(".git/hooks/pre-push"));
        let contents = fs::read_to_string(&hook).expect("read");
        assert_eq!(contents, "#!/bin/sh\nopi --check push\n");
        let mode = fs::metadata(&hook).expect("stat").permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "git skips a hook it cannot execute");

        assert_eq!(
            install_hook(dir.path()).expect("second run"),
            HookInstall::Present(hook)
        );
    }

    #[test]
    fn an_existing_hook_keeps_its_lines() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(git(dir.path(), &["init"]).is_some());
        let husky = dir.path().join(".husky");
        fs::create_dir_all(&husky).expect("mkdir");
        fs::write(husky.join("pre-push"), "pnpm verify").expect("write");

        let result = install_hook(dir.path()).expect("installed");
        assert!(
            matches!(result, HookInstall::Appended(ref hook) if hook.ends_with(".husky/pre-push"))
        );
        assert_eq!(
            fs::read_to_string(husky.join("pre-push")).expect("read"),
            "pnpm verify\nopi --check push\n"
        );
    }

    #[test]
    fn a_project_below_the_repository_root_is_stepped_into() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(git(dir.path(), &["init"]).is_some());
        let app = dir.path().join("app");
        fs::create_dir_all(&app).expect("mkdir");

        let HookInstall::Created(hook) = install_hook(&app).expect("installed") else {
            panic!("a fresh repository has no hook");
        };
        assert!(
            fs::read_to_string(hook)
                .expect("read")
                .contains("(cd 'app' && opi --check push)")
        );
    }

    #[test]
    fn outside_a_repository_there_is_no_hook_to_install() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(matches!(
            install_hook(dir.path()),
            Err(HookError::NotARepository)
        ));
    }
}
