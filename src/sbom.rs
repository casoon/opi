//! CycloneDX SBOMs, one file per ecosystem.
//!
//! Nothing here is computed by `opi`: each ecosystem's own producer writes the
//! document and `opi` places it. One file per ecosystem rather than a merged
//! document, because merging needs a tool of its own and a merged BOM no longer
//! says which producer vouches for which component.
//!
//! * npm, Yarn, Bun: `npm sbom`. With a `package-lock.json` it reads that alone
//!   (`--package-lock-only`), so the document describes what is locked, not what
//!   happens to be installed; without one it reads `node_modules`. Neither Yarn
//!   nor Bun ships a CycloneDX producer of its own.
//! * pnpm: `pnpm sbom`.
//! * Cargo: `cargo cyclonedx`, a separate install.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::project::PackageManager;

/// Where documents go unless told otherwise, below the project root.
const DEFAULT_DIR: &str = ".opi/sbom";
/// File name of the npm-family document.
pub const NPM_FILE: &str = "sbom-npm.cdx.json";
/// File name of the Cargo document.
pub const CARGO_FILE: &str = "sbom-cargo.cdx.json";

/// Why one ecosystem produced no document.
#[derive(Debug, PartialEq, Eq)]
pub enum SbomError {
    /// The producer is not installed; `install` says how to get it.
    Missing {
        tool: &'static str,
        install: &'static str,
    },
    /// The producer ran and failed.
    Failed(String),
}

/// The default output directory, created with a `.gitignore` of its own so the
/// documents never show up as untracked files.
pub fn default_dir(root: &Path) -> Result<PathBuf, SbomError> {
    let state = root.join(".opi");
    fs::create_dir_all(&state).map_err(|error| SbomError::Failed(error.to_string()))?;
    let ignore = state.join(".gitignore");
    if !ignore.exists() {
        fs::write(&ignore, "*\n").map_err(|error| SbomError::Failed(error.to_string()))?;
    }
    Ok(root.join(DEFAULT_DIR))
}

/// Writes the npm-family SBOM into `out` and returns its path.
pub fn npm(manager: PackageManager, root: &Path, out: &Path) -> Result<PathBuf, SbomError> {
    let locked = manager == PackageManager::Npm || root.join("package-lock.json").is_file();
    let (tool, args) = npm_command(manager, locked);
    let install = match manager {
        PackageManager::Pnpm => "npm install -g pnpm",
        _ => "npm install -g npm@latest",
    };
    let output = Command::new(tool)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| spawn_error(error, tool, install))?;
    if !output.status.success() {
        return Err(SbomError::Failed(first_line(&output.stderr, tool)));
    }
    write(out, NPM_FILE, &output.stdout)
}

/// Writes the Cargo SBOM into `out` and returns its path.
///
/// `cargo cyclonedx` writes beside the manifest, so the file is moved when
/// `out` is elsewhere. In a virtual workspace it writes one file per member,
/// each next to its own manifest, and none at `root`; that is reported.
pub fn cargo(root: &Path, out: &Path) -> Result<PathBuf, SbomError> {
    let output = Command::new("cargo")
        .args(cargo_args())
        .current_dir(root)
        .output()
        .map_err(|error| spawn_error(error, "cargo", "rustup"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Cargo itself answers an unknown subcommand with "no such command".
    if stderr.contains("no such command") {
        return Err(SbomError::Missing {
            tool: "cargo-cyclonedx",
            install: "cargo install cargo-cyclonedx",
        });
    }
    if !output.status.success() {
        return Err(SbomError::Failed(first_line(
            &output.stderr,
            "cargo cyclonedx",
        )));
    }
    let written = root.join(CARGO_FILE);
    if !written.is_file() {
        return Err(SbomError::Failed(
            "cargo cyclonedx wrote no file at the root; a virtual workspace gets one per member, next to its manifest".to_owned(),
        ));
    }
    let target = out.join(CARGO_FILE);
    if written != target {
        fs::create_dir_all(out).map_err(|error| SbomError::Failed(error.to_string()))?;
        fs::copy(&written, &target).map_err(|error| SbomError::Failed(error.to_string()))?;
        fs::remove_file(&written).map_err(|error| SbomError::Failed(error.to_string()))?;
    }
    Ok(target)
}

/// The producer and arguments for an npm-family project.
fn npm_command(manager: PackageManager, locked: bool) -> (&'static str, Vec<&'static str>) {
    let mut args = vec!["sbom", "--sbom-format", "cyclonedx"];
    if manager == PackageManager::Pnpm {
        return ("pnpm", args);
    }
    if locked {
        args.push("--package-lock-only");
    }
    ("npm", args)
}

/// `--override-filename` takes the name without its extension.
fn cargo_args() -> [&'static str; 5] {
    [
        "cyclonedx",
        "--format",
        "json",
        "--override-filename",
        "sbom-cargo.cdx",
    ]
}

fn write(out: &Path, name: &str, contents: &[u8]) -> Result<PathBuf, SbomError> {
    fs::create_dir_all(out).map_err(|error| SbomError::Failed(error.to_string()))?;
    let path = out.join(name);
    fs::write(&path, contents).map_err(|error| SbomError::Failed(error.to_string()))?;
    Ok(path)
}

fn spawn_error(error: io::Error, tool: &'static str, install: &'static str) -> SbomError {
    if error.kind() == io::ErrorKind::NotFound {
        SbomError::Missing { tool, install }
    } else {
        SbomError::Failed(format!("could not run {tool}: {error}"))
    }
}

fn first_line(stderr: &[u8], tool: &str) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .find(|line| !line.trim().is_empty())
        .map_or_else(|| format!("{tool} failed"), |line| line.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pnpm_asks_pnpm_and_the_rest_ask_npm() {
        assert_eq!(npm_command(PackageManager::Pnpm, true).0, "pnpm");
        for manager in [
            PackageManager::Npm,
            PackageManager::Yarn,
            PackageManager::Bun,
        ] {
            assert_eq!(npm_command(manager, false).0, "npm");
        }
    }

    #[test]
    fn the_npm_command_asks_for_cyclonedx() {
        assert_eq!(
            npm_command(PackageManager::Npm, false).1,
            ["sbom", "--sbom-format", "cyclonedx"]
        );
    }

    #[test]
    fn a_lockfile_is_read_alone() {
        assert!(
            npm_command(PackageManager::Npm, true)
                .1
                .contains(&"--package-lock-only")
        );
        assert!(
            !npm_command(PackageManager::Pnpm, true)
                .1
                .contains(&"--package-lock-only")
        );
    }

    #[test]
    fn the_default_directory_ignores_itself() {
        let root = std::env::temp_dir().join(format!("opi-sbom-{}", std::process::id()));
        let dir = default_dir(&root).unwrap();
        assert_eq!(dir, root.join(".opi/sbom"));
        assert_eq!(
            fs::read_to_string(root.join(".opi/.gitignore")).unwrap(),
            "*\n"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_cargo_filename_matches_the_expected_file() {
        let args = cargo_args();
        assert_eq!(format!("{}.json", args[4]), CARGO_FILE);
    }

    #[test]
    fn a_missing_binary_names_what_to_install() {
        let error = io::Error::from(io::ErrorKind::NotFound);
        assert_eq!(
            spawn_error(error, "pnpm", "npm install -g pnpm"),
            SbomError::Missing {
                tool: "pnpm",
                install: "npm install -g pnpm"
            }
        );
    }

    #[test]
    fn the_first_stderr_line_is_the_reason() {
        assert_eq!(
            first_line(b"\n npm error missing lockfile\nmore", "npm"),
            "npm error missing lockfile"
        );
        assert_eq!(first_line(b"", "npm"), "npm failed");
    }

    #[test]
    fn a_document_is_written_into_a_new_directory() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("sbom");
        let path = write(&out, NPM_FILE, b"{}").unwrap();
        assert_eq!(fs::read(path).unwrap(), b"{}");
    }
}
