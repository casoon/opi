//! `opi` — Operations Interface.
//!
//! A project control center for the terminal. `opi` lists a project's scripts
//! and runs the one you pick; `opi <script>` skips the list entirely.

#[cfg(not(unix))]
compile_error!(
    "opi is Unix-only: it runs a script by replacing its own process with exec, \
     and its interactive list drives termios directly."
);

mod audit;
mod cargo;
mod check;
mod clean;
mod cli;
mod manifest;
mod outdated;
mod project;
mod run;
mod task;
mod workflow;
mod workspace;

use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::ExitCode;

use runemark::{
    ColorMode, Console, DetailLevel, ErrorBlock, Finding, FindingGroup, Group, Hint, Item, Layout,
    Menu, Metric, NextStep, Outcome, Picked, Report, SelectMode, Tone, Verdict,
};

use crate::cli::Invocation;
use crate::manifest::{Manifest, ManifestError};
use crate::project::Project;
use crate::task::{Task, by_group, find, suggestions};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Lines of a failing tool's output health prints before pointing at the tool.
const OUTPUT_LINES: usize = 20;

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
        Invocation::Health => health(&manifest, &members, rust_root.as_deref(), &project, &root),
        Invocation::Clean => clean(&manifest, &members, rust_root.as_deref(), &project, &root),
        Invocation::Security => {
            security(&manifest, &members, rust_root.as_deref(), &project, &root)
        }
        Invocation::Updates => updates(&manifest, &members, &project, rust_root.as_deref(), &root),
        Invocation::Workflow(name) => run_workflow(
            &name,
            &manifest,
            &members,
            rust_root.as_deref(),
            &project,
            &root,
        ),
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
        Invocation::Help | Invocation::Version => unreachable!("handled above"),
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
        Outcome::Hotkey('H') => health(manifest, members, rust_root, project, directory),
        Outcome::Hotkey('C') => clean(manifest, members, rust_root, project, directory),
        Outcome::Hotkey('S') => security(manifest, members, rust_root, project, directory),
        Outcome::Hotkey('U') => updates(manifest, members, project, rust_root, directory),
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
/// saying "1 package" would answer a question nobody in it has.
fn summary(sections: &[(&task::Group, &[Task])], entries: usize) -> String {
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

    parts.join(" · ")
}

fn plural(count: usize, one: &str, many: &str) -> String {
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
        .with_summary(summary(&sections, entries))
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

/// Runs the project's checks and reports what each tool said.
fn health(
    manifest: &Manifest,
    members: &[workspace::Member],
    rust_root: Option<&Path>,
    project: &Project,
    root: &Path,
) -> ExitCode {
    let console = Console::stdout(ColorMode::Auto);
    let checks = check::Check::detect_all(
        manifest,
        members,
        rust_root,
        root,
        project.package_manager.manager,
    );

    println!(
        "{}  {}",
        console.paint(Tone::Title, project.display_name(root)),
        console.paint(Tone::Muted, "health")
    );
    println!();

    if checks.is_empty() {
        println!(
            "{}",
            console.paint(
                Tone::Muted,
                "No checks apply: this project depends on none of the tools opi knows."
            )
        );
        return ExitCode::SUCCESS;
    }

    // Results print as they arrive rather than after the slowest one, so a
    // long test run does not look like a hang.
    let width = checks
        .iter()
        .map(|check| check.label().chars().count())
        .max()
        .unwrap_or(0);
    let reports = check::run_all(checks, |report| {
        let (tone, mark) = match report {
            report if report.passed() => (Tone::Success, "✓"),
            report if report.unusable() => (Tone::Warning, "!"),
            _ => (Tone::Error, "✗"),
        };
        println!(
            "{} {}  {}  {}",
            console.paint(tone, mark),
            console.paint(tone, format!("{:width$}", report.check.label())),
            console.paint(
                Tone::Muted,
                format!("{:>6.1}s", report.duration.as_secs_f64())
            ),
            console.paint(Tone::Muted, report.check.tool),
        );
    });

    let failed: Vec<&check::Report> = reports.iter().filter(|report| !report.passed()).collect();
    let unusable = failed.iter().filter(|report| report.unusable()).count();

    // No score. A composite number stops meaning anything within weeks; what a
    // failing tool actually said does not.
    for report in &failed {
        println!();
        let tone = if report.unusable() {
            Tone::Warning
        } else {
            Tone::Error
        };
        println!(
            "{}",
            console.paint(
                tone,
                format!("{} — {}", report.check.label(), report.check.tool)
            )
        );
        // Capped: one failing scanner produced 49 lines on a real project, and
        // several at once bury the summary that says what to do next.
        let lines: Vec<&str> = report.output.lines().collect();
        for line in lines.iter().take(OUTPUT_LINES) {
            println!("  {line}");
        }
        if let Some(hidden) = lines.len().checked_sub(OUTPUT_LINES).filter(|n| *n > 0) {
            println!(
                "  {}",
                console.paint(
                    Tone::Muted,
                    format!(
                        "… {hidden} more lines — run `{}` in {} to see them all",
                        report.check.command_line(),
                        report.check.scope.as_deref().unwrap_or("the project root"),
                    )
                )
            );
        }
    }

    println!();
    if failed.is_empty() {
        println!(
            "{}",
            console.paint(Tone::Success, format!("{} checks passed.", reports.len()))
        );
        ExitCode::SUCCESS
    } else {
        let note = if unusable > 0 {
            format!(
                "{} of {} checks failed, {unusable} could not run.",
                failed.len(),
                reports.len()
            )
        } else {
            format!("{} of {} checks failed.", failed.len(), reports.len())
        };
        println!("{}", console.paint(Tone::Error, note));
        ExitCode::FAILURE
    }
}

/// Shows what can be removed, and removes what is chosen.
fn clean(
    manifest: &Manifest,
    members: &[workspace::Member],
    rust_root: Option<&Path>,
    project: &Project,
    root: &Path,
) -> ExitCode {
    let console = Console::stdout(ColorMode::Auto);
    let candidates = clean::candidates(manifest, members, rust_root, root);

    println!(
        "{}  {}",
        console.paint(Tone::Title, project.display_name(root)),
        console.paint(Tone::Muted, "clean")
    );
    println!();

    if candidates.is_empty() {
        println!(
            "{}",
            console.paint(Tone::Muted, "Nothing to remove; the project is clean.")
        );
        return ExitCode::SUCCESS;
    }

    let width = candidates
        .iter()
        .map(|candidate| candidate.display.chars().count())
        .max()
        .unwrap_or(0);
    for candidate in &candidates {
        println!(
            "  {}  {}",
            console.paint(
                if candidate.heavy {
                    Tone::Warning
                } else {
                    Tone::Info
                },
                format!("{:width$}", candidate.display)
            ),
            console.paint(Tone::Muted, clean::human(candidate.bytes)),
        );
    }
    println!();

    let artefacts: Vec<&clean::Candidate> =
        candidates.iter().filter(|entry| !entry.heavy).collect();
    let everything: Vec<&clean::Candidate> = candidates.iter().collect();
    let artefact_bytes: u64 = artefacts.iter().map(|entry| entry.bytes).sum();
    let total_bytes: u64 = everything.iter().map(|entry| entry.bytes).sum();

    // Concrete choices rather than per-entry ticking. The distinction that
    // matters is node_modules against the rest: it is the largest item and the
    // most expensive to rebuild, so it never rides along with a build artefact.
    let mut menu = Menu::new().add_group({
        let mut group = Group::new("Remove");
        if !artefacts.is_empty() {
            group = group.add_item(
                Item::new("artefacts", "Build artefacts")
                    .with_description(clean::human(artefact_bytes)),
            );
        }
        if artefacts.len() != everything.len() {
            group = group.add_item(
                Item::new("everything", "Everything, including node_modules")
                    .with_description(clean::human(total_bytes)),
            );
        }
        group.add_item(Item::new("cancel", "Cancel"))
    });
    menu = menu.with_note(format!("{} removable", clean::human(total_bytes)));

    let interactive = io::stdout().is_terminal() && io::stderr().is_terminal();
    let chosen = match menu.run(
        Console::stderr(ColorMode::Auto),
        SelectMode::Auto,
        interactive,
    ) {
        Ok(Outcome::Selected(id)) => id,
        Ok(Outcome::Unavailable) => {
            // Nothing is removed without someone choosing it, so a pipe gets
            // the inventory and stops there.
            println!(
                "{}",
                console.paint(
                    Tone::Muted,
                    "Run opi --clean in a terminal to remove any of it."
                )
            );
            return ExitCode::SUCCESS;
        }
        Ok(_) => return ExitCode::SUCCESS,
        Err(error) => {
            let block =
                ErrorBlock::new("Cannot open the terminal").with_explanation(format!("{error}"));
            write_block(&block, Console::stderr(ColorMode::Auto));
            return ExitCode::FAILURE;
        }
    };

    let selected: &[&clean::Candidate] = match chosen.as_str() {
        "artefacts" => &artefacts,
        "everything" => &everything,
        _ => return ExitCode::SUCCESS,
    };

    let started = std::time::Instant::now();
    let (freed, failures) = clean::remove(selected);

    println!(
        "{}",
        console.paint(
            Tone::Success,
            format!(
                "Removed {} in {:.1}s",
                clean::human(freed),
                started.elapsed().as_secs_f64()
            )
        )
    );
    for (path, error) in &failures {
        println!(
            "{}",
            console.paint(Tone::Error, format!("Could not remove {path}: {error}"))
        );
    }

    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Scans the repository for secrets and its dependencies for vulnerabilities.
fn security(
    manifest: &Manifest,
    members: &[workspace::Member],
    rust_root: Option<&Path>,
    project: &Project,
    root: &Path,
) -> ExitCode {
    let console = Console::stdout(ColorMode::Auto);
    println!(
        "{}  {}",
        console.paint(Tone::Title, project.display_name(root)),
        console.paint(Tone::Muted, "security")
    );
    println!();

    let mut clean = true;

    // Secrets is a check like any other; this only makes it reachable without
    // waiting for the tests to finish.
    // Secrets scanning is about the repository, not a toolchain, so the Rust
    // side contributes nothing here.
    let scans: Vec<check::Check> = check::Check::detect_all(
        manifest,
        members,
        None,
        root,
        project.package_manager.manager,
    )
    .into_iter()
    .filter(|check| check.name == "Secrets")
    .collect();

    if scans.is_empty() {
        println!(
            "{}",
            console.paint(
                Tone::Muted,
                "No secret scanner: this project depends on none that opi knows."
            )
        );
    } else {
        let reports = check::run_all(scans, |_| {});
        for report in &reports {
            if report.passed() {
                println!(
                    "{} {}",
                    console.paint(Tone::Success, "✓"),
                    console.paint(Tone::Success, format!("Secrets — {}", report.check.tool))
                );
            } else {
                clean = false;
                println!(
                    "{} {}",
                    console.paint(Tone::Error, "✗"),
                    console.paint(Tone::Error, format!("Secrets — {}", report.check.tool))
                );
                // Locations, not values: this output lands in scrollback, CI
                // logs and screenshots.
                let lines: Vec<&str> = report.output.lines().collect();
                for line in lines.iter().take(OUTPUT_LINES) {
                    println!("  {line}");
                }
                if let Some(rest) = lines.len().checked_sub(OUTPUT_LINES).filter(|n| *n > 0) {
                    println!(
                        "  {}",
                        console.paint(
                            Tone::Muted,
                            format!(
                                "… {rest} more lines — run `{}`",
                                report.check.command_line()
                            )
                        )
                    );
                }
            }
        }
    }

    // One section per ecosystem present, each named for itself. Health settles
    // the same question with a scope — `Tests (rust)` — and this follows it:
    // npm's section keeps the plain name, Rust's is marked.
    //
    // `project.package_manager` always holds a value, because the absence of
    // every signal still produces the npm fallback, so a Rust-only project
    // would otherwise run `npm audit` where there is no package.json and relay
    // npm's complaint about it as the result.
    if project.npm {
        println!();
        match audit::run(project.package_manager.manager, root) {
            Ok(found) if found.is_empty() => println!(
                "{} {}",
                console.paint(Tone::Success, "✓"),
                console.paint(Tone::Success, "Dependencies — no known vulnerabilities")
            ),
            Ok(found) => {
                let serious = found
                    .advisories
                    .iter()
                    .any(|advisory| advisory.severity.serious());
                clean = clean && !serious;

                // Parsed rather than relayed, so this is where a report earns its
                // keep: severity counts as metrics, each advisory with the version
                // range that fixes it as its remedy.
                let mut report = Report::new(
                    "Dependencies",
                    if serious {
                        Verdict::Failed
                    } else {
                        Verdict::Warning
                    },
                )
                .with_detail_level(DetailLevel::Detailed);

                // A verdict rather than a tone: this is read in pipes and in CI,
                // where a tone is nothing at all.
                for (severity, count) in found.counts() {
                    report = report.add_metric(
                        Metric::new(severity.label(), count.to_string()).with_verdict(
                            if severity.serious() {
                                Verdict::Failed
                            } else {
                                Verdict::Warning
                            },
                        ),
                    );
                }

                let mut group = FindingGroup::new("Vulnerable dependencies");
                for advisory in &found.advisories {
                    let mut finding = Finding::new(
                        if advisory.severity.serious() {
                            Tone::Error
                        } else {
                            Tone::Warning
                        },
                        &advisory.module,
                    )
                    .with_rule_id(advisory.severity.label());
                    if let Some(patched) = &advisory.patched {
                        finding = finding.with_remedy(format!("update to {patched}"));
                    }
                    group = group.add_finding(finding);
                }
                report = report.add_group(group);

                print!("{}", report.render(console));
            }
            Err(error) => {
                clean &= !audit_failed(&error);
                println!(
                    "{} {}",
                    console.paint(Tone::Muted, "–"),
                    console.paint(Tone::Muted, format!("Dependencies — {error}"))
                );
            }
        }
    }

    if let Some(rust_root) = rust_root {
        println!();
        match audit::cargo(rust_root) {
            Ok(found) if found.is_empty() => println!(
                "{} {}",
                console.paint(Tone::Success, "✓"),
                console.paint(
                    Tone::Success,
                    "Dependencies (rust) — no known vulnerabilities"
                )
            ),
            Ok(found) => {
                // Every advisory counts, because there is no severity here to
                // sort them by — see `audit::CargoAdvisory` — and because
                // `cargo audit` fails on any of them itself.
                clean = false;

                let mut report = Report::new("Dependencies (rust)", Verdict::Failed)
                    .with_detail_level(DetailLevel::Detailed)
                    .add_metric(
                        Metric::new("advisories", found.len().to_string())
                            .with_verdict(Verdict::Failed),
                    );

                let mut group = FindingGroup::new("Vulnerable crates");
                for advisory in &found {
                    let mut finding = Finding::new(
                        Tone::Error,
                        format!("{} {}", advisory.package, advisory.version),
                    )
                    .with_rule_id(&advisory.id);
                    if let Some(patched) = &advisory.patched {
                        finding = finding.with_remedy(format!("update to {patched}"));
                    }
                    group = group.add_finding(finding);
                }
                report = report.add_group(group);

                // The severity is missing from the JSON but present in the
                // tool's own output, so this is where to send someone who
                // wants it — along with the dependency path that pulled the
                // crate in.
                report = report.add_next_step(
                    NextStep::new("Severities and dependency paths").with_command("cargo audit"),
                );

                print!("{}", report.render(console));
            }
            Err(error) => {
                clean &= !audit_failed(&error);
                println!(
                    "{} {}",
                    console.paint(Tone::Muted, "–"),
                    console.paint(Tone::Muted, format!("Dependencies (rust) — {error}"))
                );
            }
        }
    }

    println!();
    if clean {
        println!("{}", console.paint(Tone::Success, "Nothing to act on."));
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Whether an audit error leaves the question unanswered rather than
/// answered "not here".
///
/// An audit that ran and failed has said nothing about the dependencies, so
/// "Nothing to act on." would be a false acquittal. `cargo audit` not being
/// installed is the ordinary case and is named with its remedy instead — the
/// same split `--updates` makes.
fn audit_failed(error: &audit::AuditError) -> bool {
    matches!(error, audit::AuditError::Failed(_))
}

/// Runs a named workflow: the repository questions, then the checks.
fn run_workflow(
    name: &str,
    manifest: &Manifest,
    members: &[workspace::Member],
    rust_root: Option<&Path>,
    project: &Project,
    root: &Path,
) -> ExitCode {
    let console = Console::stdout(ColorMode::Auto);

    let Some(workflow) = workflow::Workflow::parse(name) else {
        let block = ErrorBlock::new(format!("No workflow named {name}"))
            .with_remedy("Known workflows: commit, release.");
        write_block(&block, Console::stderr(ColorMode::Auto));
        return ExitCode::FAILURE;
    };

    println!(
        "{}  {}",
        console.paint(Tone::Title, project.display_name(root)),
        console.paint(Tone::Muted, workflow.label())
    );
    println!();

    let mut blocked = false;

    // The repository questions come first: a release from a dirty tree is
    // settled before spending a minute on its tests.
    for gate in workflow.gates() {
        let result = gate.check(root, manifest.version.as_deref());
        let (tone, mark) = match result.verdict {
            runemark::Verdict::Passed => (Tone::Success, "✓"),
            runemark::Verdict::Skipped => (Tone::Muted, "–"),
            _ => {
                blocked = true;
                (Tone::Error, "✗")
            }
        };
        let detail = result
            .detail
            .map_or_else(String::new, |detail| format!("  {detail}"));
        println!(
            "{} {}{}",
            console.paint(tone, mark),
            console.paint(tone, gate.name()),
            console.paint(Tone::Muted, detail)
        );
    }

    let checks: Vec<check::Check> = check::Check::detect_all(
        manifest,
        members,
        rust_root,
        root,
        project.package_manager.manager,
    )
    .into_iter()
    .filter(|check| workflow.includes(check.name))
    .collect();

    if checks.is_empty() {
        println!(
            "{}",
            console.paint(Tone::Muted, "No checks apply to this project.")
        );
    }

    let width = checks
        .iter()
        .map(|check| check.label().chars().count())
        .max()
        .unwrap_or(0);
    let reports = check::run_all(checks, |report| {
        let (tone, mark) = match report {
            report if report.passed() => (Tone::Success, "✓"),
            report if report.unusable() => (Tone::Warning, "!"),
            _ => (Tone::Error, "✗"),
        };
        println!(
            "{} {}  {}",
            console.paint(tone, mark),
            console.paint(tone, format!("{:width$}", report.check.label())),
            console.paint(Tone::Muted, report.check.tool),
        );
    });

    let failed = reports.iter().filter(|report| !report.passed()).count();
    println!();

    if blocked || failed > 0 {
        // Which step and why, rather than a count on its own.
        for report in reports.iter().filter(|report| !report.passed()) {
            println!(
                "{}",
                console.paint(
                    Tone::Error,
                    format!("{} — {}", report.check.label(), report.check.tool)
                )
            );
            for line in report.output.lines().take(OUTPUT_LINES) {
                println!("  {line}");
            }
        }
        println!();
        println!(
            "{}",
            console.paint(Tone::Error, format!("Not ready to {name}."))
        );
        ExitCode::FAILURE
    } else {
        println!(
            "{}",
            console.paint(Tone::Success, format!("Ready to {name}."))
        );
        ExitCode::SUCCESS
    }
}

/// Shows which dependencies have moved on, separated by how far.
///
/// Rendered as a runemark `Report` rather than a hand-set table: the split
/// between safe and breaking is the whole value of this screen, and a report's
/// groups make it structural instead of a sentence underneath a list.
fn updates(
    manifest: &Manifest,
    members: &[workspace::Member],
    project: &Project,
    rust_root: Option<&Path>,
    root: &Path,
) -> ExitCode {
    let console = Console::stdout(ColorMode::Auto);
    let manager = project.package_manager.manager;

    println!(
        "{}  {}",
        console.paint(Tone::Title, project.display_name(root)),
        console.paint(Tone::Muted, "updates")
    );
    println!();

    // As in `security`: without a package.json the package manager here is a
    // fallback, not a detection, and asking it would be a guess dressed up as
    // a result.
    let workspace = workspace::declared(root, manifest);
    let listed = if project.npm {
        outdated::run(manager, root, workspace)
    } else {
        Err(outdated::OutdatedError::NoManifest)
    };
    // Kept for the menu below, which needs the names rather than the rendering.
    let npm_updates = match &listed {
        Ok(found) => found.clone(),
        Err(_) => Vec::new(),
    };
    let mut failed = section(console, "Dependencies", listed);

    // Two sections rather than one merged list, the way `--security` already
    // splits them. Safe against breaking is this area's ordering principle and
    // it holds inside an ecosystem; across two it would file `tokio` beside
    // `vite` under "Safe to take" with only the name saying which is which,
    // and "Take the safe ones" could name only one of the two commands that
    // would do it.
    if let Some(rust_root) = rust_root {
        println!();
        failed |= section(console, "Dependencies (rust)", outdated::cargo(rust_root));
    }

    if failed {
        return ExitCode::FAILURE;
    }

    apply_updates(manifest, members, project, rust_root, root, &npm_updates)
}

/// Offers to take the updates, and runs the checks on what comes back.
///
/// Only npm's side is applied. `cargo update` writes the lockfile, which is
/// the line this area does not cross.
fn apply_updates(
    manifest: &Manifest,
    members: &[workspace::Member],
    project: &Project,
    rust_root: Option<&Path>,
    root: &Path,
    found: &[outdated::Update],
) -> ExitCode {
    let console = Console::stdout(ColorMode::Auto);
    let manager = project.package_manager.manager;
    let workspace = workspace::declared(root, manifest);

    let in_range = found.iter().filter(|update| update.in_range).count();
    let safe: Vec<String> = found
        .iter()
        .filter(|update| !update.jump.breaking())
        .map(|update| update.name.clone())
        .collect();
    let raises = outdated::can_raise(manager) && !safe.is_empty();
    let decides = outdated::can_raise(manager) && !found.is_empty();
    // Offered without a finding to justify it: `opi` could not read bun's
    // list, so it cannot know whether anything is outdated — bun's own list
    // can, and says so when nothing is.
    let chooses = project.npm && outdated::can_choose(manager);

    if in_range == 0 && !raises && !decides && !chooses {
        return ExitCode::SUCCESS;
    }

    // Writing is where a wrong detection stops being an annoyance. `pnpm
    // update` in a repository that actually runs npm leaves a pnpm-lock.yaml,
    // and the next `npm ci` resolves against something nobody saw. The listing
    // above keeps guessing silently; this does not.
    if let Some(conflict) = project::conflict(root, project.package_manager) {
        println!();
        println!(
            "{}",
            console.paint(
                Tone::Muted,
                format!(
                    "Not offering to apply these: {} both present, and no packageManager field says which. Adding one answers it.",
                    conflict.lockfiles.join(" and ")
                )
            )
        );
        return ExitCode::SUCCESS;
    }

    println!();
    let mut group = Group::new("Update");
    if in_range > 0 {
        group = group.add_item(
            Item::new("in-range", "Within the declared ranges")
                .with_description(format!("{in_range} packages, no package.json touched")),
        );
    }
    if raises {
        group = group.add_item(
            Item::new("latest", "Raise the ranges").with_description(format!(
                "{} safe, majors left out",
                plural(safe.len(), "package", "packages")
            )),
        );
    }
    if decides {
        group = group.add_item(
            Item::new("decide", "Decide per package")
                .with_description("tick what to raise, majors included"),
        );
    }
    if chooses {
        group = group.add_item(
            Item::new("choose", format!("Choose in {manager}'s own list"))
                .with_description("its interactive update, then the commit checks"),
        );
    }
    let menu = Menu::new().add_group(group.add_item(Item::new("cancel", "Cancel")));

    let interactive = io::stdout().is_terminal() && io::stderr().is_terminal();
    let chosen = match menu.run(
        Console::stderr(ColorMode::Auto),
        SelectMode::Auto,
        interactive,
    ) {
        Ok(Outcome::Selected(id)) => id,
        // Nothing is changed without someone choosing it, so a pipe gets the
        // list and stops there, as `--clean` does.
        Ok(Outcome::Unavailable) => {
            println!(
                "{}",
                console.paint(
                    Tone::Muted,
                    "Run opi --updates in a terminal to take any of them."
                )
            );
            return ExitCode::SUCCESS;
        }
        Ok(_) => return ExitCode::SUCCESS,
        Err(error) => {
            let block =
                ErrorBlock::new("Cannot open the terminal").with_explanation(format!("{error}"));
            write_block(&block, Console::stderr(ColorMode::Auto));
            return ExitCode::FAILURE;
        }
    };

    let (mode, names) = match chosen.as_str() {
        "in-range" => (outdated::Apply::InRange, safe),
        "latest" => (outdated::Apply::Latest, safe),
        "decide" => match pick_updates(found, &safe) {
            Some(picked) => (outdated::Apply::Latest, picked),
            None => return ExitCode::SUCCESS,
        },
        "choose" => (outdated::Apply::Choose, Vec::new()),
        _ => return ExitCode::SUCCESS,
    };
    if names.is_empty() && mode == outdated::Apply::Latest {
        return ExitCode::SUCCESS;
    }

    println!();
    if let Err(error) = outdated::apply(manager, root, workspace, mode, &names) {
        // The manager has already said why on the terminal it inherited; this
        // only stops the chain rather than repeating it.
        let block = ErrorBlock::new("The update did not finish")
            .with_explanation(format!("{error}"))
            .with_remedy("The package manager's own output above says why.");
        write_block(&block, Console::stderr(ColorMode::Auto));
        return ExitCode::FAILURE;
    }

    // The point of applying from here rather than retyping one line: what the
    // update broke is the next question, and nothing else puts the two
    // together. Measured at 1.4s here and 10.6s on the largest workspace, so
    // it is not a wait worth asking about first.
    println!();
    run_workflow("commit", manifest, members, rust_root, project, root)
}

/// Lets each update be ticked, returning the names to raise.
///
/// The safe ones start ticked and the majors do not, so the default answer is
/// the one "Raise the ranges" gives and a major is only taken on purpose.
/// `None` when the list was left without confirming.
fn pick_updates(found: &[outdated::Update], safe: &[String]) -> Option<Vec<String>> {
    let mut menu = Menu::new()
        .with_heading("Raise the ranges")
        .with_ticked(safe.iter().cloned());
    for (title, breaking) in [("Safe to take", false), ("A decision each", true)] {
        let mut group = Group::new(title);
        for update in found
            .iter()
            .filter(|update| update.jump.breaking() == breaking)
        {
            group = group.add_item(Item::new(&update.name, &update.name).with_description(
                format!(
                    "{} → {}  {}",
                    update.current,
                    update.latest,
                    update.jump.label()
                ),
            ));
        }
        if !group.items.is_empty() {
            menu = menu.add_group(group);
        }
    }

    match menu.run_multi(Console::stderr(ColorMode::Auto), SelectMode::Auto, true) {
        Ok(Picked::Chosen(names)) => Some(names),
        _ => None,
    }
}

/// Renders one ecosystem's updates, returning whether the run itself failed.
///
/// Nothing outdated and output `opi` cannot read are both said in a line and
/// leave the exit code clean: neither is a failure, and an empty section would
/// read as "nothing to report", which is the one thing it must not say where
/// nothing was asked.
fn section(
    console: Console,
    title: &str,
    listed: Result<Vec<outdated::Update>, outdated::OutdatedError>,
) -> bool {
    let found = match listed {
        Ok(found) if found.is_empty() => {
            println!(
                "{} {}",
                console.paint(Tone::Success, "✓"),
                console.paint(Tone::Success, format!("{title} — everything is current"))
            );
            return false;
        }
        Ok(found) => found,
        Err(error) => {
            let fatal = matches!(error, outdated::OutdatedError::Failed(_));
            println!(
                "{} {}",
                console.paint(Tone::Muted, "–"),
                console.paint(Tone::Muted, format!("{title} — {error}"))
            );
            return fatal;
        }
    };

    let (breaking, safe): (Vec<_>, Vec<_>) =
        found.iter().partition(|update| update.jump.breaking());

    let mut report = Report::new(
        title,
        if breaking.is_empty() {
            Verdict::Info
        } else {
            Verdict::Warning
        },
    )
    .with_detail_level(DetailLevel::Detailed);

    if !safe.is_empty() {
        report = report.add_metric(Metric::new("safe", safe.len().to_string()));
    }
    if !breaking.is_empty() {
        report = report
            .add_metric(Metric::new("major", breaking.len().to_string()).with_tone(Tone::Warning));
    }

    for (group_title, tone, group) in [
        ("Safe to take", Tone::Muted, &safe),
        ("A decision each", Tone::Warning, &breaking),
    ] {
        if group.is_empty() {
            continue;
        }
        let mut findings = FindingGroup::new(group_title);
        for update in group.iter() {
            findings = findings.add_finding(
                Finding::new(
                    tone,
                    format!("{} {} → {}", update.name, update.current, update.latest),
                )
                .with_rule_id(update.jump.label()),
            );
        }
        report = report.add_group(findings);
    }

    // No next step naming an update command. Measured, `pnpm update` moves
    // none of what this calls safe in a project whose ranges are exact or
    // `~` — the choice offered below is the one that exists.

    // No width hint: two metrics read better side by side than stacked, and
    // the findings wrap on their own.
    print!("{}", report.render(console));
    false
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
        assert_eq!(summary(&sections, 15), "15 entries · 3 groups");
    }

    #[test]
    fn a_project_with_no_workspace_says_nothing_about_packages() {
        // "1 package" would answer a question nobody in a single-package
        // project has.
        let tasks = sections(&[(TaskGroup::Development, 1)]);
        let sections = by_group(&tasks);
        assert!(!summary(&sections, 1).contains("package"));
        assert_eq!(summary(&sections, 1), "1 entry · 1 group");
    }

    #[test]
    fn the_summary_counts_workspace_packages() {
        let tasks = sections(&[
            (TaskGroup::Development, 2),
            (TaskGroup::Workspace("@casoon/blog".to_owned()), 1),
            (TaskGroup::Workspace("@casoon/starter".to_owned()), 1),
        ]);
        let sections = by_group(&tasks);
        assert_eq!(summary(&sections, 4), "4 entries · 3 groups · 2 packages");
    }
}
