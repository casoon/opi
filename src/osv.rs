//! Lockfile audit through `osv-scanner`.
//!
//! Runs *alongside* the native audits in [`crate::audit`], not instead of
//! them: the package managers know their own registries and `cargo audit` knows
//! RustSec, while `osv-scanner` reads the lockfiles directly against the OSV
//! database. It is an additional source, so a finding the native audit already
//! reported for the same package is dropped here ([`without_known`]) and the
//! two never show the same package twice. Where a native audit is missing or
//! failed, everything OSV found stays.
//!
//! The JSON is the documented `--format json` output of osv-scanner v2: the
//! `results[].packages[]` tree, with a CVSS score in `groups[].max_severity`.
//! The result is the same [`Advisory`] shape the native audits produce.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::audit::{Advisory, AuditError, Severity};

/// One vulnerable package, with the ecosystem OSV filed it under (`npm`,
/// `crates.io`, …) so it can be matched against the native audits.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub ecosystem: String,
    pub name: String,
    /// `module` reads `name version`: unlike the native audits, the lockfile
    /// says exactly which version is installed.
    pub advisory: Advisory,
}

#[derive(Debug, Deserialize)]
struct Report {
    #[serde(default)]
    results: Vec<Source>,
}

#[derive(Debug, Deserialize)]
struct Source {
    #[serde(default)]
    packages: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    package: Package,
    #[serde(default)]
    vulnerabilities: Vec<Vulnerability>,
    #[serde(default)]
    groups: Vec<Group>,
}

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    ecosystem: String,
}

#[derive(Debug, Deserialize)]
struct Vulnerability {
    id: String,
    #[serde(default)]
    affected: Vec<Affected>,
    #[serde(default)]
    database_specific: DatabaseSpecific,
}

#[derive(Debug, Default, Deserialize)]
struct DatabaseSpecific {
    #[serde(default)]
    severity: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Affected {
    #[serde(default)]
    package: Option<AffectedPackage>,
    #[serde(default)]
    ranges: Vec<Range>,
}

#[derive(Debug, Deserialize)]
struct AffectedPackage {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct Range {
    #[serde(default)]
    events: Vec<Event>,
}

#[derive(Debug, Deserialize)]
struct Event {
    #[serde(default)]
    fixed: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Group {
    #[serde(default)]
    ids: Vec<String>,
    /// A CVSS score as text, e.g. `"7.5"`; empty when the advisory has none.
    #[serde(default)]
    max_severity: String,
}

/// Runs `osv-scanner` over `root` and reads what it found.
///
/// [`AuditError::NotInstalled`] when the binary is missing, so the caller can
/// say so rather than print an empty section.
pub fn run(root: &Path) -> Result<Vec<Finding>, AuditError> {
    let output = match Command::new("osv-scanner")
        .args(["scan", "--format", "json", "-r"])
        .arg(root)
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(AuditError::NotInstalled);
        }
        Err(error) => {
            return Err(AuditError::Failed(format!(
                "could not run osv-scanner: {error}"
            )));
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() {
        // Exit 128 is osv-scanner's "found nothing to scan": no lockfile is
        // not a failure, and not a finding either.
        if output.status.code() == Some(128) {
            return Ok(Vec::new());
        }
        let reason = String::from_utf8_lossy(&output.stderr)
            .lines()
            .last()
            .unwrap_or("no output")
            .trim()
            .to_owned();
        return Err(AuditError::Failed(format!("osv-scanner: {reason}")));
    }

    // Exit 1 means findings, so the output is read whatever the status.
    read(&text).map_err(|error| AuditError::Failed(format!("could not read osv-scanner: {error}")))
}

/// What the native audits already reported, as `(ecosystem, package name)`;
/// OSV's ecosystem names (`npm`, `crates.io`).
pub fn without_known(found: Vec<Finding>, known: &[(&str, String)]) -> Vec<Finding> {
    found
        .into_iter()
        .filter(|finding| {
            !known
                .iter()
                .any(|(ecosystem, name)| *ecosystem == finding.ecosystem && *name == finding.name)
        })
        .collect()
}

/// One finding per package and version, at the worst severity among its
/// vulnerabilities, worst first.
fn read(text: &str) -> Result<Vec<Finding>, serde_json::Error> {
    let report: Report = serde_json::from_str(text)?;
    let mut found = BTreeMap::new();

    for entry in report
        .results
        .into_iter()
        .flat_map(|source| source.packages)
    {
        if entry.vulnerabilities.is_empty() {
            continue;
        }
        let severity = entry
            .vulnerabilities
            .iter()
            .map(|vulnerability| severity_of(vulnerability, &entry.groups))
            .min()
            .unwrap_or(Severity::Info);

        // A remedy only where all vulnerabilities agree on one fixed version;
        // with several, the highest is not something to guess at without
        // comparing versions per ecosystem.
        let mut fixes: Vec<&str> = entry
            .vulnerabilities
            .iter()
            .flat_map(|vulnerability| fixed_in(vulnerability, &entry.package.name))
            .collect();
        fixes.sort_unstable();
        fixes.dedup();
        let patched = match fixes.as_slice() {
            [only] => Some(format!(">={only}")),
            _ => None,
        };

        let module = format!("{} {}", entry.package.name, entry.package.version);
        found.insert(
            (entry.package.ecosystem.clone(), module.clone()),
            Finding {
                ecosystem: entry.package.ecosystem,
                name: entry.package.name,
                advisory: Advisory {
                    module,
                    severity,
                    patched,
                },
            },
        );
    }

    let mut found: Vec<Finding> = found.into_values().collect();
    found.sort_by(|a, b| {
        a.advisory
            .severity
            .cmp(&b.advisory.severity)
            .then_with(|| a.advisory.module.cmp(&b.advisory.module))
    });
    Ok(found)
}

/// The group's CVSS score where there is one, else the database's own label.
/// A vulnerability with neither is [`Severity::Info`]: not guessed upwards, and
/// not failing a run on a number nobody gave.
fn severity_of(vulnerability: &Vulnerability, groups: &[Group]) -> Severity {
    groups
        .iter()
        .find(|group| group.ids.contains(&vulnerability.id))
        .and_then(|group| group.max_severity.parse::<f64>().ok())
        .map(from_score)
        .or_else(|| {
            vulnerability
                .database_specific
                .severity
                .as_deref()
                .and_then(from_label)
        })
        .unwrap_or(Severity::Info)
}

/// The CVSS v3 qualitative bands.
fn from_score(score: f64) -> Severity {
    match score {
        s if s >= 9.0 => Severity::Critical,
        s if s >= 7.0 => Severity::High,
        s if s >= 4.0 => Severity::Moderate,
        s if s > 0.0 => Severity::Low,
        _ => Severity::Info,
    }
}

fn from_label(label: &str) -> Option<Severity> {
    match label.to_ascii_lowercase().as_str() {
        "critical" => Some(Severity::Critical),
        "high" => Some(Severity::High),
        "moderate" | "medium" => Some(Severity::Moderate),
        "low" => Some(Severity::Low),
        _ => None,
    }
}

fn fixed_in<'a>(vulnerability: &'a Vulnerability, name: &str) -> impl Iterator<Item = &'a str> {
    let name = name.to_owned();
    vulnerability
        .affected
        .iter()
        .filter(move |affected| {
            affected
                .package
                .as_ref()
                .is_some_and(|package| package.name == name)
        })
        .flat_map(|affected| &affected.ranges)
        .flat_map(|range| &range.events)
        .filter_map(|event| event.fixed.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Written to osv-scanner v2's documented `--format json` schema and cut
    /// down to the keys read (no osv-scanner was available to record one):
    /// an npm lockfile with lodash (two vulnerabilities, one fix) and a package
    /// with only a database label, plus a Cargo.lock with one scored crate.
    const SAMPLE: &str = r#"{
      "results": [
        {
          "source": { "path": "/app/package-lock.json", "type": "lockfile" },
          "packages": [
            {
              "package": { "name": "lodash", "version": "4.17.15", "ecosystem": "npm" },
              "vulnerabilities": [
                {
                  "id": "GHSA-35jh-r3h4-6jhm",
                  "affected": [{
                    "package": { "name": "lodash", "ecosystem": "npm" },
                    "ranges": [{ "type": "SEMVER",
                      "events": [{ "introduced": "0" }, { "fixed": "4.17.21" }] }]
                  }],
                  "database_specific": { "severity": "HIGH" }
                },
                {
                  "id": "GHSA-29mw-wpgm-hmr9",
                  "affected": [{
                    "package": { "name": "lodash", "ecosystem": "npm" },
                    "ranges": [{ "type": "SEMVER",
                      "events": [{ "introduced": "0" }, { "fixed": "4.17.21" }] }]
                  }],
                  "database_specific": { "severity": "MODERATE" }
                }
              ],
              "groups": [
                { "ids": ["GHSA-35jh-r3h4-6jhm"], "max_severity": "7.2" },
                { "ids": ["GHSA-29mw-wpgm-hmr9"], "max_severity": "5.3" }
              ]
            },
            {
              "package": { "name": "minimist", "version": "1.2.0", "ecosystem": "npm" },
              "vulnerabilities": [
                { "id": "GHSA-xvch-5gv4-984h", "database_specific": { "severity": "CRITICAL" } }
              ],
              "groups": [{ "ids": ["GHSA-xvch-5gv4-984h"], "max_severity": "" }]
            }
          ]
        },
        {
          "source": { "path": "/app/Cargo.lock", "type": "lockfile" },
          "packages": [
            {
              "package": { "name": "quick-xml", "version": "0.38.4", "ecosystem": "crates.io" },
              "vulnerabilities": [{ "id": "RUSTSEC-2026-0195" }],
              "groups": [{ "ids": ["RUSTSEC-2026-0195"], "max_severity": "9.8" }]
            }
          ]
        }
      ]
    }"#;

    #[test]
    fn one_finding_per_package_at_its_worst_severity() {
        let found = read(SAMPLE).expect("parse");
        let lodash = found.iter().find(|f| f.name == "lodash").expect("lodash");
        assert_eq!(found.len(), 3);
        assert_eq!(lodash.advisory.module, "lodash 4.17.15");
        assert_eq!(lodash.advisory.severity, Severity::High);
        assert_eq!(lodash.ecosystem, "npm");
    }

    #[test]
    fn worst_first_and_cargo_is_read_too() {
        let found = read(SAMPLE).expect("parse");
        let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
        // quick-xml scores 9.8, minimist falls back to its CRITICAL label.
        assert_eq!(names, ["minimist", "quick-xml", "lodash"]);
        assert_eq!(found[1].ecosystem, "crates.io");
        assert_eq!(found[1].advisory.severity, Severity::Critical);
    }

    #[test]
    fn a_single_agreed_fix_becomes_the_remedy_and_none_otherwise() {
        let found = read(SAMPLE).expect("parse");
        let lodash = found.iter().find(|f| f.name == "lodash").unwrap();
        assert_eq!(lodash.advisory.patched.as_deref(), Some(">=4.17.21"));
        let minimist = found.iter().find(|f| f.name == "minimist").unwrap();
        assert_eq!(minimist.advisory.patched, None);
    }

    #[test]
    fn packages_without_vulnerabilities_and_empty_reports_yield_nothing() {
        assert!(read(r#"{"results":[]}"#).unwrap().is_empty());
        assert!(read("{}").unwrap().is_empty());
        let clean = r#"{"results":[{"packages":[{"package":
            {"name":"a","version":"1","ecosystem":"npm"}}]}]}"#;
        assert!(read(clean).unwrap().is_empty());
    }

    #[test]
    fn what_the_native_audits_reported_is_dropped_per_ecosystem() {
        let found = read(SAMPLE).expect("parse");
        let known = [
            ("npm", "lodash".to_owned()),
            ("npm", "quick-xml".to_owned()),
        ];
        let rest = without_known(found, &known);
        let names: Vec<&str> = rest.iter().map(|f| f.name.as_str()).collect();
        // quick-xml is a crate, so an npm package of that name does not hide it.
        assert_eq!(names, ["minimist", "quick-xml"]);
    }

    #[test]
    fn scores_map_to_the_cvss_bands() {
        assert_eq!(from_score(9.0), Severity::Critical);
        assert_eq!(from_score(7.0), Severity::High);
        assert_eq!(from_score(4.0), Severity::Moderate);
        assert_eq!(from_score(0.1), Severity::Low);
        assert_eq!(from_score(0.0), Severity::Info);
    }
}
