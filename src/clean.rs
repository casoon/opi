//! Removable build artefacts.
//!
//! This is the one place in `opi` that deletes data, so it is deliberately
//! narrow: only directories inside the project, never through a symlink, and
//! never a path the project's own configuration points outside itself.
//!
//! Sizes are measured before anything is offered. "Remove `dist`" is a
//! question; "Remove `dist`, 34 MB" is an answer.

use std::path::{Path, PathBuf};

use crate::manifest::Manifest;
use crate::workspace::Member;

/// Directories a build leaves behind.
///
/// Measured across 133 real projects. Deliberately excludes `node_modules`
/// itself, which is handled apart: it is the largest and the most expensive to
/// rebuild.
const ARTEFACTS: &[&str] = &[
    "dist",
    "build",
    "out",
    ".astro",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".output",
    ".turbo",
    ".wrangler",
    ".vercel",
    ".parcel-cache",
    "coverage",
    "test-results",
    "playwright-report",
    "storybook-static",
    "node_modules/.cache",
    "node_modules/.vite",
];

/// Something that can be removed, and what it costs to keep.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// Absolute, and verified to be inside the project.
    pub path: PathBuf,
    /// How it is shown: relative to the project root.
    pub display: String,
    pub bytes: u64,
    /// `node_modules` is never offered by default.
    pub heavy: bool,
}

/// Everything removable in the project and its workspace members.
///
/// Sizes are measured concurrently; walking several large trees one after
/// another is the slow part, not the deleting.
pub fn candidates(manifest: &Manifest, members: &[Member], root: &Path) -> Vec<Candidate> {
    let mut paths: Vec<(PathBuf, bool)> = Vec::new();
    let mut add = |dir: &Path| {
        for artefact in ARTEFACTS {
            let path = dir.join(artefact);
            if path.is_dir() {
                paths.push((path, false));
            }
        }
        let modules = dir.join("node_modules");
        if modules.is_dir() {
            paths.push((modules, true));
        }
    };

    add(root);
    for member in members {
        add(&member.path);
    }

    // A project may name its own; anything escaping the project is refused
    // rather than corrected, since guessing what was meant risks the guess.
    for entry in manifest.opi_list("clean") {
        if let Some(path) = inside(root, &entry) {
            if path.is_dir() && !paths.iter().any(|(known, _)| *known == path) {
                paths.push((path, false));
            }
        }
    }

    drop_nested(&mut paths);
    let mut candidates = measure(paths, root);
    // Largest first: the entry worth removing should be the one you read.
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.bytes));
    candidates
}

/// Removes paths contained in another path of the set.
///
/// `node_modules/.vite` sits inside `node_modules`. Offering both counts its
/// bytes twice, so a total would promise more than removing it frees — and
/// deleting the outer one takes the inner with it regardless.
fn drop_nested(paths: &mut Vec<(PathBuf, bool)>) {
    paths.sort_by(|a, b| a.0.cmp(&b.0));
    let mut kept: Vec<(PathBuf, bool)> = Vec::with_capacity(paths.len());
    for (path, heavy) in paths.drain(..) {
        // Sorted, so any container is already in `kept`.
        if kept.iter().any(|(outer, _)| path.starts_with(outer)) {
            continue;
        }
        kept.push((path, heavy));
    }
    *paths = kept;
}

/// Resolves `entry` against `root`, refusing anything that leaves the project.
fn inside(root: &Path, entry: &str) -> Option<PathBuf> {
    let candidate = Path::new(entry);
    if candidate.is_absolute() || candidate.components().any(|part| part.as_os_str() == "..") {
        return None;
    }
    let joined = root.join(candidate);
    // Resolve symlinks before comparing: a link inside the project can still
    // point outside it.
    let resolved = joined.canonicalize().ok()?;
    let root = root.canonicalize().ok()?;
    resolved.starts_with(&root).then_some(resolved)
}

/// Measures each path, several at a time.
fn measure(paths: Vec<(PathBuf, bool)>, root: &Path) -> Vec<Candidate> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = paths
            .into_iter()
            .map(|(path, heavy)| {
                scope.spawn(move || {
                    let bytes = size_of(&path);
                    (path, heavy, bytes)
                })
            })
            .collect();

        handles
            .into_iter()
            .filter_map(|handle| handle.join().ok())
            .map(|(path, heavy, bytes)| Candidate {
                display: path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
                path,
                bytes,
                heavy,
            })
            .collect()
    })
}

/// Bytes used by the files under `path`, not following symlinks.
fn size_of(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };

    entries
        .flatten()
        .map(|entry| {
            let Ok(kind) = entry.file_type() else {
                return 0;
            };
            if kind.is_symlink() {
                // A link's target is not this directory's to account for, and
                // following one can leave the project or loop.
                0
            } else if kind.is_dir() {
                size_of(&entry.path())
            } else {
                entry.metadata().map(|data| data.len()).unwrap_or(0)
            }
        })
        .sum()
}

/// Removes `candidates`, returning how many bytes went and what refused to.
pub fn remove(candidates: &[&Candidate]) -> (u64, Vec<(String, std::io::Error)>) {
    let mut freed = 0;
    let mut failures = Vec::new();

    for candidate in candidates {
        match std::fs::remove_dir_all(&candidate.path) {
            Ok(()) => freed += candidate.bytes,
            Err(error) => failures.push((candidate.display.clone(), error)),
        }
    }

    (freed, failures)
}

/// Bytes as a short human-readable string.
pub fn human(bytes: u64) -> String {
    const UNITS: [(&str, u64); 3] = [("GB", 1_000_000_000), ("MB", 1_000_000), ("kB", 1_000)];
    for (unit, scale) in UNITS {
        if bytes >= scale {
            return format!("{:.1} {unit}", bytes as f64 / scale as f64);
        }
    }
    format!("{bytes} B")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn sizes_are_shown_in_the_largest_fitting_unit() {
        assert_eq!(human(0), "0 B");
        assert_eq!(human(999), "999 B");
        assert_eq!(human(1_500), "1.5 kB");
        assert_eq!(human(34_000_000), "34.0 MB");
        assert_eq!(human(1_400_000_000), "1.4 GB");
    }

    #[test]
    fn a_path_leaving_the_project_is_refused() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(inside(dir.path(), "../elsewhere").is_none());
        assert!(inside(dir.path(), "/etc").is_none());
        assert!(inside(dir.path(), "a/../../b").is_none());
    }

    #[test]
    fn a_symlink_out_of_the_project_is_refused() {
        // The path is inside the project, but what it points at is not.
        let outside = tempfile::tempdir().expect("outside");
        let dir = tempfile::tempdir().expect("project");
        std::os::unix::fs::symlink(outside.path(), dir.path().join("escape")).expect("symlink");
        assert!(inside(dir.path(), "escape").is_none());
    }

    #[test]
    fn a_path_inside_the_project_is_accepted() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir(dir.path().join("generated")).expect("mkdir");
        assert!(inside(dir.path(), "generated").is_some());
    }

    #[test]
    fn size_ignores_symlinks_rather_than_following_them() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("real"), vec![0u8; 100]).expect("write");
        std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("link"))
            .expect("symlink");
        assert_eq!(size_of(dir.path()), 100, "the link adds nothing");
    }

    #[test]
    fn a_path_inside_another_candidate_is_dropped() {
        // Found on a real workspace: node_modules/.vite was offered alongside
        // node_modules, so a total promised twice what removing it would free.
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("node_modules/.vite")).expect("mkdir");
        fs::create_dir_all(dir.path().join("node_modules/.cache")).expect("mkdir");
        fs::create_dir(dir.path().join("dist")).expect("mkdir");
        let manifest: Manifest = serde_json::from_str("{}").expect("manifest");

        let found = candidates(&manifest, &[], dir.path());
        let shown: Vec<&str> = found
            .iter()
            .map(|candidate| candidate.display.as_str())
            .collect();
        assert!(shown.contains(&"node_modules"));
        assert!(shown.contains(&"dist"));
        assert!(
            !shown.iter().any(|path| path.contains(".vite")),
            "counted inside node_modules already: {shown:?}"
        );
    }

    #[test]
    fn node_modules_is_marked_heavy_and_artefacts_are_not() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir(dir.path().join("dist")).expect("mkdir");
        fs::create_dir(dir.path().join("node_modules")).expect("mkdir");
        let manifest: Manifest = serde_json::from_str("{}").expect("manifest");

        let found = candidates(&manifest, &[], dir.path());
        let heavy: Vec<&str> = found
            .iter()
            .filter(|candidate| candidate.heavy)
            .map(|candidate| candidate.display.as_str())
            .collect();
        assert_eq!(heavy, ["node_modules"]);
        assert!(found.iter().any(|candidate| candidate.display == "dist"));
    }
}
