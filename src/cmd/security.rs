//! Security checks: advisories, supply chain, secrets, licenses, install scripts.

use std::path::Path;
use std::process::ExitCode;

use runemark::{
    ColorMode, Console, DetailLevel, Finding, FindingGroup, Metric, NextStep, Report, Tone, Verdict,
};

use crate::actions;
use crate::audit;
use crate::check;
use crate::manifest::Manifest;
use crate::osv;
use crate::project;
use crate::project::Project;
use crate::scripts;
use crate::sources;
use crate::supply;
use crate::workspace;
use crate::{OUTPUT_LINES, plural};

/// Scans the repository for secrets and its dependencies for vulnerabilities.
pub(crate) fn security(
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
    // Packages the native audits reported, so the OSV section can skip them.
    let mut known: Vec<(&str, String)> = Vec::new();

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
            if report.passed() && !report.warned() {
                println!(
                    "{} {}",
                    console.paint(Tone::Success, "✓"),
                    console.paint(Tone::Success, format!("Secrets — {}", report.check.tool))
                );
            } else {
                let (tone, mark) = if report.warned() {
                    (Tone::Warning, "!")
                } else {
                    clean = false;
                    (Tone::Error, "✗")
                };
                println!(
                    "{} {}",
                    console.paint(tone, mark),
                    console.paint(tone, format!("Secrets — {}", report.check.tool))
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
                known.extend(found.advisories.iter().map(|a| ("npm", a.module.clone())));
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
        println!();
        clean &= supply_chain(console, project.package_manager.manager, root);
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
                known.extend(found.iter().map(|a| ("crates.io", a.package.clone())));

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
        license_report(console, supply::cargo_licenses(rust_root));
    }

    if project.npm || rust_root.is_some() {
        println!();
        clean &= osv_audit(console, root, &known);
    }

    unpinned_actions(console, root);

    println!();
    non_registry_sources(console, &[Some(root), rust_root]);
    if clean {
        println!("{}", console.paint(Tone::Success, "Nothing to act on."));
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The lockfile audit through `osv-scanner`, beside the native ones: only what
/// they did not already report. Returns whether nothing needs acting on.
fn osv_audit(console: Console, root: &Path, known: &[(&str, String)]) -> bool {
    let name = "Dependencies (osv)";
    match osv::run(root) {
        Ok(found) => {
            let found = osv::without_known(found, known);
            if found.is_empty() {
                println!(
                    "{} {}",
                    console.paint(Tone::Success, "✓"),
                    console.paint(
                        Tone::Success,
                        format!("{name} — nothing beyond the audits above")
                    )
                );
                return true;
            }
            let serious = found.iter().any(|f| f.advisory.severity.serious());
            let verdict = |serious: bool| {
                if serious {
                    Verdict::Failed
                } else {
                    Verdict::Warning
                }
            };
            let mut report =
                Report::new(name, verdict(serious)).with_detail_level(DetailLevel::Detailed);
            let audit = audit::Audit {
                advisories: found.iter().map(|f| f.advisory.clone()).collect(),
            };
            for (severity, count) in audit.counts() {
                report = report.add_metric(
                    Metric::new(severity.label(), count.to_string())
                        .with_verdict(verdict(severity.serious())),
                );
            }
            let mut group = FindingGroup::new("Vulnerable packages in the lockfiles");
            for advisory in &audit.advisories {
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
            print!("{}", report.add_group(group).render(console));
            !serious
        }
        Err(error) => {
            let reason = match &error {
                audit::AuditError::NotInstalled => {
                    "osv-scanner is not installed; see google.github.io/osv-scanner".to_owned()
                }
                other => other.to_string(),
            };
            println!(
                "{} {}",
                console.paint(Tone::Muted, "–"),
                console.paint(Tone::Muted, format!("{name} — {reason}"))
            );
            !audit_failed(&error)
        }
    }
}

/// Lists dependencies resolved from git, a tarball or a path outside the
/// workspace. Reported, not judged: a fork is often a deliberate choice.
fn non_registry_sources(console: Console, dirs: &[Option<&Path>]) {
    let mut found: Vec<sources::Foreign> = dirs
        .iter()
        .flatten()
        .flat_map(|dir| sources::scan(dir))
        .collect();
    found.dedup();
    if found.is_empty() {
        return;
    }
    println!(
        "{} {}",
        console.paint(Tone::Warning, "!"),
        console.paint(
            Tone::Warning,
            format!(
                "Sources — {} from outside the registry",
                plural(found.len(), "dependency", "dependencies")
            )
        )
    );
    for item in found.iter().take(OUTPUT_LINES) {
        println!(
            "  {}  {}",
            item.name,
            console.paint(Tone::Muted, &item.source)
        );
    }
    if let Some(rest) = found.len().checked_sub(OUTPUT_LINES).filter(|n| *n > 0) {
        println!("  {}", console.paint(Tone::Muted, format!("… {rest} more")));
    }
    println!();
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

/// Workflow `uses:` references a tag or branch can move.
///
/// Reported, not judged: pinning is a policy, so it does not fail the run.
fn unpinned_actions(console: Console, root: &Path) {
    let found = actions::unpinned(root);
    if found.is_empty() {
        return;
    }
    println!();
    println!(
        "{} {}",
        console.paint(Tone::Warning, "!"),
        console.paint(
            Tone::Warning,
            format!(
                "GitHub Actions — {} not pinned to a commit SHA",
                plural(found.len(), "reference", "references")
            )
        )
    );
    for item in found.iter().take(OUTPUT_LINES) {
        println!("  {}:{}  {}", item.file, item.line, item.reference);
    }
    if let Some(rest) = found.len().checked_sub(OUTPUT_LINES).filter(|n| *n > 0) {
        println!("  {}", console.paint(Tone::Muted, format!("… {rest} more")));
    }
}

/// Signatures, release age and licenses, beside the vulnerability audit.
///
/// Returns whether nothing needs acting on. Only a signature that does not
/// verify counts against that: an unset release age or a copyleft dependency
/// is a policy the project may have chosen, so both are reported, not judged.
fn supply_chain(console: Console, manager: project::PackageManager, root: &Path) -> bool {
    let line = |tone: Tone, mark: &str, text: String| {
        println!(
            "{} {}",
            console.paint(tone, mark),
            console.paint(tone, text)
        );
    };
    let list = |names: &[String]| {
        for name in names.iter().take(OUTPUT_LINES) {
            println!("  {name}");
        }
        if let Some(rest) = names.len().checked_sub(OUTPUT_LINES).filter(|n| *n > 0) {
            println!("  {}", console.paint(Tone::Muted, format!("… {rest} more")));
        }
    };

    let mut clean = true;
    match supply::signatures(root) {
        Ok(found) if found.invalid.is_empty() && found.missing.is_empty() => line(
            Tone::Success,
            "✓",
            "Signatures — every installed package verified".to_owned(),
        ),
        Ok(found) => {
            if !found.invalid.is_empty() {
                clean = false;
                line(
                    Tone::Error,
                    "✗",
                    format!(
                        "Signatures — {} do not match what the registry published",
                        plural(found.invalid.len(), "package", "packages")
                    ),
                );
                list(&found.invalid);
            }
            if !found.missing.is_empty() {
                line(
                    Tone::Warning,
                    "!",
                    format!(
                        "Signatures — {} without a registry signature",
                        plural(found.missing.len(), "package", "packages")
                    ),
                );
                list(&found.missing);
            }
        }
        Err(reason) => line(Tone::Muted, "–", format!("Signatures — {reason}")),
    }

    match supply::release_age(manager, root) {
        Some(supply::ReleaseAge::Set(age)) => line(
            Tone::Success,
            "✓",
            format!("Release age — versions younger than {age} are not installed"),
        ),
        Some(supply::ReleaseAge::Unset) => {
            line(
                Tone::Warning,
                "!",
                "Release age — not set: a version installs the moment it is published".to_owned(),
            );
            let remedy = match manager {
                project::PackageManager::Npm => "min-release-age=<days> in .npmrc",
                _ => "minimumReleaseAge: <minutes> in pnpm-workspace.yaml",
            };
            println!("  {}", console.paint(Tone::Muted, remedy));
        }
        None => {}
    }

    install_scripts(console, manager, root);

    if let Some(licenses) = supply::licenses(manager, root) {
        license_report(console, licenses);
    }

    clean
}

/// The license line, with the notable packages beneath it.
fn license_report(console: Console, found: Result<supply::Licenses, String>) {
    let found = match found {
        Ok(found) => found,
        Err(reason) => {
            println!(
                "{} {}",
                console.paint(Tone::Muted, "–"),
                console.paint(Tone::Muted, format!("Licenses — {reason}"))
            );
            return;
        }
    };
    if found.counts.is_empty() {
        println!(
            "{} {}",
            console.paint(Tone::Success, "✓"),
            console.paint(Tone::Success, "Licenses — no dependencies")
        );
        return;
    }
    const SHOWN: usize = 4;
    let mut parts: Vec<String> = found
        .counts
        .iter()
        .take(SHOWN)
        .map(|(license, count)| format!("{count} {license}"))
        .collect();
    let others: usize = found.counts.iter().skip(SHOWN).map(|(_, n)| n).sum();
    if others > 0 {
        parts.push(format!("{others} other"));
    }
    let (tone, mark) = if found.notable.is_empty() {
        (Tone::Success, "✓")
    } else {
        (Tone::Warning, "!")
    };
    println!(
        "{} {}",
        console.paint(tone, mark),
        console.paint(tone, format!("Licenses — {}", parts.join(" · ")))
    );
    for (license, name) in found.notable.iter().take(OUTPUT_LINES) {
        println!("  {name}  {license}");
    }
    if let Some(rest) = found
        .notable
        .len()
        .checked_sub(OUTPUT_LINES)
        .filter(|n| *n > 0)
    {
        println!("  {}", console.paint(Tone::Muted, format!("… {rest} more")));
    }
}

/// Dependencies with install scripts and whether the project allows them.
///
/// Reported, never judged: an unreviewed script is a policy gap worth a look,
/// not a failure.
fn install_scripts(console: Console, manager: project::PackageManager, root: &Path) {
    let Some(found) = scripts::install_scripts(manager, root) else {
        return;
    };
    let count = |status| found.packages.iter().filter(|(_, s)| *s == status).count();
    let (allowed, ignored) = (
        count(scripts::Status::Allowed),
        count(scripts::Status::Ignored),
    );
    let unreviewed: Vec<&str> = found
        .packages
        .iter()
        .filter(|(_, status)| *status == scripts::Status::Unreviewed)
        .map(|(name, _)| name.as_str())
        .collect();
    let summary = format!(
        "{} with install scripts: {allowed} allowed · {ignored} ignored · {} not reviewed",
        plural(found.packages.len(), "dependency", "dependencies"),
        unreviewed.len()
    );
    if unreviewed.is_empty() {
        println!(
            "{} {}",
            console.paint(Tone::Success, "✓"),
            console.paint(Tone::Success, format!("Install scripts — {summary}"))
        );
        return;
    }
    let tone = Tone::Warning;
    println!(
        "{} {}",
        console.paint(tone, "!"),
        console.paint(tone, format!("Install scripts — {summary}"))
    );
    for name in unreviewed.iter().take(OUTPUT_LINES) {
        println!("  {name}");
    }
    if let Some(rest) = unreviewed
        .len()
        .checked_sub(OUTPUT_LINES)
        .filter(|n| *n > 0)
    {
        println!("  {}", console.paint(Tone::Muted, format!("… {rest} more")));
    }
    let remedy = match manager {
        project::PackageManager::Pnpm => {
            "list them in onlyBuiltDependencies or ignoredBuiltDependencies"
        }
        project::PackageManager::Bun => "trust them in trustedDependencies, or leave them blocked",
        _ => "they run on install; ignore-scripts=true in .npmrc turns that off",
    };
    println!("  {}", console.paint(Tone::Muted, remedy));
}
