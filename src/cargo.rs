//! Rust projects.
//!
//! Measured across 231 directories here: `Cargo.toml` in 51, `package.json` in
//! 133, and twelve carrying both. A .NET marker appeared in none on its own,
//! so that half of the original plan is not built — the same reasoning that
//! replaced Knip and taze elsewhere.
//!
//! Unlike npm, a Rust project defines no scripts. Its commands are the same
//! everywhere, so this offers the ones worth a keystroke rather than reading
//! anything out of the manifest.

use std::path::{Path, PathBuf};

/// A Rust project found on disk.
#[derive(Debug, Clone)]
pub struct Project {
    /// The package name, absent for a manifest that is only a workspace.
    pub name: Option<String>,
    /// Whether the crate produces something runnable.
    pub runnable: bool,
}

/// Finds the nearest `Cargo.toml`, searching `dir` and then its parents.
pub fn discover(dir: &Path) -> Option<(Project, PathBuf)> {
    for candidate in dir.ancestors() {
        let manifest = candidate.join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        return Some((read(&text, candidate), candidate.to_path_buf()));
    }
    None
}

/// Reads what little is needed out of a `Cargo.toml`.
///
/// Deliberately not a TOML parser. Two facts are wanted — the package name and
/// whether anything is runnable — and a dependency to learn them would cost
/// more than they are worth.
fn read(text: &str, root: &Path) -> Project {
    let mut name = None;
    let mut in_package = false;

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package && name.is_none() {
            if let Some(value) = line.strip_prefix("name") {
                name = value
                    .trim_start()
                    .strip_prefix('=')
                    .map(|value| value.trim().trim_matches(['"', '\'']).to_owned())
                    .filter(|value| !value.is_empty());
            }
        }
    }

    // `cargo run` needs a binary; a library-only crate has nothing to run.
    let runnable = root.join("src/main.rs").exists() || root.join("src/bin").is_dir();

    Project { name, runnable }
}

/// A command worth offering, and what it does.
pub struct Command {
    pub name: &'static str,
    pub args: &'static [&'static str],
    pub description: &'static str,
    /// Which group it belongs in, by the label `opi` already uses.
    pub group: &'static str,
}

/// The commands offered for a Rust project.
///
/// Taken from what these projects actually run in CI rather than from the
/// cargo book: `fmt --check`, `clippy` with warnings denied, `test` with all
/// features.
pub fn commands(project: &Project) -> Vec<&'static Command> {
    const ALL: &[Command] = &[
        Command {
            name: "run",
            args: &["run"],
            description: "Build and run the binary",
            group: "Development",
        },
        Command {
            name: "build",
            args: &["build", "--release"],
            description: "Build in release mode",
            group: "Build",
        },
        Command {
            name: "check",
            args: &["check", "--all-targets"],
            description: "Type-check without building",
            group: "Build",
        },
        Command {
            name: "test",
            args: &["test", "--all-features"],
            description: "Run the test suite",
            group: "Quality",
        },
        Command {
            name: "clippy",
            args: &[
                "clippy",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
            description: "Lint with warnings denied",
            group: "Quality",
        },
        Command {
            name: "fmt",
            args: &["fmt"],
            description: "Format the source",
            group: "Quality",
        },
        Command {
            name: "doc",
            args: &["doc", "--no-deps", "--open"],
            description: "Build and open the documentation",
            group: "Quality",
        },
    ];

    ALL.iter()
        .filter(|command| command.name != "run" || project.runnable)
        .collect()
}

/// The checks a Rust project answers, as `(name, tool args)`.
///
/// These are the same three every one of these repositories runs in CI.
pub const CHECKS: &[(&str, &[&str])] = &[
    ("Format", &["fmt", "--check"]),
    (
        "Lint",
        &[
            "clippy",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    ),
    ("Tests", &["test", "--all-features"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn project(toml: &str, root: &Path) -> Project {
        read(toml, root)
    }

    #[test]
    fn the_package_name_is_read() {
        let dir = tempfile::tempdir().expect("temp dir");
        let found = project(
            "[package]\nname = \"opi\"\nversion = \"0.1.0\"\n",
            dir.path(),
        );
        assert_eq!(found.name.as_deref(), Some("opi"));
    }

    #[test]
    fn a_name_outside_the_package_section_is_not_the_package_name() {
        // Dependency tables and bin sections have names too.
        let dir = tempfile::tempdir().expect("temp dir");
        let toml = "[workspace]\nmembers = [\"a\"]\n\n[[bin]]\nname = \"other\"\n";
        assert_eq!(project(toml, dir.path()).name, None);
    }

    #[test]
    fn a_workspace_with_a_package_still_has_its_name() {
        let dir = tempfile::tempdir().expect("temp dir");
        let toml = "[workspace]\nmembers = [\"a\"]\n\n[package]\nname = \"root\"\n";
        assert_eq!(project(toml, dir.path()).name.as_deref(), Some("root"));
    }

    #[test]
    fn a_library_has_nothing_to_run() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("src")).expect("mkdir");
        std::fs::write(dir.path().join("src/lib.rs"), "").expect("write");

        let found = project("[package]\nname = \"lib\"\n", dir.path());
        assert!(!found.runnable);
        assert!(
            !commands(&found).iter().any(|command| command.name == "run"),
            "offering `cargo run` for a library would fail on use"
        );
    }

    #[test]
    fn a_binary_is_runnable() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("src")).expect("mkdir");
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").expect("write");

        let found = project("[package]\nname = \"app\"\n", dir.path());
        assert!(found.runnable);
        assert!(commands(&found).iter().any(|command| command.name == "run"));
    }

    #[test]
    fn a_bin_directory_counts_as_runnable() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src/bin")).expect("mkdir");
        assert!(project("[package]\nname = \"x\"\n", dir.path()).runnable);
    }

    #[test]
    fn discovery_walks_up_like_cargo_does() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("write");
        let deep = dir.path().join("src").join("nested");
        std::fs::create_dir_all(&deep).expect("mkdir");

        let (found, root) = discover(&deep).expect("discover");
        assert_eq!(found.name.as_deref(), Some("x"));
        assert_eq!(
            root.canonicalize().expect("canonicalize"),
            dir.path().canonicalize().expect("canonicalize")
        );
    }

    #[test]
    fn no_manifest_is_simply_absent() {
        let dir = tempfile::tempdir().expect("temp dir");
        // A temp dir has no Cargo.toml above it either, on any sane machine.
        assert!(discover(dir.path()).is_none_or(|(_, root)| root != dir.path()));
    }
}
