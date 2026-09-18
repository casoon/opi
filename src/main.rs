//! `opi` — Operations Interface.
//!
//! A project control center for the terminal. `opi` lists a project's scripts
//! and runs the one you pick; `opi <script>` skips the list entirely.

#[cfg(not(unix))]
compile_error!(
    "opi is Unix-only: it runs a script by replacing its own process with exec, \
     and its interactive list drives termios directly."
);

mod cli;
mod manifest;
mod project;
mod run;
mod task;
mod workspace;

use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::ExitCode;

use runemark::{ColorMode, Console, ErrorBlock, Group, Item, Menu, Outcome, SelectMode, Tone};

use crate::cli::Invocation;
use crate::manifest::{Manifest, ManifestError};
use crate::project::Project;
use crate::task::{Task, by_group, find, suggestions};

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

    // The manifest may sit above the working directory; everything downstream
    // resolves against the directory it was actually found in.
    let (manifest, root) = match Manifest::discover(&directory) {
        Ok(found) => found,
        Err(error) => {
            report_error(&error);
            return ExitCode::FAILURE;
        }
    };

    let project = Project::detect(&manifest, &root);
    let members = workspace::members(&root, &manifest);
    let tasks = Task::from_workspace(&manifest, &members);

    match invocation {
        Invocation::List => list(&project, &tasks, &root),
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

/// Shows the project's scripts and runs whichever one is chosen.
fn list(project: &Project, tasks: &[Task], directory: &Path) -> ExitCode {
    let manager = project.package_manager;

    if tasks.is_empty() {
        let console = Console::stdout(ColorMode::Auto);
        println!(
            "{}  {}",
            console.paint(Tone::Title, project.display_name(directory)),
            console.paint(Tone::Muted, manager.manager)
        );
        println!(
            "{}",
            console.paint(Tone::Muted, "This project defines no scripts.")
        );
        return ExitCode::SUCCESS;
    }

    if !manager.is_certain() {
        let console = Console::stderr(ColorMode::Auto);
        eprintln!(
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

    let menu = build_menu(project, tasks, directory);

    // Both streams have to be terminals. stdout decides whether the list is
    // being captured rather than read, and the menu draws its frames on
    // stderr, so a redirect on either one means plain text is what is wanted.
    let interactive = io::stdout().is_terminal() && io::stderr().is_terminal();

    let outcome = match menu.run(
        Console::stderr(ColorMode::Auto),
        SelectMode::Auto,
        interactive,
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            let block = ErrorBlock::new("Cannot open the terminal")
                .with_explanation(format!("{error}"))
                .with_remedy("Run opi <script> to start a script without the list.");
            write_block(&block, Console::stderr(ColorMode::Auto));
            return ExitCode::FAILURE;
        }
    };

    match outcome {
        Outcome::Selected(id) => start(project, tasks, &id, &[]),
        // Nothing was chosen; that is not a failure.
        Outcome::Cancelled => ExitCode::SUCCESS,
        // No hints are registered until the maintenance areas exist.
        Outcome::Hotkey(_) => ExitCode::SUCCESS,
        Outcome::Unavailable => {
            print!("{}", menu.render(Console::stdout(ColorMode::Auto)));
            ExitCode::SUCCESS
        }
    }
}

/// Turns the task list into a menu.
///
/// The item id is the script name, so a selection is ready to run as-is.
fn build_menu(project: &Project, tasks: &[Task], directory: &Path) -> Menu {
    let mut menu = Menu::new()
        .with_heading(project.display_name(directory))
        .with_note(project.package_manager.manager.to_string());

    for (group, section) in by_group(tasks) {
        let mut rendered = Group::new(group.label());
        for task in section {
            // The id has to disambiguate: root and member scripts share names
            // in every workspace repository measured.
            let id = match &task.workspace {
                Some(member) => format!("{member}/{}", task.name),
                None => task.name.clone(),
            };
            let mut item = Item::new(id, &task.name);
            if let Some(description) = &task.description {
                item = item.with_description(description);
            }
            rendered = rendered.add_item(item);
        }
        menu = menu.add_group(rendered);
    }

    menu
}

/// Runs `name`, or explains why it cannot.
///
/// Returns only on failure: a started script replaces this process.
fn start(project: &Project, tasks: &[Task], name: &str, args: &[String]) -> ExitCode {
    let Some(task) = find(tasks, name) else {
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
    let error = run::execute(manager.manager, &task.name, task.workspace.as_deref(), args);

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

fn report_error(error: &ManifestError) {
    let console = Console::stderr(ColorMode::Auto);
    let block = match error {
        ManifestError::Missing { directory } => ErrorBlock::new("No package.json found")
            .with_explanation(format!(
                "Searched {} and every directory above it.",
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
