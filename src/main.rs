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
    ColorMode, Console, ErrorBlock, Group, Hint, Item, Menu, Outcome, SelectMode, Tone,
};

use crate::cli::Invocation;
use crate::manifest::{Manifest, ManifestError};
use crate::project::Project;
use crate::task::{Task, by_group, find, suggestions};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Lines of a failing tool's output health prints before pointing at the tool.
const OUTPUT_LINES: usize = 20;

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
        Invocation::List => list(&manifest, &members, &project, &tasks, &root),
        Invocation::Health => health(&manifest, &members, &project, &root),
        Invocation::Clean => clean(&manifest, &members, &project, &root),
        Invocation::Security => security(&manifest, &members, &project, &root),
        Invocation::Updates => updates(&project, &root),
        Invocation::Workflow(name) => run_workflow(&name, &manifest, &members, &project, &root),
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
fn list(
    manifest: &Manifest,
    members: &[workspace::Member],
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

    let mut menu = build_menu(project, tasks, directory);
    // Offered only where it would do something; a key that answers "nothing
    // applies" is worse than no key.
    let checks = check::Check::detect_all(manifest, members, directory);
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
        Outcome::Selected(id) => start(project, tasks, &id, &[]),
        // Nothing was chosen; that is not a failure.
        Outcome::Cancelled => ExitCode::SUCCESS,
        Outcome::Hotkey('H') => health(manifest, members, project, directory),
        Outcome::Hotkey('C') => clean(manifest, members, project, directory),
        Outcome::Hotkey('S') => security(manifest, members, project, directory),
        Outcome::Hotkey('U') => updates(project, directory),
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

/// Runs the project's checks and reports what each tool said.
fn health(
    manifest: &Manifest,
    members: &[workspace::Member],
    project: &Project,
    root: &Path,
) -> ExitCode {
    let console = Console::stdout(ColorMode::Auto);
    let checks = check::Check::detect_all(manifest, members, root);

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
    project: &Project,
    root: &Path,
) -> ExitCode {
    let console = Console::stdout(ColorMode::Auto);
    let candidates = clean::candidates(manifest, members, root);

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
    let scans: Vec<check::Check> = check::Check::detect_all(manifest, members, root)
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

            let summary = found
                .counts()
                .iter()
                .map(|(severity, count)| format!("{count} {}", severity.label()))
                .collect::<Vec<_>>()
                .join(", ");
            println!(
                "{} {}",
                console.paint(if serious { Tone::Error } else { Tone::Warning }, "!"),
                console.paint(
                    if serious { Tone::Error } else { Tone::Warning },
                    format!("Dependencies — {summary}")
                )
            );

            // Summarised by default: a tree of six hundred dependencies
            // produces far more prose than anyone reads.
            let width = found
                .advisories
                .iter()
                .map(|advisory| advisory.module.chars().count())
                .max()
                .unwrap_or(0);
            for advisory in found.advisories.iter().take(OUTPUT_LINES) {
                let fix = advisory
                    .patched
                    .as_deref()
                    .map_or_else(String::new, |patched| format!("  → {patched}"));
                println!(
                    "  {}  {}{}",
                    console.paint(Tone::Info, format!("{:width$}", advisory.module)),
                    console.paint(Tone::Muted, advisory.severity.label()),
                    console.paint(Tone::Muted, fix),
                );
            }
            if let Some(rest) = found
                .advisories
                .len()
                .checked_sub(OUTPUT_LINES)
                .filter(|n| *n > 0)
            {
                println!("  {}", console.paint(Tone::Muted, format!("… {rest} more")));
            }
        }
        Err(error) => println!(
            "{} {}",
            console.paint(Tone::Muted, "–"),
            console.paint(Tone::Muted, format!("Dependencies — {error}"))
        ),
    }

    println!();
    if clean {
        println!("{}", console.paint(Tone::Success, "Nothing to act on."));
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Runs a named workflow: the repository questions, then the checks.
fn run_workflow(
    name: &str,
    manifest: &Manifest,
    members: &[workspace::Member],
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

    let checks: Vec<check::Check> = check::Check::detect_all(manifest, members, root)
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
fn updates(project: &Project, root: &Path) -> ExitCode {
    let console = Console::stdout(ColorMode::Auto);
    let manager = project.package_manager.manager;

    println!(
        "{}  {}",
        console.paint(Tone::Title, project.display_name(root)),
        console.paint(Tone::Muted, "updates")
    );
    println!();

    let found = match outdated::run(manager, root) {
        Ok(found) => found,
        Err(error) => {
            let block = ErrorBlock::new("Cannot list outdated dependencies")
                .with_explanation(format!("{error}"));
            write_block(&block, Console::stderr(ColorMode::Auto));
            return ExitCode::FAILURE;
        }
    };

    if found.is_empty() {
        println!("{}", console.paint(Tone::Success, "Everything is current."));
        return ExitCode::SUCCESS;
    }

    let width = found
        .iter()
        .map(|update| update.name.chars().count())
        .max()
        .unwrap_or(0);
    let versions = found
        .iter()
        .map(|update| update.current.chars().count())
        .max()
        .unwrap_or(0);

    for update in &found {
        println!(
            "  {}  {} → {}  {}",
            console.paint(Tone::Info, format!("{:width$}", update.name)),
            console.paint(Tone::Muted, format!("{:>versions$}", update.current)),
            console.paint(Tone::Muted, &update.latest),
            console.paint(
                if update.jump.breaking() {
                    Tone::Warning
                } else {
                    Tone::Muted
                },
                update.jump.label()
            ),
        );
    }

    let safe = found
        .iter()
        .filter(|update| !update.jump.breaking())
        .count();
    let breaking = found.len() - safe;
    println!();

    // The separation is the point: "four safe, one major" is a decision, a
    // column of version numbers is homework.
    let summary = match (safe, breaking) {
        (0, n) => format!("{n} major update(s) — each one a decision of its own"),
        (n, 0) => format!("{n} safe update(s)"),
        (n, m) => format!("{n} safe update(s) · {m} major"),
    };
    println!("{}", console.paint(Tone::Info, summary));

    if safe > 0 {
        // opi stops here on purpose. Applying updates rewrites package.json
        // and the lockfile, and with pnpm catalogs the versions may not even
        // live in package.json — a wrong guess there is expensive, and the
        // package manager already does it correctly.
        println!(
            "{}",
            console.paint(Tone::Muted, format!("Apply them with: {manager} update"))
        );
    }

    ExitCode::SUCCESS
}
