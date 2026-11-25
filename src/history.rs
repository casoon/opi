//! What the workflows found, remembered per clone.
//!
//! Each workflow run appends one line to `checks.jsonl` in git's own
//! directory: never versioned, never shared, gone with the clone. Two things
//! read it back. The push workflow skips its checks for a tree that already
//! passed them, so a second push of the same commits costs nothing; and the
//! start screen names the last result for the commit that is checked out.
//!
//! The file is a convenience. Without it, or with a line that does not parse,
//! everything runs as if it had never been written.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// How many runs are kept. Enough to find the last green tree after a day of
/// pushing branches; old enough to be worthless after that.
const KEEP: usize = 500;

/// One workflow run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub workflow: String,
    pub commit: String,
    /// The tree that was checked, only when nothing was uncommitted: a dirty
    /// working tree is not the tree of any commit.
    pub tree: Option<String>,
    pub ready: bool,
    pub checks: Vec<CheckResult>,
    pub duration_ms: u64,
    /// Seconds since the Unix epoch.
    pub time: u64,
    /// The opi that ran it. A newer opi may check more, so its runs are not
    /// taken on trust from an older one.
    pub opi: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckResult {
    pub name: String,
    pub status: String,
}

impl Entry {
    /// The first check that did not pass.
    pub fn first_failure(&self) -> Option<&str> {
        self.checks
            .iter()
            .find(|check| check.status == "failed" || check.status == "unavailable")
            .map(|check| check.name.as_str())
    }

    /// One line for the start screen: `✓ push 12 min ago`, or what stopped it.
    /// A run on uncommitted changes says so, since it did not check the commit.
    pub fn status(&self, now: u64) -> String {
        let outcome = if self.ready {
            format!("✓ {}", self.workflow)
        } else {
            match self.first_failure() {
                Some(check) => format!("✗ {}: {check}", self.workflow),
                None => format!("✗ {}", self.workflow),
            }
        };
        let changes = if self.tree.is_none() {
            " with uncommitted changes"
        } else {
            ""
        };
        format!("{outcome}{changes} {}", ago(self.time, now))
    }
}

/// The commit checked out and, when nothing is uncommitted, its tree.
pub struct State {
    pub commit: String,
    pub tree: Option<String>,
}

/// Where the repository stands, or `None` outside one or before the first
/// commit.
pub fn state(root: &Path) -> Option<State> {
    let commit = git(root, &["rev-parse", "HEAD"])?;
    let clean = git(root, &["status", "--porcelain"])?.is_empty();
    let tree = if clean {
        Some(git(root, &["rev-parse", "HEAD^{tree}"])?)
    } else {
        None
    };
    Some(State { commit, tree })
}

/// The run of `workflow` that already passed on exactly this tree, with this
/// opi.
pub fn green(root: &Path, workflow: &str, tree: &str) -> Option<Entry> {
    read(&file(root)?).into_iter().rev().find(|entry| {
        entry.ready
            && entry.workflow == workflow
            && entry.tree.as_deref() == Some(tree)
            && entry.opi == env!("CARGO_PKG_VERSION")
    })
}

/// The last run on `commit`, whatever the workflow.
pub fn latest(root: &Path, commit: &str) -> Option<Entry> {
    read(&file(root)?)
        .into_iter()
        .rev()
        .find(|entry| entry.commit == commit)
}

/// Appends a run. Failing to write is not reported: a hook must not fail
/// because a cache could not be kept.
pub fn record(root: &Path, entry: &Entry) {
    let Some(path) = file(root) else {
        return;
    };
    let _ = append(&path, entry);
}

fn append(path: &Path, entry: &Entry) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let line = serde_json::to_string(entry).map_err(std::io::Error::other)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    drop(file);

    let contents = fs::read_to_string(path)?;
    let lines: Vec<&str> = contents.lines().collect();
    if lines.len() > KEEP {
        let kept = lines[lines.len() - KEEP..].join("\n");
        fs::write(path, format!("{kept}\n"))?;
    }
    Ok(())
}

fn read(path: &Path) -> Vec<Entry> {
    fs::read_to_string(path)
        .map(|contents| {
            contents
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// `checks.jsonl` in git's common directory, shared by every worktree of the
/// clone: a tree that passed in one has passed in all of them.
fn file(root: &Path) -> Option<PathBuf> {
    git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .map(|dir| PathBuf::from(dir).join("opi").join("checks.jsonl"))
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// `just now`, `4 min ago`, `3 h ago`, `2 days ago`.
pub fn ago(time: u64, now: u64) -> String {
    let seconds = now.saturating_sub(time);
    match seconds {
        0..60 => "just now".to_owned(),
        60..3600 => format!("{} min ago", seconds / 60),
        3600..86_400 => format!("{} h ago", seconds / 3600),
        _ => crate::plural((seconds / 86_400) as usize, "day ago", "days ago"),
    }
}

/// The first seven characters of a commit, as git shows it.
pub fn short(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        for args in [
            &["init", "-q"][..],
            &["config", "user.email", "test@example.com"],
            &["config", "user.name", "Test"],
            &["config", "commit.gpgsign", "false"],
        ] {
            assert!(git(dir.path(), args).is_some(), "git {args:?}");
        }
        fs::write(dir.path().join("a.txt"), "a").expect("write");
        assert!(git(dir.path(), &["add", "."]).is_some());
        assert!(git(dir.path(), &["commit", "-qm", "a"]).is_some());
        dir
    }

    fn entry(state: &State, ready: bool) -> Entry {
        Entry {
            workflow: "push".to_owned(),
            commit: state.commit.clone(),
            tree: state.tree.clone(),
            ready,
            checks: vec![CheckResult {
                name: "Tests".to_owned(),
                status: if ready { "passed" } else { "failed" }.to_owned(),
            }],
            duration_ms: 1200,
            time: now(),
            opi: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }

    #[test]
    fn a_green_clean_tree_is_found_again() {
        let dir = repository();
        let state = state(dir.path()).expect("state");
        let tree = state.tree.clone().expect("clean");
        assert!(green(dir.path(), "push", &tree).is_none());

        record(dir.path(), &entry(&state, true));
        assert!(green(dir.path(), "push", &tree).is_some());
        assert!(green(dir.path(), "release", &tree).is_none());
        assert!(dir.path().join(".git/opi/checks.jsonl").is_file());
    }

    #[test]
    fn a_red_run_is_never_taken_as_green() {
        let dir = repository();
        let state = state(dir.path()).expect("state");
        record(dir.path(), &entry(&state, false));
        assert!(green(dir.path(), "push", state.tree.as_deref().expect("clean")).is_none());
        assert_eq!(
            latest(dir.path(), &state.commit)
                .expect("recorded")
                .first_failure(),
            Some("Tests")
        );
    }

    #[test]
    fn a_dirty_tree_has_no_tree_to_remember() {
        let dir = repository();
        fs::write(dir.path().join("b.txt"), "b").expect("write");
        let state = state(dir.path()).expect("state");
        assert!(state.tree.is_none());
    }

    #[test]
    fn another_opi_version_runs_again() {
        let dir = repository();
        let state = state(dir.path()).expect("state");
        let mut old = entry(&state, true);
        old.opi = "0.0.1".to_owned();
        record(dir.path(), &old);
        assert!(green(dir.path(), "push", state.tree.as_deref().expect("clean")).is_none());
    }

    #[test]
    fn unreadable_lines_are_passed_over() {
        let dir = repository();
        let state = state(dir.path()).expect("state");
        fs::create_dir_all(dir.path().join(".git/opi")).expect("mkdir");
        fs::write(dir.path().join(".git/opi/checks.jsonl"), "not json\n").expect("write");
        record(dir.path(), &entry(&state, true));
        assert!(green(dir.path(), "push", state.tree.as_deref().expect("clean")).is_some());
    }

    #[test]
    fn the_file_is_kept_short() {
        let dir = repository();
        let state = state(dir.path()).expect("state");
        for _ in 0..KEEP + 3 {
            record(dir.path(), &entry(&state, true));
        }
        let lines = fs::read_to_string(dir.path().join(".git/opi/checks.jsonl"))
            .expect("read")
            .lines()
            .count();
        assert_eq!(lines, KEEP);
    }

    #[test]
    fn outside_a_repository_nothing_is_written_or_found() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(state(dir.path()).is_none());
        assert!(latest(dir.path(), "abc").is_none());
    }

    #[test]
    fn the_status_names_the_workflow_and_what_stopped_it() {
        let dir = repository();
        let state = state(dir.path()).expect("state");
        let now = now();
        assert_eq!(entry(&state, true).status(now), "✓ push just now");
        assert_eq!(entry(&state, false).status(now), "✗ push: Tests just now");
        let mut dirty = entry(&state, true);
        dirty.tree = None;
        assert_eq!(
            dirty.status(now),
            "✓ push with uncommitted changes just now"
        );
    }

    #[test]
    fn ages_read_like_git() {
        assert_eq!(ago(100, 130), "just now");
        assert_eq!(ago(0, 240), "4 min ago");
        assert_eq!(ago(0, 3 * 3600), "3 h ago");
        assert_eq!(ago(0, 86_400), "1 day ago");
        assert_eq!(ago(0, 2 * 86_400), "2 days ago");
    }
}
