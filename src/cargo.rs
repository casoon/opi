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

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// A Rust project found on disk.
#[derive(Debug, Clone)]
pub struct Project {
    /// The package name, absent for a manifest that is only a workspace.
    pub name: Option<String>,
    /// Whether the crate produces something runnable.
    pub runnable: bool,
}

/// Finds the project `dir` belongs to, searching `dir` and then its parents.
///
/// Not simply the nearest `Cargo.toml`: in a workspace that would be the crate
/// the caller happens to stand in, and everything downstream would be scoped to
/// it. `target/` lives at the workspace root, so Clean would miss the largest
/// directory in the repository — the one reason anyone starts it — and the
/// checks would cover one crate instead of the workspace.
///
/// So the nearest manifest establishes that this is a Rust project at all, and
/// the search keeps rising for a manifest that declares a workspace. The
/// outermost one wins.
pub fn discover(dir: &Path) -> Option<(Project, PathBuf)> {
    let mut nearest: Option<(PathBuf, String)> = None;
    let mut workspace: Option<(PathBuf, String)> = None;

    for candidate in dir.ancestors() {
        let Ok(text) = std::fs::read_to_string(candidate.join("Cargo.toml")) else {
            continue;
        };
        if declares_workspace(&text) {
            workspace = Some((candidate.to_path_buf(), text.clone()));
        }
        nearest.get_or_insert_with(|| (candidate.to_path_buf(), text));
    }

    let (here, here_text) = nearest?;
    let (root, root_text) = workspace.unwrap_or((here.clone(), here_text));

    // The two arguments deliberately point at different places in a workspace:
    // the name belongs to the root, but `cargo run` runs wherever the caller
    // stands — `run::execute` sets no directory — so whether something is
    // runnable is a question about `here`.
    Some((read(&root_text, &here), root))
}

/// Whether this manifest is a workspace root.
///
/// A member declares `[package]` and nothing more; only the root carries a
/// `[workspace]` table. `[workspace.dependencies]` and `[workspace.package]`
/// are the same statement written as a sub-table, and real roots use them.
fn declares_workspace(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        line == "[workspace]" || line.starts_with("[workspace.")
    })
}

/// Reads what little is needed out of a `Cargo.toml`.
///
/// Deliberately not a TOML parser. Two facts are wanted — the package name and
/// whether anything is runnable — and a dependency to learn them would cost
/// more than they are worth.
///
/// `runs_in` is where `cargo run` would start, which is not always the
/// directory `text` came from; see `discover`.
fn read(text: &str, runs_in: &Path) -> Project {
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
    let runnable = runs_in.join("src/main.rs").exists() || runs_in.join("src/bin").is_dir();

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

/// Whether a cargo subcommand is installed.
///
/// A subcommand is an executable named `cargo-<name>` somewhere on `PATH`;
/// that is how cargo finds one, so it is how this looks. Running `cargo <name>`
/// and reading "no such command" out of its stderr would be a parser on an
/// undocumented format, which is the thing this project refuses everywhere
/// else.
///
/// `cargo audit` and `cargo outdated` are both installed this way, and neither
/// comes with the toolchain — an absent one is a check that does not exist
/// rather than one that failed.
pub fn has_subcommand(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| lists(&path, name))
}

/// The search itself, given the value of `PATH`.
///
/// Split out so a test can hand it a directory rather than change the
/// environment: `set_var` is unsafe in edition 2024 for good reason, and the
/// test suite runs in threads that would see the change.
fn lists(path: &OsStr, name: &str) -> bool {
    let file = format!("cargo-{name}");
    std::env::split_paths(path).any(|dir| dir.join(&file).is_file())
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

    /// A workspace root with one member crate at `crates/app`.
    ///
    /// The root is virtual — no `[package]` — which is the common shape and
    /// the one that used to leave the project unnamed.
    fn workspace(dir: &Path, root_toml: &str) -> PathBuf {
        std::fs::write(dir.join("Cargo.toml"), root_toml).expect("write");
        let member = dir.join("crates").join("app");
        std::fs::create_dir_all(&member).expect("mkdir");
        std::fs::write(member.join("Cargo.toml"), "[package]\nname = \"app\"\n").expect("write");
        member
    }

    #[test]
    fn a_crate_in_a_workspace_resolves_to_the_workspace_root() {
        // Otherwise Clean looks for target/ inside the crate, where it is not,
        // and the checks cover one crate instead of the workspace.
        let dir = tempfile::tempdir().expect("temp dir");
        let member = workspace(dir.path(), "[workspace]\nmembers = [\"crates/*\"]\n");

        let (_, root) = discover(&member).expect("discover");
        assert_eq!(
            root.canonicalize().expect("canonicalize"),
            dir.path().canonicalize().expect("canonicalize")
        );
    }

    #[test]
    fn a_parent_without_a_workspace_does_not_capture_the_crate() {
        // Two unrelated manifests stacked in a directory tree are not a
        // workspace, and the nearest one is still the answer.
        let dir = tempfile::tempdir().expect("temp dir");
        let member = workspace(dir.path(), "[package]\nname = \"outer\"\n");

        let (_, root) = discover(&member).expect("discover");
        assert_eq!(
            root.canonicalize().expect("canonicalize"),
            member.canonicalize().expect("canonicalize")
        );
    }

    #[test]
    fn the_outermost_workspace_wins() {
        let dir = tempfile::tempdir().expect("temp dir");
        let member = workspace(dir.path(), "[workspace]\nmembers = [\"crates/*\"]\n");
        // A member that declares a workspace of its own. Untested territory in
        // real repositories, but the rule has to be written down somewhere.
        std::fs::write(
            member.join("Cargo.toml"),
            "[workspace]\n\n[package]\nname = \"app\"\n",
        )
        .expect("write");

        let (_, root) = discover(&member).expect("discover");
        assert_eq!(
            root.canonicalize().expect("canonicalize"),
            dir.path().canonicalize().expect("canonicalize")
        );
    }

    #[test]
    fn the_workspace_root_names_the_project() {
        let dir = tempfile::tempdir().expect("temp dir");
        let member = workspace(
            dir.path(),
            "[workspace]\nmembers = [\"crates/*\"]\n\n[package]\nname = \"repo\"\n",
        );

        let (found, _) = discover(&member).expect("discover");
        assert_eq!(
            found.name.as_deref(),
            Some("repo"),
            "the crate stood in is not what the repository is called"
        );
    }

    #[test]
    fn a_virtual_workspace_root_has_no_name_to_give() {
        let dir = tempfile::tempdir().expect("temp dir");
        let member = workspace(dir.path(), "[workspace]\nmembers = [\"crates/*\"]\n");

        let (found, _) = discover(&member).expect("discover");
        assert_eq!(found.name, None, "falls back to the directory name instead");
    }

    #[test]
    fn a_binary_crate_in_a_workspace_keeps_its_run_entry() {
        // `cargo run` starts in the working directory — run::execute sets
        // none — so standing in a binary crate it still works, even though the
        // workspace root has nothing to run.
        let dir = tempfile::tempdir().expect("temp dir");
        let member = workspace(dir.path(), "[workspace]\nmembers = [\"crates/*\"]\n");
        std::fs::create_dir(member.join("src")).expect("mkdir");
        std::fs::write(member.join("src/main.rs"), "fn main() {}").expect("write");

        let (found, _) = discover(&member).expect("discover");
        assert!(found.runnable);
        assert!(commands(&found).iter().any(|command| command.name == "run"));
    }

    #[test]
    fn a_workspace_sub_table_is_still_a_workspace() {
        // Roots that inherit dependencies write `[workspace.dependencies]`,
        // and a few write no bare `[workspace]` line at all.
        assert!(declares_workspace(
            "[workspace.dependencies]\nserde = \"1\"\n"
        ));
        assert!(declares_workspace(
            "[package]\nname = \"x\"\n\n[workspace]\n"
        ));
        assert!(!declares_workspace("[package]\nname = \"x\"\n"));
    }

    #[test]
    fn a_cargo_subcommand_is_an_executable_named_for_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("cargo-pretend"), "").expect("write");
        let path = dir.path().as_os_str();

        assert!(lists(path, "pretend"));
        assert!(!lists(path, "audit"), "only what is actually there");
    }

    #[test]
    fn an_empty_path_finds_nothing() {
        assert!(!lists(OsStr::new(""), "audit"));
    }

    #[test]
    fn no_manifest_is_simply_absent() {
        let dir = tempfile::tempdir().expect("temp dir");
        // A temp dir has no Cargo.toml above it either, on any sane machine.
        assert!(discover(dir.path()).is_none_or(|(_, root)| root != dir.path()));
    }
}
