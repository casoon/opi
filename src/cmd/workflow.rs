//! Runs a named workflow: a sequence of checks and repository gates.

use std::path::Path;
use std::process::ExitCode;

use runemark::{ColorMode, Console, ErrorBlock, Tone};

use super::checks::check_config_error;
use crate::check;
use crate::check_config;
use crate::manifest::Manifest;
use crate::project::Project;
use crate::workflow;
use crate::workspace;
use crate::{OUTPUT_LINES, write_block};

/// Runs a named workflow: the repository questions, then the checks.
pub(crate) fn run_workflow(
    name: &str,
    manifest: &Manifest,
    members: &[workspace::Member],
    rust_root: Option<&Path>,
    project: &Project,
    root: &Path,
    json: bool,
) -> ExitCode {
    let console = Console::stdout(ColorMode::Auto);

    let Some(workflow) = workflow::Workflow::parse(name) else {
        let block = ErrorBlock::new(format!("No workflow named {name}"))
            .with_remedy("Known workflows: commit, push, release, deploy.");
        write_block(&block, Console::stderr(ColorMode::Auto));
        return ExitCode::FAILURE;
    };

    let config = match check_config::Config::load(root) {
        Ok(config) => config,
        Err(error) => return check_config_error(error),
    };
    let selection = config.workflows.get(name);

    if !json {
        println!(
            "{}  {}",
            console.paint(Tone::Title, project.display_name(root)),
            console.paint(Tone::Muted, workflow.label())
        );
        println!();
    }

    let mut blocked = false;
    let mut gates = Vec::new();

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
        if json {
            gates.push(serde_json::json!({
                "name": gate.name(),
                "status": match result.verdict {
                    runemark::Verdict::Passed => "passed",
                    runemark::Verdict::Skipped => "skipped",
                    _ => "failed",
                },
                "detail": result.detail,
            }));
            continue;
        }
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
    .filter(|check| {
        selection.map_or_else(
            || workflow.includes(check.name),
            |selected| selected.includes(check.name),
        )
    })
    .collect();

    let checks = if workflow == workflow::Workflow::Commit {
        check::Check::for_commit(checks, root)
            .into_iter()
            .filter(|check| selection.is_none_or(|selected| selected.includes(check.name)))
            .collect()
    } else {
        checks
    };

    let builds = if selection.map_or_else(|| workflow.builds(), |selected| selected.build) {
        check::Check::builds(
            manifest,
            members,
            rust_root,
            root,
            project.package_manager.manager,
        )
    } else {
        Vec::new()
    };
    if let Some(selection) = selection {
        let mut missing: Vec<_> = selection
            .checks
            .iter()
            .filter(|name| !checks.iter().any(|check| check.name == name.as_str()))
            .map(String::as_str)
            .collect();
        if selection.build && builds.is_empty() {
            missing.push("Build");
        }
        if !missing.is_empty() {
            return check_config_error(format!(
                "Selected checks unavailable in this project: {}",
                missing.join(", "),
            ));
        }
    }
    if checks.is_empty() && builds.is_empty() && !json {
        println!(
            "{}",
            console.paint(Tone::Muted, "No checks apply to this project.")
        );
    }

    let width = checks
        .iter()
        .chain(&builds)
        .map(|check| check.label().chars().count())
        .max()
        .unwrap_or(0);
    let print = |report: &check::Report| {
        if json {
            return;
        }
        let (tone, mark) = match report {
            report if report.warned() => (Tone::Warning, "!"),
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
    };
    let mut reports = check::run_all(checks, print);
    // After the checks and one at a time: a build pins the machine, and run
    // beside the checks it would slow every one of them down.
    for build in builds {
        let report = build.run();
        print(&report);
        reports.push(report);
    }

    let failed = reports.iter().filter(|report| !report.passed()).count();
    if json {
        let ready = !blocked && failed == 0;
        super::json::print(&serde_json::json!({
            "schema": super::json::SCHEMA,
            "command": "check",
            "workflow": name,
            "project": project.display_name(root),
            "ready": ready,
            "gates": gates,
            "checks": reports.iter().map(super::json::check).collect::<Vec<_>>(),
        }));
        return super::json::exit(ready);
    }
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
