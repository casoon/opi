//! Runs the project's checks and reports what each tool said.

use std::path::Path;
use std::process::ExitCode;

use runemark::{ColorMode, Console, Tone};

use crate::OUTPUT_LINES;
use crate::check;
use crate::manifest::Manifest;
use crate::project::Project;
use crate::workspace;

/// Runs the project's checks and reports what each tool said.
pub(crate) fn health(
    manifest: &Manifest,
    members: &[workspace::Member],
    rust_root: Option<&Path>,
    project: &Project,
    root: &Path,
    json: bool,
) -> ExitCode {
    let console = Console::stdout(ColorMode::Auto);
    let checks = check::Check::detect_all(
        manifest,
        members,
        rust_root,
        root,
        project.package_manager.manager,
    );

    if json {
        let reports = check::run_all(checks, |_| {});
        let ok = reports.iter().all(check::Report::passed);
        super::json::print(&serde_json::json!({
            "schema": super::json::SCHEMA,
            "command": "health",
            "project": project.display_name(root),
            "ok": ok,
            "checks": reports.iter().map(super::json::check).collect::<Vec<_>>(),
        }));
        return super::json::exit(ok);
    }

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
            report if report.warned() => (Tone::Warning, "!"),
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
    let warned = reports.iter().filter(|report| report.warned()).count();

    // No score. A composite number stops meaning anything within weeks; what a
    // failing tool actually said does not.
    for report in reports
        .iter()
        .filter(|report| !report.passed() || report.warned())
    {
        println!();
        let tone = if report.unusable() || report.warned() {
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
        let note = if warned > 0 {
            format!("{} checks passed, {warned} with warnings.", reports.len())
        } else {
            format!("{} checks passed.", reports.len())
        };
        println!("{}", console.paint(Tone::Success, note));
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
