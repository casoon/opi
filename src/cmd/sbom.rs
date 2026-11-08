//! Writes a software bill of materials for the project.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use runemark::{ColorMode, Console, ErrorBlock, Tone};

use crate::project::Project;
use crate::sbom;
use crate::write_block;

/// Writes a CycloneDX SBOM per ecosystem, into `out` or the project root.
///
/// A relative `out` is relative to where `opi` was started, not to the project
/// root. Each ecosystem is tried; the exit code fails if any of them did.
pub(crate) fn sbom(
    project: &Project,
    rust_root: Option<&Path>,
    root: &Path,
    out: Option<&str>,
) -> ExitCode {
    let console = Console::stdout(ColorMode::Auto);
    let out = match out {
        Some(dir) => PathBuf::from(dir),
        None => match sbom::default_dir(root) {
            Ok(dir) => dir,
            Err(error) => {
                let block =
                    ErrorBlock::new("No place for the SBOM").with_explanation(format!("{error:?}"));
                write_block(&block, Console::stderr(ColorMode::Auto));
                return ExitCode::FAILURE;
            }
        },
    };
    let mut results = Vec::new();
    if project.npm {
        results.push((
            "npm",
            sbom::npm(project.package_manager.manager, root, &out),
        ));
    }
    if let Some(rust_root) = rust_root {
        results.push(("Cargo", sbom::cargo(rust_root, &out)));
    }
    if results.is_empty() {
        let block = ErrorBlock::new("Nothing to describe")
            .with_remedy("An SBOM needs a package.json or a Cargo.toml.");
        write_block(&block, Console::stderr(ColorMode::Auto));
        return ExitCode::FAILURE;
    }
    let mut ok = true;
    for (ecosystem, result) in results {
        match result {
            Ok(path) => println!(
                "{} {}  {}",
                console.paint(Tone::Success, "✓"),
                ecosystem,
                console.paint(Tone::Muted, path.display().to_string())
            ),
            Err(error) => {
                ok = false;
                let block = match error {
                    sbom::SbomError::Missing { tool, install } => {
                        ErrorBlock::new(format!("{ecosystem}: {tool} is not installed"))
                            .with_remedy(format!("Install it with {install}."))
                    }
                    sbom::SbomError::Failed(reason) => {
                        ErrorBlock::new(format!("{ecosystem}: no SBOM written"))
                            .with_explanation(reason)
                    }
                };
                write_block(&block, Console::stderr(ColorMode::Auto));
            }
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
