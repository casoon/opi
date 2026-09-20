//! Dependency vulnerabilities.
//!
//! Unlike the checks in [`crate::check`], this one is parsed rather than
//! relayed: `npm` and `pnpm` both emit a documented JSON shape, and a tree of
//! six hundred dependencies produces far too much prose to read. What matters
//! is how many findings there are, how bad they are, and which package to
//! update.
//!
//! Two sources, kept apart. `run` asks the package manager, `cargo` asks
//! `cargo audit`, and they produce different types on purpose: a RustSec
//! advisory carries no severity, and folding it into npm's model would mean
//! either inventing one or making npm's optional.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::project::PackageManager;

/// How bad a finding is, worst first when ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Critical,
    High,
    Moderate,
    Low,
    Info,
}

impl Severity {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "critical" => Some(Self::Critical),
            "high" => Some(Self::High),
            "moderate" => Some(Self::Moderate),
            "low" => Some(Self::Low),
            "info" => Some(Self::Info),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Moderate => "moderate",
            Self::Low => "low",
            Self::Info => "info",
        }
    }

    /// Whether this warrants acting on rather than noting.
    pub fn serious(self) -> bool {
        matches!(self, Self::Critical | Self::High)
    }
}

/// One vulnerable package.
#[derive(Debug, Clone)]
pub struct Advisory {
    pub module: String,
    pub severity: Severity,
    /// The version range that fixes it, which is the part worth showing.
    /// What kind of vulnerability it is takes a line of prose and changes
    /// nothing about what to do; the tool itself has that.
    pub patched: Option<String>,
}

/// What an audit found.
#[derive(Debug, Default, Clone)]
pub struct Audit {
    pub advisories: Vec<Advisory>,
}

impl Audit {
    /// Findings per severity, worst first.
    pub fn counts(&self) -> BTreeMap<Severity, usize> {
        let mut counts = BTreeMap::new();
        for advisory in &self.advisories {
            *counts.entry(advisory.severity).or_insert(0) += 1;
        }
        counts
    }

    pub fn is_empty(&self) -> bool {
        self.advisories.is_empty()
    }
}

/// Why an audit could not be produced.
#[derive(Debug)]
pub enum AuditError {
    /// `cargo audit` is not installed.
    ///
    /// It is an external subcommand rather than part of the toolchain, so this
    /// is the ordinary case rather than a broken setup — and an empty section
    /// would read like "no findings", which is the one thing it must not say.
    NotInstalled,
    /// The package manager has no audit `opi` knows how to read.
    Unsupported(PackageManager),
    Failed(String),
}

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled => {
                f.write_str("cargo audit is not installed; cargo install cargo-audit adds it")
            }
            Self::Unsupported(manager) => {
                write!(f, "opi cannot read {manager}'s audit output")
            }
            Self::Failed(reason) => f.write_str(reason),
        }
    }
}

/// The shape `npm` and `pnpm` share, from npm's v6 audit format.
#[derive(Debug, Deserialize)]
struct AdvisoryReport {
    #[serde(default)]
    advisories: BTreeMap<String, RawAdvisory>,
    /// npm 7 and newer key findings by package name instead.
    #[serde(default)]
    vulnerabilities: BTreeMap<String, RawVulnerability>,
}

#[derive(Debug, Deserialize)]
struct RawAdvisory {
    #[serde(default)]
    module_name: String,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    patched_versions: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawVulnerability {
    #[serde(default)]
    name: String,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    range: Option<String>,
}

/// Runs the package manager's audit and reads what it found.
pub fn run(manager: PackageManager, root: &Path) -> Result<Audit, AuditError> {
    // yarn and bun each report in their own shape; claiming to audit them and
    // then showing nothing would be worse than saying so.
    if !matches!(manager, PackageManager::Npm | PackageManager::Pnpm) {
        return Err(AuditError::Unsupported(manager));
    }

    let output = Command::new(manager.program())
        .args(["audit", "--json"])
        .current_dir(root)
        .output()
        .map_err(|error| AuditError::Failed(format!("could not run {manager}: {error}")))?;

    // A non-zero exit means findings, not a broken run, so the output is read
    // either way.
    let text = String::from_utf8_lossy(&output.stdout);
    let report: AdvisoryReport = serde_json::from_str(&text).map_err(|error| {
        AuditError::Failed(format!("could not read {manager}'s audit output: {error}"))
    })?;

    let mut advisories: Vec<Advisory> = report
        .advisories
        .into_values()
        .filter_map(|raw| {
            Some(Advisory {
                module: raw.module_name,
                severity: Severity::parse(&raw.severity)?,
                patched: raw.patched_versions,
            })
        })
        .chain(report.vulnerabilities.into_values().filter_map(|raw| {
            Some(Advisory {
                module: raw.name,
                severity: Severity::parse(&raw.severity)?,
                patched: raw.range,
            })
        }))
        .collect();

    advisories.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| a.module.cmp(&b.module))
    });
    advisories.dedup_by(|a, b| a.module == b.module && a.severity == b.severity);

    Ok(Audit { advisories })
}

/// One advisory `cargo audit` reported.
///
/// Deliberately not an [`Advisory`]: measured against cargo-audit 0.22.1, the
/// JSON carries no severity at all — the key is absent, and only a `cvss`
/// vector string is there. The console output *does* print `Severity: 7.5
/// (high)`, because cargo-audit scores the vector itself, but reading a number
/// out of console text is the parser this project does not write, and scoring
/// the vector here would be reimplementing CVSS inside a tool whose rule is to
/// reimplement nothing.
///
/// So the severity is left out rather than guessed, and the report says where
/// to find it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoAdvisory {
    /// The crate, and the version the lockfile pins.
    pub package: String,
    pub version: String,
    /// The RUSTSEC id — what you search for, and what stands in for a severity
    /// label on screen.
    pub id: String,
    /// The version range that fixes it, where the advisory names one.
    pub patched: Option<String>,
}

/// The shape `cargo audit --json` produces, read down to what is used.
#[derive(Debug, Deserialize)]
struct CargoReport {
    #[serde(default)]
    vulnerabilities: CargoVulnerabilities,
}

/// Only `vulnerabilities`. `warnings` carries `unmaintained` and `unsound`,
/// which are informational and have no counterpart on the npm side — where
/// `npm audit` likewise reports vulnerabilities and nothing else.
#[derive(Debug, Default, Deserialize)]
struct CargoVulnerabilities {
    #[serde(default)]
    list: Vec<CargoEntry>,
}

#[derive(Debug, Deserialize)]
struct CargoEntry {
    advisory: RawCargoAdvisory,
    #[serde(default)]
    versions: RawVersions,
    package: RawPackage,
}

#[derive(Debug, Deserialize)]
struct RawCargoAdvisory {
    #[serde(default)]
    id: String,
}

#[derive(Debug, Default, Deserialize)]
struct RawVersions {
    /// A list, though every advisory measured named exactly one range.
    #[serde(default)]
    patched: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawPackage {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
}

/// Runs `cargo audit` and reads what it found.
pub fn cargo(root: &Path) -> Result<Vec<CargoAdvisory>, AuditError> {
    if !crate::cargo::has_subcommand("audit") {
        return Err(AuditError::NotInstalled);
    }

    let output = Command::new("cargo")
        .args(["audit", "--json"])
        .current_dir(root)
        .output()
        .map_err(|error| AuditError::Failed(format!("could not run cargo audit: {error}")))?;

    // Non-zero means findings here too, so the output is read either way.
    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() {
        // An advisory database it could not fetch leaves stdout empty and says
        // why on stderr. Relaying that beats a parse error about nothing.
        let reason = String::from_utf8_lossy(&output.stderr);
        let reason = reason
            .lines()
            .last()
            .unwrap_or("no output")
            .trim()
            .to_owned();
        return Err(AuditError::Failed(format!("cargo audit: {reason}")));
    }

    let report: CargoReport = serde_json::from_str(&text)
        .map_err(|error| AuditError::Failed(format!("could not read cargo audit: {error}")))?;

    Ok(read_cargo(report))
}

/// Turns the parsed report into the advisories worth showing.
fn read_cargo(report: CargoReport) -> Vec<CargoAdvisory> {
    let mut advisories: Vec<CargoAdvisory> = report
        .vulnerabilities
        .list
        .into_iter()
        .map(|entry| CargoAdvisory {
            package: entry.package.name,
            version: entry.package.version,
            id: entry.advisory.id,
            patched: Some(entry.versions.patched.join(", ")).filter(|range| !range.is_empty()),
        })
        .collect();

    // By crate, then by id: two advisories against the same crate belong next
    // to each other, and there is no severity to order them by.
    advisories.sort_by(|a, b| {
        a.package
            .cmp(&b.package)
            .then_with(|| a.version.cmp(&b.version))
            .then_with(|| a.id.cmp(&b.id))
    });
    advisories.dedup();
    advisories
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_cargo(json: &str) -> Vec<CargoAdvisory> {
        read_cargo(serde_json::from_str(json).expect("parse"))
    }

    /// Trimmed from a real `cargo audit --json` run (cargo-audit 0.22.1),
    /// keeping the keys that are read and one that is deliberately not.
    const CARGO_SAMPLE: &str = r#"{
      "database": { "advisory-count": 1251 },
      "vulnerabilities": {
        "found": true,
        "count": 2,
        "list": [
          {
            "advisory": {
              "id": "RUSTSEC-2026-0195",
              "package": "quick-xml",
              "title": "Unbounded namespace-declaration allocation",
              "severity": null,
              "cvss": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H"
            },
            "versions": { "patched": [">=0.41.0"], "unaffected": [] },
            "package": { "name": "quick-xml", "version": "0.38.4" }
          },
          {
            "advisory": { "id": "RUSTSEC-2024-0001", "package": "openssl" },
            "versions": { "patched": [] },
            "package": { "name": "openssl", "version": "0.10.1" }
          }
        ]
      },
      "warnings": {
        "unmaintained": [
          {
            "kind": "unmaintained",
            "advisory": { "id": "RUSTSEC-2020-0999" },
            "versions": { "patched": [] },
            "package": { "name": "abandoned", "version": "1.0.0" }
          }
        ]
      }
    }"#;

    #[test]
    fn a_cargo_advisory_carries_the_crate_the_id_and_the_fix() {
        let found = parse_cargo(CARGO_SAMPLE);
        assert_eq!(found.len(), 2);

        let quick_xml = found
            .iter()
            .find(|advisory| advisory.package == "quick-xml")
            .expect("quick-xml");
        assert_eq!(quick_xml.version, "0.38.4");
        assert_eq!(quick_xml.id, "RUSTSEC-2026-0195");
        assert_eq!(quick_xml.patched.as_deref(), Some(">=0.41.0"));
    }

    #[test]
    fn an_advisory_with_no_fix_yet_has_no_remedy() {
        let found = parse_cargo(CARGO_SAMPLE);
        let openssl = found
            .iter()
            .find(|advisory| advisory.package == "openssl")
            .expect("openssl");
        assert_eq!(
            openssl.patched, None,
            "an empty patched list is no range, not an empty one"
        );
    }

    #[test]
    fn warnings_are_not_vulnerabilities() {
        // `unmaintained` and `unsound` are informational, and npm's audit has
        // nothing like them. Counting them would inflate every Rust project.
        let found = parse_cargo(CARGO_SAMPLE);
        assert!(!found.iter().any(|advisory| advisory.package == "abandoned"));
    }

    #[test]
    fn a_clean_project_parses_to_nothing() {
        // What `cargo audit` emits where there is nothing to report: the
        // `list` key is present and empty.
        let found = parse_cargo(r#"{"vulnerabilities":{"found":false,"count":0,"list":[]}}"#);
        assert!(found.is_empty());
    }

    #[test]
    fn the_same_advisory_twice_is_reported_once() {
        let found = parse_cargo(
            r#"{"vulnerabilities":{"list":[
              {"advisory":{"id":"RUSTSEC-1"},"versions":{"patched":[">=2"]},
               "package":{"name":"a","version":"1.0"}},
              {"advisory":{"id":"RUSTSEC-1"},"versions":{"patched":[">=2"]},
               "package":{"name":"a","version":"1.0"}}
            ]}}"#,
        );
        assert_eq!(found.len(), 1);
    }

    fn parse(json: &str) -> Vec<Advisory> {
        let report: AdvisoryReport = serde_json::from_str(json).expect("parse");
        report
            .advisories
            .into_values()
            .filter_map(|raw| {
                Some(Advisory {
                    module: raw.module_name,
                    severity: Severity::parse(&raw.severity)?,
                    patched: raw.patched_versions,
                })
            })
            .collect()
    }

    #[test]
    fn the_pnpm_shape_is_read() {
        // Taken from a real `pnpm audit --json` run.
        let json = r#"{"advisories":{"1095100":{"id":1095100,
            "title":"Uncontrolled Resource Consumption in trim-newlines",
            "module_name":"trim-newlines","severity":"high",
            "patched_versions":">=3.0.1"}},"metadata":{}}"#;
        let found = parse(json);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].module, "trim-newlines");
        assert_eq!(found[0].severity, Severity::High);
        assert_eq!(found[0].patched.as_deref(), Some(">=3.0.1"));
    }

    #[test]
    fn an_unknown_severity_is_skipped_rather_than_guessed() {
        let json = r#"{"advisories":{"1":{"module_name":"x","severity":"spicy"}}}"#;
        assert!(parse(json).is_empty());
    }

    #[test]
    fn a_clean_audit_has_no_findings() {
        let json = r#"{"advisories":{},"metadata":{"vulnerabilities":{"high":0}}}"#;
        assert!(parse(json).is_empty());
    }

    #[test]
    fn severity_orders_worst_first() {
        let mut all = [
            Severity::Low,
            Severity::Critical,
            Severity::Moderate,
            Severity::High,
        ];
        all.sort();
        assert_eq!(
            all,
            [
                Severity::Critical,
                Severity::High,
                Severity::Moderate,
                Severity::Low
            ]
        );
        assert!(Severity::Critical.serious() && Severity::High.serious());
        assert!(!Severity::Moderate.serious());
    }

    #[test]
    fn counts_group_by_severity() {
        let audit = Audit {
            advisories: vec![
                Advisory {
                    module: "a".into(),
                    severity: Severity::High,
                    patched: None,
                },
                Advisory {
                    module: "b".into(),
                    severity: Severity::High,
                    patched: None,
                },
                Advisory {
                    module: "c".into(),
                    severity: Severity::Low,
                    patched: None,
                },
            ],
        };
        let counts = audit.counts();
        assert_eq!(counts.get(&Severity::High), Some(&2));
        assert_eq!(counts.get(&Severity::Low), Some(&1));
        assert_eq!(counts.keys().next(), Some(&Severity::High), "worst first");
    }
}
