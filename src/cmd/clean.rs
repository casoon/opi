//! Removes build output and caches, after saying how much each takes.

use std::io::{self, IsTerminal};
use std::path::Path;
use std::process::ExitCode;

use runemark::{ColorMode, Console, ErrorBlock, Group, Item, Menu, Outcome, SelectMode, Tone};

use crate::clean;
use crate::manifest::Manifest;
use crate::project::Project;
use crate::workspace;
use crate::write_block;

/// Shows what can be removed, and removes what is chosen.
pub(crate) fn clean(
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
