//! `opi` — Operations Interface.
//!
//! A project control center for the terminal. This build detects the project
//! and reports what it found; the script list is not implemented yet.

mod project;

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use runemark::{ColorMode, Console, ErrorBlock, Tone};

use crate::project::{DetectError, Project};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let directory = match std::env::current_dir() {
        Ok(directory) => directory,
        Err(error) => {
            let console = Console::stderr(ColorMode::Auto);
            eprintln!(
                "{}",
                console.paint(
                    Tone::Error,
                    format!("Cannot read the current directory: {error}")
                )
            );
            return ExitCode::FAILURE;
        }
    };

    match Project::detect(&directory) {
        Ok(project) => {
            report(&project, &directory);
            ExitCode::SUCCESS
        }
        Err(error) => {
            report_error(&error);
            ExitCode::FAILURE
        }
    }
}

fn report(project: &Project, directory: &Path) {
    let console = Console::stdout(ColorMode::Auto);
    let manager = project.package_manager;

    // The header of the eventual script screen: project on the left, the
    // package manager its scripts will be run with on the right.
    println!(
        "{}  {}",
        console.paint(Tone::Title, project.display_name(directory)),
        console.paint(Tone::Muted, manager.manager)
    );

    if !manager.is_certain() {
        println!(
            "{}",
            console.paint(
                Tone::Warning,
                format!(
                    "No lockfile and no packageManager field — assuming {}.",
                    manager.manager
                ),
            )
        );
    }

    if let Some(node) = &project.node_version {
        println!("{}", console.paint(Tone::Muted, format!("Node {node}")));
    }

    println!();
    println!(
        "{}",
        console.paint(
            Tone::Muted,
            format!("opi {VERSION} — the script list is not implemented yet."),
        )
    );
}

fn report_error(error: &DetectError) {
    let console = Console::stderr(ColorMode::Auto);
    let block = match error {
        DetectError::NoManifest { directory } => ErrorBlock::new("No package.json found")
            .with_explanation(format!(
                "{} does not contain a package.json, so there is no project to show.",
                directory.display()
            ))
            .with_remedy("Change into a project directory and run opi again."),
        DetectError::Unreadable { path, error } => ErrorBlock::new("package.json is unreadable")
            .with_explanation(format!("{}: {error}", path.display()))
            .with_remedy("Check the file permissions."),
        DetectError::Malformed { path, error } => ErrorBlock::new("package.json is not valid JSON")
            .with_explanation(format!("{}: {error}", path.display()))
            .with_remedy("Fix the syntax error and run opi again."),
    };

    let mut stderr = io::stderr().lock();
    let _ = block.write_to(console, &mut stderr);
    let _ = stderr.flush();
}
