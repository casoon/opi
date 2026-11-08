//! Lists outdated dependencies and applies the ones the user picks.

use std::io::{self, IsTerminal};
use std::path::Path;
use std::process::ExitCode;

use runemark::{
    ColorMode, Console, DetailLevel, ErrorBlock, Finding, FindingGroup, Group, Item, Menu, Metric,
    Outcome, Picked, Report, SelectMode, Tone, Verdict,
};

use super::workflow::run_workflow;
use crate::manifest::Manifest;
use crate::outdated;
use crate::project;
use crate::project::Project;
use crate::workspace;
use crate::{plural, write_block};

/// Shows which dependencies have moved on, separated by how far.
///
/// Rendered as a runemark `Report` rather than a hand-set table: the split
/// between safe and breaking is the whole value of this screen, and a report's
/// groups make it structural instead of a sentence underneath a list.
pub(crate) fn updates(
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
