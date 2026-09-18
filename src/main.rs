//! `opi` — Operations Interface.
//!
//! A project control center for the terminal. This build lists a project's
//! scripts and runs them by name; the interactive list is not implemented yet.

mod cli;
mod manifest;
mod project;
mod run;
mod task;

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use runemark::{ColorMode, Console, ErrorBlock, Tone};

use crate::cli::Invocation;
use crate::manifest::{Manifest, ManifestError};
use crate::project::Project;
use crate::task::{Task, by_group, suggestions};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let invocation = cli::parse(std::env::args().skip(1));

    match &invocation {
        Invocation::Help => {
            println!("{}", cli::help(VERSION));
            return ExitCode::SUCCESS;
        }
        Invocation::Version => {
            println!("opi {VERSION}");
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

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

    match invocation {
        Invocation::List => {
            report(&project, &tasks, &directory);
            ExitCode::SUCCESS
        }
        Invocation::Run { name, args } => start(&project, &tasks, &name, &args),
        // An unknown flag is only reported once a project is present, so the
        // missing-package.json message wins where both are true — that is the
        // problem the user has to fix first.
        Invocation::UnknownFlag(flag) => {
            let console = Console::stderr(ColorMode::Auto);
            let block = ErrorBlock::new(format!("Unknown option {flag}"))
                .with_remedy("Run opi --help to see the available options.");
            write_block(&block, console);
            ExitCode::FAILURE
        }
        Invocation::Help | Invocation::Version => unreachable!("handled above"),
    }
}

/// Runs `name`, or explains why it cannot.
///
/// Returns only on failure: a started script replaces this process.
fn start(project: &Project, tasks: &[Task], name: &str, args: &[String]) -> ExitCode {
    let Some(task) = tasks.iter().find(|task| task.name == name) else {
        let console = Console::stderr(ColorMode::Auto);
        let mut block = ErrorBlock::new(format!("No script named {name}"));

        let close = suggestions(tasks, name);
        block = if close.is_empty() {
            block.with_remedy("Run opi to see the project's scripts.")
        } else {
            block.with_explanation(format!("Did you mean {}?", close.join(", ")))
        };
        for candidate in close {
            block = block.add_command(format!("opi {candidate}"));
        }

        write_block(&block, console);
        return ExitCode::FAILURE;
    };

    let manager = project.package_manager;
    let error = run::execute(manager.manager, &task.name, args);

    let console = Console::stderr(ColorMode::Auto);
    let block = ErrorBlock::new(format!("Cannot run {}", manager.manager))
        .with_explanation(format!("{error}"))
        .with_remedy(if manager.is_certain() {
            format!(
                "Check that {} is installed and on your PATH.",
                manager.manager
            )
        } else {
            format!(
                "No lockfile or packageManager field names a package manager, so {} was assumed.",
                manager.manager
            )
        });
    write_block(&block, console);
    ExitCode::FAILURE
}

/// Writes an error block to stderr, ignoring a broken pipe.
fn write_block(block: &ErrorBlock, console: Console) {
    let mut stderr = io::stderr().lock();
    let _ = block.write_to(console, &mut stderr);
    let _ = stderr.flush();
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
            format!("opi {VERSION} — run a script with: opi <script>"),
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

    write_block(&block, console);
}
