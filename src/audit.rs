//! Dependency vulnerabilities.
//!
//! Unlike the checks in [`crate::check`], this one is parsed rather than
//! relayed: `npm` and `pnpm` both emit a documented JSON shape, and a tree of
//! six hundred dependencies produces far too much prose to read. What matters
//! is how many findings there are, how bad they are, and which package to
//! update.

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
    /// The package manager has no audit `opi` knows how to read.
    Unsupported(PackageManager),
    Failed(String),
}

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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

#[cfg(test)]
mod tests {
    use super::*;

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
