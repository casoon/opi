//! Workspace member discovery.
//!
//! A monorepo root usually carries orchestrating scripts of its own while the
//! interesting per-app scripts live in its members. Finding the members lets
//! `opi` offer both from one place.
//!
//! Patterns come from `pnpm-workspace.yaml` or the `workspaces` field, and are
//! matched against the filesystem. Nothing here runs the package manager.

use std::path::{Path, PathBuf};

use crate::manifest::Manifest;

/// A workspace member with scripts.
#[derive(Debug, Clone)]
pub struct Member {
    /// The package's `name`, used to address it when running a script.
    pub name: String,
    pub manifest: Manifest,
}

/// Finds the members of the workspace rooted at `dir`.
///
/// Returns an empty list for a project that declares no workspace, so the
/// caller needs no special case for the ordinary single-package repository.
pub fn members(dir: &Path, manifest: &Manifest) -> Vec<Member> {
    let (includes, excludes) = patterns(dir, manifest);
    if includes.is_empty() {
        return Vec::new();
    }

    // A workspace may list "." among its packages, making the root its own
    // member. Its scripts are already on screen, so including it again shows
    // every one of them twice.
    let root = dir.canonicalize().ok();

    let mut members: Vec<Member> = expand(dir, &includes)
        .into_iter()
        .filter(|path| path.canonicalize().ok() != root)
        .filter(|path| !matches_any(dir, path, &excludes))
        .filter_map(|path| {
            let manifest = Manifest::load(&path).ok()?;
            // A member without scripts has nothing to offer a menu.
            if manifest.scripts.is_empty() {
                return None;
            }
            let name = manifest.name.clone().or_else(|| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })?;
            Some(Member { name, manifest })
        })
        .collect();

    members.sort_by(|a, b| a.name.cmp(&b.name));
    members.dedup_by(|a, b| a.name == b.name);
    members
}

/// The include and exclude patterns declared by the workspace.
fn patterns(dir: &Path, manifest: &Manifest) -> (Vec<String>, Vec<String>) {
    let declared = pnpm_packages(&dir.join("pnpm-workspace.yaml"))
        .unwrap_or_else(|| manifest.workspaces.clone());

    let (excludes, includes): (Vec<String>, Vec<String>) = declared
        .into_iter()
        .partition(|pattern| pattern.starts_with('!'));

    (
        includes,
        excludes
            .into_iter()
            .map(|pattern| pattern[1..].to_owned())
            .collect(),
    )
}

/// Reads the `packages:` list out of a `pnpm-workspace.yaml`.
///
/// Deliberately reads only that one block. Real files carry several unrelated
/// top-level keys whose values are also lists — `minimumReleaseAgeExclude` and
/// `trustPolicyExclude` among them — and a parser that collected every `- item`
/// in the file would treat those version pins as workspace packages.
///
/// Returns `None` when the file is absent or declares no `packages:` key, so
/// the caller can fall back to the manifest.
fn pnpm_packages(path: &Path) -> Option<Vec<String>> {
    let contents = std::fs::read_to_string(path).ok()?;
    let mut packages = Vec::new();
    let mut inside = false;

    for line in contents.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim_start().starts_with('#') || trimmed.trim().is_empty() {
            continue;
        }

        if inside {
            // Any line back at column zero ends the block.
            if !trimmed.starts_with([' ', '\t']) {
                break;
            }
            if let Some(entry) = trimmed.trim_start().strip_prefix("- ") {
                packages.push(unquote(entry));
            }
            continue;
        }

        if trimmed.starts_with("packages:") {
            inside = true;
        }
    }

    inside.then_some(packages)
}

/// Strips surrounding quotes and a trailing comment.
fn unquote(entry: &str) -> String {
    let entry = entry.trim();
    let entry = match entry.chars().next() {
        Some(quote @ ('\'' | '"')) => entry
            .strip_prefix(quote)
            .and_then(|rest| rest.split(quote).next())
            .unwrap_or(entry),
        _ => entry.split('#').next().unwrap_or(entry).trim(),
    };
    entry.to_owned()
}

/// Resolves patterns to directories that hold a `package.json`.
fn expand(dir: &Path, patterns: &[String]) -> Vec<PathBuf> {
    let mut found = Vec::new();

    for pattern in patterns {
        let full = dir.join(pattern).join("package.json");
        let Some(pattern) = full.to_str() else {
            continue;
        };
        let Ok(paths) = glob::glob(pattern) else {
            continue;
        };
        for path in paths.flatten() {
            // A pattern like `packages/**/*` reaches into installed
            // dependencies, which are not workspace members.
            if path
                .components()
                .any(|part| part.as_os_str() == "node_modules")
            {
                continue;
            }
            if let Some(parent) = path.parent() {
                found.push(parent.to_path_buf());
            }
        }
    }

    found.sort();
    found.dedup();
    found
}

/// Whether `path` is covered by any of the exclude patterns.
fn matches_any(dir: &Path, path: &Path, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| {
        let pattern = dir.join(pattern);
        pattern
            .to_str()
            .and_then(|pattern| glob::Pattern::new(pattern).ok())
            .is_some_and(|pattern| pattern.matches_path(path))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Builds a workspace on disk and discovers its members.
    fn workspace(files: &[(&str, &str)]) -> (tempfile::TempDir, Vec<Member>) {
        let dir = tempfile::tempdir().expect("temp dir");
        for (path, contents) in files {
            let full = dir.path().join(path);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).expect("mkdir");
            }
            fs::write(full, contents).expect("write");
        }
        let manifest = Manifest::load(dir.path()).expect("root manifest");
        let members = members(dir.path(), &manifest);
        (dir, members)
    }

    fn names(members: &[Member]) -> Vec<&str> {
        members.iter().map(|member| member.name.as_str()).collect()
    }

    #[test]
    fn a_project_without_a_workspace_has_no_members() {
        let (_dir, members) = workspace(&[("package.json", r#"{"name":"solo"}"#)]);
        assert!(members.is_empty());
    }

    #[test]
    fn the_workspaces_field_finds_members() {
        let (_dir, members) = workspace(&[
            ("package.json", r#"{"name":"root","workspaces":["apps/*"]}"#),
            (
                "apps/blog/package.json",
                r#"{"name":"blog","scripts":{"dev":"x"}}"#,
            ),
            (
                "apps/shop/package.json",
                r#"{"name":"shop","scripts":{"dev":"x"}}"#,
            ),
        ]);
        assert_eq!(names(&members), ["blog", "shop"]);
    }

    #[test]
    fn pnpm_workspace_yaml_wins_over_the_manifest_field() {
        let (_dir, members) = workspace(&[
            (
                "package.json",
                r#"{"name":"root","workspaces":["ignored/*"]}"#,
            ),
            ("pnpm-workspace.yaml", "packages:\n  - 'apps/*'\n"),
            (
                "apps/blog/package.json",
                r#"{"name":"blog","scripts":{"dev":"x"}}"#,
            ),
            (
                "ignored/nope/package.json",
                r#"{"name":"nope","scripts":{"dev":"x"}}"#,
            ),
        ]);
        assert_eq!(names(&members), ["blog"]);
    }

    #[test]
    fn only_the_packages_block_of_the_yaml_is_read() {
        // Real pnpm-workspace.yaml files carry several unrelated top-level keys
        // whose values are also lists. Collecting every "- item" would turn
        // version pins into workspace packages.
        let yaml = "packages:\n  - 'apps/*'\n\nminimumReleaseAgeExclude:\n  - '@biomejs/*'\n  - 'apps/*'\n\noverrides:\n  nanoid: ^3.3.18\n";
        let (_dir, members) = workspace(&[
            ("package.json", r#"{"name":"root"}"#),
            ("pnpm-workspace.yaml", yaml),
            (
                "apps/blog/package.json",
                r#"{"name":"blog","scripts":{"dev":"x"}}"#,
            ),
        ]);
        assert_eq!(names(&members), ["blog"]);
    }

    #[test]
    fn negated_patterns_exclude_members() {
        let yaml = "packages:\n  - 'apps/*'\n  - '!apps/private'\n";
        let (_dir, members) = workspace(&[
            ("package.json", r#"{"name":"root"}"#),
            ("pnpm-workspace.yaml", yaml),
            (
                "apps/blog/package.json",
                r#"{"name":"blog","scripts":{"dev":"x"}}"#,
            ),
            (
                "apps/private/package.json",
                r#"{"name":"private","scripts":{"dev":"x"}}"#,
            ),
        ]);
        assert_eq!(names(&members), ["blog"]);
    }

    #[test]
    fn the_root_is_not_a_member_of_itself() {
        // Found in a real repository: pnpm-workspace.yaml listing "." made
        // every root script appear a second time under its own package name.
        let (_dir, members) = workspace(&[
            (
                "package.json",
                r#"{"name":"@casoon/thing","scripts":{"build":"x"}}"#,
            ),
            ("pnpm-workspace.yaml", "packages:\n  - '.'\n"),
        ]);
        assert!(members.is_empty());
    }

    #[test]
    fn members_without_scripts_are_left_out() {
        let (_dir, members) = workspace(&[
            ("package.json", r#"{"name":"root","workspaces":["apps/*"]}"#),
            (
                "apps/blog/package.json",
                r#"{"name":"blog","scripts":{"dev":"x"}}"#,
            ),
            ("apps/types/package.json", r#"{"name":"types"}"#),
        ]);
        assert_eq!(
            names(&members),
            ["blog"],
            "a member with nothing to run is noise"
        );
    }

    #[test]
    fn node_modules_is_never_a_workspace_member() {
        // "packages/**/*" reaches into installed dependencies otherwise.
        let (_dir, members) = workspace(&[
            (
                "package.json",
                r#"{"name":"root","workspaces":["packages/**/*"]}"#,
            ),
            (
                "packages/ui/package.json",
                r#"{"name":"ui","scripts":{"dev":"x"}}"#,
            ),
            (
                "packages/ui/node_modules/dep/package.json",
                r#"{"name":"dep","scripts":{"dev":"x"}}"#,
            ),
        ]);
        assert_eq!(names(&members), ["ui"]);
    }

    #[test]
    fn a_member_without_a_name_falls_back_to_its_directory() {
        let (_dir, members) = workspace(&[
            ("package.json", r#"{"name":"root","workspaces":["apps/*"]}"#),
            ("apps/blog/package.json", r#"{"scripts":{"dev":"x"}}"#),
        ]);
        assert_eq!(names(&members), ["blog"]);
    }

    #[test]
    fn comments_and_quoting_styles_are_tolerated() {
        let yaml =
            "# leading comment\npackages:\n  # inner comment\n  - \"apps/*\"\n  - 'shared'\n";
        let (_dir, members) = workspace(&[
            ("package.json", r#"{"name":"root"}"#),
            ("pnpm-workspace.yaml", yaml),
            (
                "apps/blog/package.json",
                r#"{"name":"blog","scripts":{"dev":"x"}}"#,
            ),
            (
                "shared/package.json",
                r#"{"name":"shared","scripts":{"build":"x"}}"#,
            ),
        ]);
        assert_eq!(names(&members), ["blog", "shared"]);
    }
}
