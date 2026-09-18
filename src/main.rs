//! `opi` — Operations Interface.
//!
//! A project control center for the terminal. This build lists a project's
//! scripts; selecting and running them is not implemented yet.

mod manifest;
mod project;
mod task;

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use runemark::{ColorMode, Console, ErrorBlock, Tone};

use crate::manifest::{Manifest, ManifestError};
use crate::project::Project;
use crate::task::{Task, by_group};

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

    let manifest = match Manifest::load(&directory) {
        Ok(manifest) => manifest,
        Err(error) => {
            report_error(&error);
            return ExitCode::FAILURE;
        }
    };

    let project = Project::detect(&manifest, &directory);
    let tasks = Task::from_manifest(&manifest);
    report(&project, &tasks, &directory);
    ExitCode::SUCCESS
}

fn report(project: &Project, tasks: &[Task], directory: &Path) {
    let console = Console::stdout(ColorMode::Auto);
    let manager = project.package_manager;

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

    println!();

    if tasks.is_empty() {
        println!(
            "{}",
            console.paint(Tone::Muted, "This project defines no scripts.")
        );
        return;
    }

    // Names share one column width across all groups, so the descriptions line
    // up down the whole list rather than per section.
    let width = tasks.iter().map(|task| task.name.len()).max().unwrap_or(0);

    for (group, section) in by_group(tasks) {
        println!("{}", console.paint(Tone::Info, group.label()));
        for task in section {
            let name = format!("  {:width$}", task.name);
            match &task.description {
                Some(description) => println!(
                    "{}  {}",
                    console.paint(Tone::Success, name),
                    console.paint(Tone::Muted, description)
                ),
                None => println!("{}", console.paint(Tone::Success, name.trim_end())),
            }
        }
    }

    println!();
    println!(
        "{}",
        console.paint(
            Tone::Muted,
            format!("opi {VERSION} — selecting and running is not implemented yet."),
        )
    );
}

fn report_error(error: &ManifestError) {
    let console = Console::stderr(ColorMode::Auto);
    let block = match error {
        ManifestError::Missing { directory } => ErrorBlock::new("No package.json found")
            .with_explanation(format!(
                "{} does not contain a package.json, so there is no project to show.",
                directory.display()
            ))
            .with_remedy("Change into a project directory and run opi again."),
        ManifestError::Unreadable { path, error } => ErrorBlock::new("package.json is unreadable")
            .with_explanation(format!("{}: {error}", path.display()))
            .with_remedy("Check the file permissions."),
        ManifestError::Malformed { path, error } => {
            ErrorBlock::new("package.json is not valid JSON")
                .with_explanation(format!("{}: {error}", path.display()))
                .with_remedy("Fix the syntax error and run opi again.")
        }
    };

    let mut stderr = io::stderr().lock();
    let _ = block.write_to(console, &mut stderr);
    let _ = stderr.flush();
}
