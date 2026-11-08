//! The machine-readable renderer: `--json` for security and updates, and the
//! pieces health and the workflows share.
//!
//! A second renderer over the same results, not a second implementation: the
//! questions are asked of the same modules and the exit code answers them the
//! same way. The schema is documented in `docs/reference/cli.md`; a change to
//! a key's meaning raises `SCHEMA`.

use std::path::Path;
use std::process::ExitCode;

use serde_json::{Value, json};

use crate::audit;
use crate::check;
use crate::manifest::Manifest;
use crate::osv;
use crate::outdated;
use crate::project::Project;
use crate::scripts;
use crate::sources;
use crate::supply;
use crate::workspace;
use crate::{actions, check::Report};

/// Version of the JSON layout, in every document as `schema`.
pub(crate) const SCHEMA: u32 = 1;

pub(crate) fn print(document: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(document).expect("a JSON value serializes")
    );
}

pub(crate) fn exit(ok: bool) -> ExitCode {
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// `passed`, `warning`, `failed`, or `unavailable` where the tool could not run.
fn status(report: &Report) -> &'static str {
    if report.warned() {
        "warning"
    } else if report.passed() {
        "passed"
    } else if report.unusable() {
        "unavailable"
    } else {
        "failed"
    }
}

/// What a withheld output reads as in place of the tool's text.
const WITHHELD: &str = "output withheld: a secrets scanner may print the secrets it found";

/// One check's result, with the tool's output kept whole, except for secret
/// scanners, whose output can contain the secrets themselves.
pub(crate) fn check(report: &Report) -> Value {
    let output = if report.check.name == "Secrets" {
        WITHHELD
    } else {
        &report.output
    };
    json!({
        "name": report.check.name,
        "tool": report.check.tool,
        "scope": report.check.scope,
        "status": status(report),
        "duration_ms": u64::try_from(report.duration.as_millis()).unwrap_or(u64::MAX),
        "output": output,
    })
}

/// A section that did not run to an answer: `failed` where the tool broke,
/// `unavailable` where it is simply not there.
fn unanswered(name: &str, failed: bool, message: String) -> Value {
    json!({
        "name": name,
        "status": if failed { "failed" } else { "unavailable" },
        "message": message,
    })
}

fn advisories(found: &[audit::Advisory]) -> Vec<Value> {
    found
        .iter()
        .map(|advisory| {
            json!({
                "package": advisory.module,
                "severity": advisory.severity.label(),
                "patched": advisory.patched,
            })
        })
        .collect()
}

fn license_section(name: &str, found: Result<supply::Licenses, String>) -> Value {
    match found {
        Err(reason) => unanswered(name, false, reason),
        Ok(found) => json!({
            "name": name,
            "status": if found.notable.is_empty() { "passed" } else { "warning" },
            "counts": found.counts.iter()
                .map(|(license, count)| json!({"license": license, "count": count}))
                .collect::<Vec<_>>(),
            "notable": found.notable.iter()
                .map(|(license, package)| json!({"license": license, "package": package}))
                .collect::<Vec<_>>(),
        }),
    }
}

/// The security areas, in the order `--security` prints them.
///
/// `ok` follows the text renderer exactly: a serious advisory, a signature
/// that does not verify or an audit that broke fails it; policy findings and
/// tools that are not installed do not.
pub(crate) fn security(
    manifest: &Manifest,
    members: &[workspace::Member],
    rust_root: Option<&Path>,
    project: &Project,
    root: &Path,
) -> ExitCode {
    let manager = project.package_manager.manager;
    let mut clean = true;
    let mut sections = Vec::new();
    let mut known: Vec<(&str, String)> = Vec::new();

    let scans: Vec<check::Check> = check::Check::detect_all(manifest, members, None, root, manager)
        .into_iter()
        .filter(|check| check.name == "Secrets")
        .collect();
    if scans.is_empty() {
        sections.push(json!({
            "name": "Secrets",
            "status": "skipped",
            "message": "this project depends on no secret scanner opi knows",
        }));
    }
    for report in check::run_all(scans, |_| {}) {
        clean &= report.passed();
        sections.push(check(&report));
    }

    if project.npm {
        match audit::run(manager, root) {
            Ok(found) => {
                known.extend(found.advisories.iter().map(|a| ("npm", a.module.clone())));
                let serious = found.advisories.iter().any(|a| a.severity.serious());
                clean &= !serious;
                sections.push(json!({
                    "name": "Dependencies",
                    "status": if serious { "failed" } else if found.is_empty() { "passed" } else { "warning" },
                    "advisories": advisories(&found.advisories),
                }));
            }
            Err(error) => {
                let failed = matches!(error, audit::AuditError::Failed(_));
                clean &= !failed;
                sections.push(unanswered("Dependencies", failed, error.to_string()));
            }
        }
        clean &= supply_chain(&mut sections, manager, root);
    }

    if let Some(rust_root) = rust_root {
        match audit::cargo(rust_root) {
            Ok(found) => {
                clean &= found.is_empty();
                known.extend(found.iter().map(|a| ("crates.io", a.package.clone())));
                sections.push(json!({
                    "name": "Dependencies (rust)",
                    "status": if found.is_empty() { "passed" } else { "failed" },
                    "advisories": found.iter().map(|a| json!({
                        "package": a.package,
                        "version": a.version,
                        "id": a.id,
                        "patched": a.patched,
                    })).collect::<Vec<_>>(),
                }));
            }
            Err(error) => {
                let failed = matches!(error, audit::AuditError::Failed(_));
                clean &= !failed;
                sections.push(unanswered("Dependencies (rust)", failed, error.to_string()));
            }
        }
        sections.push(license_section(
            "Licenses (rust)",
            supply::cargo_licenses(rust_root),
        ));
    }

    if project.npm || rust_root.is_some() {
        match osv::run(root) {
            Ok(found) => {
                let found = osv::without_known(found, &known);
                let serious = found.iter().any(|f| f.advisory.severity.serious());
                clean &= !serious;
                sections.push(json!({
                    "name": "Dependencies (osv)",
                    "status": if serious { "failed" } else if found.is_empty() { "passed" } else { "warning" },
                    "advisories": found.iter().map(|f| json!({
                        "ecosystem": f.ecosystem,
                        "package": f.advisory.module,
                        "severity": f.advisory.severity.label(),
                        "patched": f.advisory.patched,
                    })).collect::<Vec<_>>(),
                }));
            }
            Err(error) => {
                let failed = matches!(error, audit::AuditError::Failed(_));
                clean &= !failed;
                sections.push(unanswered("Dependencies (osv)", failed, error.to_string()));
            }
        }
    }

    let unpinned = actions::unpinned(root);
    if !unpinned.is_empty() {
        sections.push(json!({
            "name": "GitHub Actions",
            "status": "warning",
            "unpinned": unpinned.iter().map(|u| json!({
                "file": u.file,
                "line": u.line,
                "reference": u.reference,
            })).collect::<Vec<_>>(),
        }));
    }

    let mut foreign: Vec<sources::Foreign> = [Some(root), rust_root]
        .iter()
        .flatten()
        .flat_map(|dir| sources::scan(dir))
        .collect();
    foreign.dedup();
    if !foreign.is_empty() {
        sections.push(json!({
            "name": "Sources",
            "status": "warning",
            "outside_registry": foreign.iter()
                .map(|f| json!({"package": f.name, "source": f.source}))
                .collect::<Vec<_>>(),
        }));
    }

    print(&json!({
        "schema": SCHEMA,
        "command": "security",
        "project": project.display_name(root),
        "ok": clean,
        "sections": sections,
    }));
    exit(clean)
}

/// Signatures, release age, install scripts and licenses of an npm project.
/// Returns whether nothing needs acting on.
fn supply_chain(
    sections: &mut Vec<Value>,
    manager: crate::project::PackageManager,
    root: &Path,
) -> bool {
    let mut clean = true;
    sections.push(match supply::signatures(root) {
        Ok(found) => {
            clean = found.invalid.is_empty();
            json!({
                "name": "Signatures",
                "status": if !clean { "failed" } else if found.missing.is_empty() { "passed" } else { "warning" },
                "invalid": found.invalid,
                "missing": found.missing,
            })
        }
        Err(reason) => unanswered("Signatures", false, reason),
    });

    match supply::release_age(manager, root) {
        Some(supply::ReleaseAge::Set(age)) => sections.push(json!({
            "name": "Release age",
            "status": "passed",
            "age": age,
        })),
        Some(supply::ReleaseAge::Unset) => sections.push(json!({
            "name": "Release age",
            "status": "warning",
            "age": null,
        })),
        None => {}
    }

    if let Some(found) = scripts::install_scripts(manager, root) {
        let unreviewed = found
            .packages
            .iter()
            .any(|(_, status)| *status == scripts::Status::Unreviewed);
        sections.push(json!({
            "name": "Install scripts",
            "status": if unreviewed { "warning" } else { "passed" },
            "packages": found.packages.iter().map(|(name, status)| json!({
                "package": name,
                "policy": match status {
                    scripts::Status::Allowed => "allowed",
                    scripts::Status::Ignored => "ignored",
                    scripts::Status::Unreviewed => "unreviewed",
                },
            })).collect::<Vec<_>>(),
        }));
    }

    if let Some(licenses) = supply::licenses(manager, root) {
        sections.push(license_section("Licenses", licenses));
    }
    clean
}

fn ecosystem(
    name: &str,
    listed: Result<Vec<outdated::Update>, outdated::OutdatedError>,
) -> (Value, bool) {
    match listed {
        Ok(found) => (
            json!({
                "ecosystem": name,
                "status": if found.is_empty() { "current" } else { "outdated" },
                "updates": found.iter().map(|update| json!({
                    "package": update.name,
                    "current": update.current,
                    "latest": update.latest,
                    "jump": update.jump.label(),
                    "in_range": update.in_range,
                })).collect::<Vec<_>>(),
            }),
            false,
        ),
        Err(error) => {
            let failed = matches!(error, outdated::OutdatedError::Failed(_));
            (
                json!({
                    "ecosystem": name,
                    "status": if failed { "failed" } else { "unavailable" },
                    "message": error.to_string(),
                    "updates": [],
                }),
                failed,
            )
        }
    }
}

/// The listing `--updates` prints, without the offer to apply anything.
/// Only a failed listing fails it, as in the text renderer.
pub(crate) fn updates(
    manifest: &Manifest,
    project: &Project,
    rust_root: Option<&Path>,
    root: &Path,
) -> ExitCode {
    let mut ecosystems = Vec::new();
    let mut failed = false;
    if project.npm {
        let workspace = workspace::declared(root, manifest);
        let (value, bad) = ecosystem(
            "npm",
            outdated::run(project.package_manager.manager, root, workspace),
        );
        ecosystems.push(value);
        failed |= bad;
    }
    if let Some(rust_root) = rust_root {
        let (value, bad) = ecosystem("rust", outdated::cargo(rust_root));
        ecosystems.push(value);
        failed |= bad;
    }
    print(&json!({
        "schema": SCHEMA,
        "command": "updates",
        "project": project.display_name(root),
        "ok": !failed,
        "ecosystems": ecosystems,
    }));
    exit(!failed)
}
