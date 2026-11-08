//! Chooses which checks the workflows run, stored in `.opi/checks.json`.

use std::io::{self, IsTerminal};
use std::path::Path;
use std::process::ExitCode;

use runemark::{
    ColorMode, Console, ErrorBlock, Group, Item, Menu, Outcome, Picked, SelectMode, Tone,
};

use crate::check;
use crate::check_config;
use crate::manifest::Manifest;
use crate::project::Project;
use crate::workflow;
use crate::workspace;
use crate::write_block;

/// Configures a named workflow without running any checks.
pub(crate) fn configure_checks(
    name: Option<&str>,
    manifest: &Manifest,
    members: &[workspace::Member],
    rust_root: Option<&Path>,
    project: &Project,
    root: &Path,
) -> ExitCode {
    let mut config = match check_config::Config::load(root) {
        Ok(config) => config,
        Err(error) => return check_config_error(error),
    };
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        write_block(
            &ErrorBlock::new("Selecting checks needs a terminal")
                .with_remedy("Run opi --checks in a terminal, or edit .opi/checks.json."),
            Console::stderr(ColorMode::Auto),
        );
        return ExitCode::FAILURE;
    }
    let name = match name {
        Some(name) => name.to_owned(),
        None => {
            let group = ["commit", "push", "release", "deploy"]
                .into_iter()
                .fold(Group::new("Workflow"), |group, name| {
                    group.add_item(Item::new(name, name))
                });
            match Menu::new()
                .with_heading("Configure checks")
                .add_group(group)
                .run(Console::stderr(ColorMode::Auto), SelectMode::Auto, true)
            {
                Ok(Outcome::Selected(name)) => name,
                Ok(_) => return ExitCode::SUCCESS,
                Err(error) => return check_config_error(error.to_string()),
            }
        }
    };
    let Some(workflow) = workflow::Workflow::parse(&name) else {
        return check_config_error(format!(
            "Unknown workflow {name:?}; use commit, push, release or deploy."
        ));
    };
    let mut checks = check::Check::detect_all(
        manifest,
        members,
        rust_root,
        root,
        project.package_manager.manager,
    );
    if workflow == workflow::Workflow::Commit {
        checks = check::Check::for_commit(checks, root);
    }
    let available: std::collections::BTreeSet<&str> =
        checks.iter().map(|check| check.name).collect();
    let previous = config.workflows.get(&name);
    let mut ticked: Vec<String> = match previous {
        Some(previous) => previous.checks.clone(),
        None => available
            .iter()
            .filter(|name| workflow.includes(name))
            .map(|name| (*name).to_owned())
            .collect(),
    };
    let has_build = !check::Check::builds(
        manifest,
        members,
        rust_root,
        root,
        project.package_manager.manager,
    )
    .is_empty();
    let build = previous.map_or_else(
        || workflow.builds() && has_build,
        |selection| selection.build,
    );
    if build {
        ticked.push("@build".to_owned());
    }
    let mut group = Group::new("Checks — all detected tools and workspace members");
    for name in &available {
        group = group.add_item(Item::new(*name, *name));
    }
    if let Some(previous) = previous {
        for name in previous
            .checks
            .iter()
            .filter(|name| !available.contains(name.as_str()))
        {
            group = group.add_item(
                Item::new(name, name).with_description("Unavailable here; deselect to remove"),
            );
        }
    }
    if has_build || build {
        group =
            group.add_item(Item::new("@build", "Build").with_description("Run after the checks"));
    }
    let menu = Menu::new()
        .with_heading(format!("Checks before {name}"))
        .with_ticked(ticked)
        .add_group(group);
    let chosen = match menu.run_multi(Console::stderr(ColorMode::Auto), SelectMode::Auto, true) {
        Ok(Picked::Chosen(chosen)) => chosen,
        Ok(_) => return ExitCode::SUCCESS,
        Err(error) => return check_config_error(error.to_string()),
    };
    let selection = check_config::Selection {
        build: chosen.iter().any(|name| name == "@build"),
        checks: chosen.into_iter().filter(|name| name != "@build").collect(),
    };
    config.workflows.insert(name.clone(), selection);
    match config.save(root) {
        Ok(_) => {
            println!(
                "{}",
                Console::stdout(ColorMode::Auto).paint(
                    Tone::Success,
                    format!(
                        "Saved {name} checks in {}. Run opi --check {name} to use them.",
                        check_config::FILE
                    )
                )
            );
            ExitCode::SUCCESS
        }
        Err(error) => check_config_error(error),
    }
}

pub(crate) fn check_config_error(error: impl Into<String>) -> ExitCode {
    write_block(
        &ErrorBlock::new("Cannot use the workflow check selection")
            .with_explanation(error.into())
            .with_remedy("Review .opi/checks.json or run opi --checks in a terminal."),
        Console::stderr(ColorMode::Auto),
    );
    ExitCode::FAILURE
}
