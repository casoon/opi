//! Runs a named workflow: a sequence of checks and repository gates.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

use runemark::{ColorMode, Console, ErrorBlock, Tone};

use super::checks::check_config_error;
use crate::check;
use crate::check_config;
use crate::history;
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

    let in_clean_checkout = std::env::var_os(CLEAN_CHECKOUT).is_some();
    if !json {
        let label = if in_clean_checkout {
            format!("{}, in a clean checkout of HEAD", workflow.label())
        } else {
            workflow.label().to_owned()
        };
        println!(
            "{}  {}",
            console.paint(Tone::Title, project.display_name(root)),
            console.paint(Tone::Muted, label)
        );
        println!();
    }

    // package.json first, as before; a Rust project without one has its
    // version in Cargo.toml, and a release of it is tagged just the same.
    let version = manifest
        .version
        .clone()
        .or_else(|| rust_root.and_then(crate::cargo::version));
    let started = Instant::now();
    let state = history::state(root);
    let mut blocked = false;
    let mut gates = Vec::new();

    // The repository questions come first: a release from a dirty tree is
    // settled before spending a minute on its tests.
    // A fresh checkout is clean by construction, apart from the linked
    // `node_modules`, which an ignore rule for directories does not cover; and
    // its detached HEAD has no upstream, so the opi outside asked about that.
    let asked = workflow.gates().iter().filter(|gate| {
        !(in_clean_checkout
            && matches!(
                gate,
                workflow::Gate::WorkingTreeClean | workflow::Gate::NotBehindUpstream
            ))
    });
    for gate in asked {
        let result = gate.check(root, version.as_deref());
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

    // A tree that already passed the push checks passes them again: the hook
    // on the second push of the same commits costs nothing. Only a clean tree
    // has a tree hash to compare, and only text output takes the shortcut, so
    // a program asking for JSON always gets a fresh run.
    if workflow == workflow::Workflow::Push && !json && !blocked && !in_clean_checkout {
        let tree = state.as_ref().and_then(|state| state.tree.as_deref());
        if let Some(entry) = tree.and_then(|tree| history::green(root, name, tree)) {
            println!(
                "{} {}{}",
                console.paint(Tone::Success, "✓"),
                console.paint(Tone::Success, "Checks"),
                console.paint(
                    Tone::Muted,
                    format!(
                        "  passed on this tree {} ({})",
                        history::ago(entry.time, history::now()),
                        history::short(&entry.commit)
                    )
                )
            );
            println!();
            println!(
                "{}",
                console.paint(Tone::Success, format!("Ready to {name}."))
            );
            return ExitCode::SUCCESS;
        }
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
    if let Some(state) = state {
        history::record(
            root,
            &history::Entry {
                workflow: name.to_owned(),
                commit: state.commit,
                tree: state.tree,
                ready: !blocked && failed == 0,
                checks: reports
                    .iter()
                    .map(|report| history::CheckResult {
                        name: report.check.label(),
                        status: super::json::status(report).to_owned(),
                    })
                    .collect(),
                duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                time: history::now(),
                opi: env!("CARGO_PKG_VERSION").to_owned(),
            },
        );
    }
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

/// Whether `script` may run: the workflow it requires has passed on exactly
/// the tree checked out.
///
/// opi does not run the workflow itself. A deploy that starts minutes of
/// checks on its own is opi acting where it should report; the block names the
/// command instead.
pub(crate) fn required(root: &Path, script: &str, workflow: &str) -> Result<(), ErrorBlock> {
    let command = format!("opi --check {workflow}");
    if workflow::Workflow::parse(workflow).is_none() {
        return Err(
            ErrorBlock::new(format!("{script} requires a workflow named {workflow}"))
                .with_remedy("Known workflows: commit, push, release, deploy."),
        );
    }
    let Some(state) = history::state(root) else {
        return Err(ErrorBlock::new(format!("{script} requires {command}"))
            .with_explanation("That needs a git repository with a commit to check."));
    };
    let Some(tree) = state.tree else {
        return Err(ErrorBlock::new(format!("{script} requires {command}"))
            .with_explanation(
                "There are uncommitted changes, so what would run is not a checked commit.",
            )
            .with_remedy("Commit or stash them, then run the check."));
    };
    if history::green(root, workflow, &tree).is_none() {
        return Err(ErrorBlock::new(format!("{script} requires {command}"))
            .with_explanation(format!(
                "{} has not passed it yet.",
                history::short(&state.commit)
            ))
            .add_command(command));
    }
    Ok(())
}

/// Set for the opi that runs inside the temporary checkout, so that it checks
/// instead of trusting a green run of the working tree it was made to avoid.
const CLEAN_CHECKOUT: &str = "OPI_CLEAN_CHECKOUT";

/// Runs a workflow in a temporary `git worktree` of HEAD: what is committed,
/// without the file that only exists here.
///
/// Dependencies are not installed: `node_modules` is linked in from the
/// working tree and Cargo keeps its target directory, so the run costs a
/// checkout, not an install. What a clean checkout leaves out is exactly what
/// is not committed: an untracked source file, an ignored `.env`, a local edit.
pub(crate) fn run_clean(
    name: &str,
    members: &[workspace::Member],
    rust_root: Option<&Path>,
    root: &Path,
    json: bool,
) -> ExitCode {
    let fail = |message: String, remedy: &str| {
        let block = ErrorBlock::new(message).with_remedy(remedy);
        write_block(&block, Console::stderr(ColorMode::Auto));
        ExitCode::FAILURE
    };
    if name == "commit" {
        return fail(
            "The commit workflow cannot run in a clean checkout".to_owned(),
            "It checks what is staged, which a checkout of HEAD does not have.",
        );
    }
    let Some(toplevel) = git(root, &["rev-parse", "--show-toplevel"]).map(PathBuf::from) else {
        return fail(
            "Not a git repository".to_owned(),
            "A clean checkout is made from HEAD.",
        );
    };
    // The one gate the checkout cannot answer: its HEAD is detached.
    if workflow::Workflow::parse(name).is_some_and(|workflow| {
        workflow
            .gates()
            .contains(&workflow::Gate::NotBehindUpstream)
    }) {
        let behind = workflow::Gate::NotBehindUpstream.check(root, None);
        if behind.verdict == runemark::Verdict::Failed {
            return fail(
                format!("Not ready to {name}: {}", behind.detail.unwrap_or_default()),
                "Pull first; the clean checkout would check commits that cannot be pushed.",
            );
        }
    }

    let relative = match (root.canonicalize(), toplevel.canonicalize()) {
        (Ok(root), Ok(toplevel)) => root.strip_prefix(toplevel).map(Path::to_path_buf).ok(),
        _ => None,
    };
    let Some(relative) = relative else {
        return fail(
            "Could not place the project in its repository".to_owned(),
            "Run opi from inside the repository.",
        );
    };

    let checkout = std::env::temp_dir().join(format!("opi-clean-{}", std::process::id()));
    let checkout_arg = checkout.to_string_lossy().into_owned();
    if git(
        root,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            &checkout_arg,
            "HEAD",
        ],
    )
    .is_none()
    {
        return fail(
            "Could not make a checkout of HEAD".to_owned(),
            "git worktree add failed; is there a commit yet?",
        );
    }

    let project = checkout.join(&relative);
    let mut links = Vec::new();
    for dir in std::iter::once(root.to_path_buf()).chain(members.iter().map(|m| m.path.clone())) {
        let modules = dir.join("node_modules");
        let Ok(inner) = dir.strip_prefix(root) else {
            continue;
        };
        let target = project.join(inner).join("node_modules");
        if modules.is_dir()
            && target.parent().is_some_and(Path::is_dir)
            && !target.exists()
            && symlink(&modules, &target).is_ok()
        {
            links.push(target);
        }
    }

    let mut command = Command::new(std::env::current_exe().unwrap_or_else(|_| "opi".into()));
    command
        .args(["--check", name])
        .args(json.then_some("--json"))
        .current_dir(&project)
        .env(CLEAN_CHECKOUT, "1");
    let target = rust_root
        .map(|rust| rust.join("target"))
        .filter(|target| target.is_dir());
    if let (None, Some(target)) = (std::env::var_os("CARGO_TARGET_DIR"), target) {
        command.env("CARGO_TARGET_DIR", target);
    }
    let status = command.status();

    // The links first, so that removing the checkout can never reach through
    // one into the working tree's dependencies.
    for link in links {
        let _ = fs::remove_file(link);
    }
    let _ = git(root, &["worktree", "remove", "--force", &checkout_arg]);

    match status {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(error) => fail(
            format!("Could not start opi in the checkout: {error}"),
            "The checkout was removed again.",
        ),
    }
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
