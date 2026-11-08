//! `opi` — Operations Interface.
//!
//! A project control center for the terminal. `opi` lists a project's scripts
//! and runs the one you pick; `opi <script>` skips the list entirely.

#[cfg(not(unix))]
compile_error!(
    "opi is Unix-only: it runs a script by replacing its own process with exec, \
     and its interactive list drives termios directly."
);

mod actions;
mod audit;
mod cargo;
mod check;
mod check_config;
mod clean;
mod cli;
mod cmd;
mod completions;
mod manifest;
mod osv;
mod outdated;
mod project;
mod run;
mod sbom;
mod scripts;
mod secrets;
mod sources;
mod staged;
mod supply;
mod task;
mod workflow;
mod workspace;

use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::ExitCode;

use runemark::{
    ColorMode, Console, ErrorBlock, Group, Hint, Item, Layout, Menu, Outcome, SelectMode, Tone,
};

use crate::cli::Invocation;
use crate::manifest::{Manifest, ManifestError};
use crate::project::Project;
use crate::task::{Task, by_group, find, suggestions};

pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Lines of a failing tool's output health prints before pointing at the tool.
pub(crate) const OUTPUT_LINES: usize = 20;

/// Above either of these the list is shown as tabs rather than flat.
///
/// The flat list was decided against `astro-v7-template` — 24 entries in 8
/// groups, still readable. `web-casoon` has 27 in the root alone, and with a
/// heading per group that is 34 lines: taller than a full-screen terminal, so
/// the top scrolls away and the whole point of the screen is lost.
///
/// A threshold rather than the terminal's own height, which would have fitted
/// more exactly. The height changes while the menu is open, and a list that
/// rearranged itself mid-keystroke would move entries under a cursor already
/// on its way to one. This way `opi` looks the same in every window, and the
/// same as what a pipe prints.
const TABS_ABOVE_TASKS: usize = 15;
const TABS_ABOVE_GROUPS: usize = 5;

/// The one tab every workspace package shares.
const PACKAGES_TAB: &str = "Packages";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (invocation, json) = cli::parse_with_json(&args);

    match &invocation {
        Invocation::Help => {
            println!("{}", cli::help(VERSION));
            return ExitCode::SUCCESS;
        }
        Invocation::Version => {
            println!("opi {VERSION}");
            return ExitCode::SUCCESS;
        }
        Invocation::Completions(shell) => {
            return if let Some(script) = completions::script(shell) {
                print!("{script}");
                ExitCode::SUCCESS
            } else {
                let block = ErrorBlock::new(format!("No completions for {shell}"))
                    .with_remedy("Choose bash, zsh or fish.");
                write_block(&block, Console::stderr(ColorMode::Auto));
                ExitCode::FAILURE
            };
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

    // A repository may be more than one kind of project at once — twelve of
    // the ones measured carry both a package.json and a Cargo.toml — so both
    // are looked for and neither is allowed to win.
    let npm = Manifest::discover(&directory);
    let rust = cargo::discover(&directory);

    if let (Err(error), None) = (&npm, &rust) {
        report_error(error);
        return ExitCode::FAILURE;
    }

    // Kept before the match consumes it. An empty `Manifest` stands in for a
    // missing one below, and from there nothing can tell the two apart — but
    // the areas that would start a package manager have to.
    let has_npm = npm.is_ok();

    let (manifest, root) = match npm {
        Ok((manifest, root)) => (manifest, root),
        // A Rust-only project still needs somewhere to stand and something to
        // read; an empty manifest answers both without a special case below.
        Err(_) => (
            Manifest::default(),
            rust.as_ref()
                .map_or_else(|| directory.clone(), |(_, root)| root.clone()),
        ),
    };

    let project = Project::detect(&manifest, &root)
        .or_named(rust.as_ref().and_then(|(rust, _)| rust.name.clone()))
        .with_npm(has_npm);
    // Two different questions: the menu only runs what has a script, while
    // health, clean, security and the workflows look inside every member's
    // directory regardless of whether it has one.
    let scripted_members = workspace::members(&root, &manifest);
    let members = workspace::all(&root, &manifest);
    let mut tasks = Task::from_workspace(&manifest, &scripted_members);
    if let Some((rust, _)) = &rust {
        tasks.extend(task::from_cargo(rust));
    }
    task::arrange(&mut tasks);

    let rust_root = rust.as_ref().map(|(_, root)| root.clone());

    match invocation {
        Invocation::List => list(
            &manifest,
            &members,
            rust_root.as_deref(),
            &project,
            &tasks,
            &root,
        ),
        Invocation::Health => cmd::health::health(
            &manifest,
            &members,
            rust_root.as_deref(),
            &project,
            &root,
            json,
        ),
        Invocation::Clean => {
            cmd::clean::clean(&manifest, &members, rust_root.as_deref(), &project, &root)
        }
        Invocation::Security if json => {
            cmd::json::security(&manifest, &members, rust_root.as_deref(), &project, &root)
        }
        Invocation::Security => {
            cmd::security::security(&manifest, &members, rust_root.as_deref(), &project, &root)
        }
        Invocation::Updates if json => {
            cmd::json::updates(&manifest, &project, rust_root.as_deref(), &root)
        }
        Invocation::Updates => {
            cmd::updates::updates(&manifest, &members, &project, rust_root.as_deref(), &root)
        }
        Invocation::Workflow(name) => cmd::workflow::run_workflow(
            &name,
            &manifest,
            &members,
            rust_root.as_deref(),
            &project,
            &root,
            json,
        ),
        Invocation::Hooks => cmd::hooks::hooks(&root),
        Invocation::Checks(name) => cmd::checks::configure_checks(
            name.as_deref(),
            &manifest,
            &members,
            rust_root.as_deref(),
            &project,
            &root,
        ),
        Invocation::Sbom(out) => {
            cmd::sbom::sbom(&project, rust_root.as_deref(), &root, out.as_deref())
        }
        Invocation::Run {
            name,
            args,
            confirmed,
        } => start(&project, &tasks, &name, &args, confirmed),
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
        // Names only, for the completion scripts: no heading, no interface.
        Invocation::ListNames => {
            for task in tasks.iter().filter(|task| !task.hidden) {
                println!("{}", task.name);
            }
            ExitCode::SUCCESS
        }
        Invocation::Help | Invocation::Version | Invocation::Completions(_) => {
            unreachable!("handled above")
        }
    }
}

/// Shows the project's scripts and runs whichever one is chosen.
fn list(
    manifest: &Manifest,
    members: &[workspace::Member],
    rust_root: Option<&Path>,
    project: &Project,
    tasks: &[Task],
    directory: &Path,
) -> ExitCode {
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
        let others = task::other_runners(directory);
        if !others.is_empty() {
            println!(
                "{}",
                console.paint(
                    Tone::Muted,
                    format!("Its tasks may live in {}.", others.join(", "))
                )
            );
        }
        return ExitCode::SUCCESS;
    }

    // Only worth saying where something would actually be run with it: a
    // Rust-only project has no use for a package manager and no reason to hear
    // that one was guessed.
    let runs_scripts = tasks
        .iter()
        .any(|task| matches!(task.exec, task::Exec::Script));
    if !manager.is_certain() && runs_scripts {
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

    let mut menu = build_menu(project, tasks, directory);
    // Offered only where it would do something; a key that answers "nothing
    // applies" is worse than no key.
    let checks = check::Check::detect_all(
        manifest,
        members,
        rust_root,
        directory,
        project.package_manager.manager,
    );
    if !checks.is_empty() {
        menu = menu.add_hint(Hint::new('H', "Health"));
    }
    menu = menu
        .add_hint(Hint::new('C', "Clean"))
        .add_hint(Hint::new('S', "Security"))
        .add_hint(Hint::new('U', "Updates"));

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
        Outcome::Selected(id) => start(project, tasks, &id, &[], false),
        // Nothing was chosen; that is not a failure.
        Outcome::Cancelled => ExitCode::SUCCESS,
        Outcome::Hotkey('H') => {
            cmd::health::health(manifest, members, rust_root, project, directory, false)
        }
        Outcome::Hotkey('C') => cmd::clean::clean(manifest, members, rust_root, project, directory),
        Outcome::Hotkey('S') => {
            cmd::security::security(manifest, members, rust_root, project, directory)
        }
        Outcome::Hotkey('U') => {
            cmd::updates::updates(manifest, members, project, rust_root, directory)
        }
        Outcome::Hotkey(_) => ExitCode::SUCCESS,
        Outcome::Unavailable => {
            print!("{}", menu.render(Console::stdout(ColorMode::Auto)));
            ExitCode::SUCCESS
        }
    }
}

/// What the header says the project is built with.
///
/// A repository carrying both manifests says both. Naming only one would make
/// the other half of its list look like it arrived from nowhere.
fn toolchains(tasks: &[Task], project: &Project) -> String {
    let mut names = Vec::new();
    if tasks
        .iter()
        .any(|task| matches!(task.exec, task::Exec::Script))
    {
        names.push(project.package_manager.manager.to_string());
    }
    if tasks
        .iter()
        .any(|task| matches!(&task.exec, task::Exec::Direct { program, .. } if program == "cargo"))
    {
        names.push("cargo".to_owned());
    }
    if names.is_empty() {
        names.push(project.package_manager.manager.to_string());
    }
    names.join(" · ")
}

/// Whether the list is shown as tabs or flat.
fn layout(entries: usize, groups: usize) -> Layout {
    if entries > TABS_ABOVE_TASKS || groups > TABS_ABOVE_GROUPS {
        Layout::Tabs
    } else {
        Layout::Flat
    }
}

/// What the list adds up to, for the line under the heading.
///
/// The counts are what the list itself only says by being counted, and in the
/// tab layout two of the three are no longer on screen at once.
///
/// Packages are named only where there are some. A single-package project
/// saying "1 package" would answer a question nobody in it has. The same goes
/// for task runner files beside the manifest: named when present, so the list
/// does not pass for everything the project can do.
fn summary(sections: &[(&task::Group, &[Task])], entries: usize, others: &[String]) -> String {
    let mut parts = vec![
        plural(entries, "entry", "entries"),
        plural(sections.len(), "group", "groups"),
    ];

    let packages = sections
        .iter()
        .filter(|(group, _)| matches!(group, task::Group::Workspace(_)))
        .count();
    if packages > 0 {
        parts.push(plural(packages, "package", "packages"));
    }
    if !others.is_empty() {
        parts.push(format!("also {}", others.join(", ")));
    }

    parts.join(" · ")
}

pub(crate) fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// Turns the task list into a menu.
///
/// The item id is the script name, so a selection is ready to run as-is.
fn build_menu(project: &Project, tasks: &[Task], directory: &Path) -> Menu {
    let sections = by_group(tasks);
    let entries: usize = sections.iter().map(|(_, section)| section.len()).sum();

    let mut menu = Menu::new()
        .with_heading(project.display_name(directory))
        .with_note(toolchains(tasks, project))
        .with_summary(summary(&sections, entries, &task::other_runners(directory)))
        .with_layout(layout(entries, sections.len()));

    let mut packaged = false;
    for (group, section) in sections {
        let mut rendered = Group::new(group.label());
        // Every workspace package shares one tab, and the first of them marks
        // where the row stops naming actions and starts naming packages.
        //
        // One tab each was measured and does not scale: this repository's
        // largest workspace has 52 packages against 6 action groups, which is
        // a tab row of 58 that is almost entirely package names, permanently
        // scrolling, with the digits worthless past the ninth. Inside the tab
        // each package keeps its heading, so nothing is lost but the length.
        if matches!(group, task::Group::Workspace(_)) {
            rendered = rendered.in_tab(PACKAGES_TAB);
            if !packaged {
                rendered = rendered.with_divider();
                packaged = true;
            }
        }
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
fn start(
    project: &Project,
    tasks: &[Task],
    name: &str,
    args: &[String],
    confirmed: bool,
) -> ExitCode {
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

    if task.confirm && !confirmed && !confirm(task) {
        return ExitCode::FAILURE;
    }

    let manager = project.package_manager;
    let error = run::execute(task, manager.manager, args);

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

/// Asks before running a task the project marked as needing it.
///
/// Without a terminal this refuses rather than assuming yes. Skipping the
/// question where it cannot be asked would remove the protection in exactly
/// the case it exists for — a script, a hook, CI — so `--yes` has to be said
/// out loud there.
fn confirm(task: &Task) -> bool {
    let console = Console::stderr(ColorMode::Auto);
    let interactive = io::stdout().is_terminal() && io::stderr().is_terminal();

    if !interactive {
        let block = ErrorBlock::new(format!("{} needs confirming", task.name))
            .with_explanation("This project marked it as needing a confirmation, and there is no terminal to ask in.")
            .with_remedy("Run it again with --yes if that is what you mean.")
            .add_command(format!("opi --yes {}", task.name));
        write_block(&block, console);
        return false;
    }

    let menu = Menu::new()
        .with_heading(format!("Run {}?", task.name))
        .with_note(&task.command)
        .add_group(
            Group::new("Confirm")
                .add_item(Item::new("no", "Cancel"))
                .add_item(Item::new("yes", format!("Run {}", task.name))),
        );

    // Cancel first, so the cursor starts on the harmless answer.
    matches!(
        menu.run(console, SelectMode::Auto, true),
        Ok(Outcome::Selected(choice)) if choice == "yes"
    )
}

/// Writes an error block to stderr, ignoring a broken pipe.
pub(crate) fn write_block(block: &ErrorBlock, console: Console) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::Group as TaskGroup;

    fn task(name: &str, group: TaskGroup) -> Task {
        Task {
            name: name.to_owned(),
            description: None,
            command: name.to_owned(),
            group,
            exec: task::Exec::Script,
            confirm: false,
            workspace: None,
            hidden: false,
        }
    }

    fn sections(counts: &[(TaskGroup, usize)]) -> Vec<Task> {
        counts
            .iter()
            .flat_map(|(group, count)| {
                (0..*count).map(move |n| task(&format!("s{n}"), group.clone()))
            })
            .collect()
    }

    #[test]
    fn a_short_list_stays_flat() {
        // The 0.1.0 screen: everything visible at once, no row of tabs above
        // it saying what would fit anyway.
        assert_eq!(layout(12, 4), Layout::Flat);
        assert_eq!(layout(15, 5), Layout::Flat, "the thresholds are inclusive");
    }

    #[test]
    fn a_long_list_becomes_tabs() {
        // web-casoon: 27 entries in 7 groups, 34 lines with a heading each.
        assert_eq!(layout(27, 7), Layout::Tabs);
    }

    #[test]
    fn either_count_is_enough_on_its_own() {
        // Few entries spread very thin still costs a heading per group, and
        // many entries in two groups still scroll.
        assert_eq!(layout(8, 8), Layout::Tabs);
        assert_eq!(layout(30, 2), Layout::Tabs);
    }

    #[test]
    fn the_summary_counts_entries_and_groups() {
        let tasks = sections(&[
            (TaskGroup::Development, 4),
            (TaskGroup::Build, 7),
            (TaskGroup::Quality, 4),
        ]);
        let sections = by_group(&tasks);
        assert_eq!(summary(&sections, 15, &[]), "15 entries · 3 groups");
    }

    #[test]
    fn a_project_with_no_workspace_says_nothing_about_packages() {
        // "1 package" would answer a question nobody in a single-package
        // project has.
        let tasks = sections(&[(TaskGroup::Development, 1)]);
        let sections = by_group(&tasks);
        assert!(!summary(&sections, 1, &[]).contains("package"));
        assert_eq!(summary(&sections, 1, &[]), "1 entry · 1 group");
    }

    #[test]
    fn the_summary_counts_workspace_packages() {
        let tasks = sections(&[
            (TaskGroup::Development, 2),
            (TaskGroup::Workspace("@casoon/blog".to_owned()), 1),
            (TaskGroup::Workspace("@casoon/starter".to_owned()), 1),
        ]);
        let sections = by_group(&tasks);
        assert_eq!(
            summary(&sections, 4, &[]),
            "4 entries · 3 groups · 2 packages"
        );
    }

    #[test]
    fn the_summary_names_other_task_runners() {
        let tasks = sections(&[(TaskGroup::Development, 2)]);
        let sections = by_group(&tasks);
        assert_eq!(
            summary(&sections, 2, &["Makefile".to_owned()]),
            "2 entries · 1 group · also Makefile"
        );
    }
}
