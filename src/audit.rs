//! Dependency vulnerabilities.
//!
//! Unlike the checks in [`crate::check`], this one is parsed rather than
//! relayed: npm, pnpm, bun and yarn each emit a documented JSON shape — three
//! different ones between them — and a tree of six hundred dependencies
//! produces far too much prose to read. What matters is how many findings
//! there are, how bad they are, and which package to update.
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
    Failed(String),
}

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled => {
                f.write_str("cargo audit is not installed; cargo install cargo-audit adds it")
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

/// The shape bun's own `audit` produces.
///
/// A map from package name to a **list** of advisories, rather than npm's
/// object carrying one each — so one package can arrive with five, which npm's
/// shape has no way to express.
///
/// Measured against bun 1.3.3: the banner goes to stderr, stdout is pure JSON,
/// and the severity names are the five [`Severity::parse`] already knows.
///
/// Only `severity` is read. Bun names `vulnerable_versions`, never the patched
/// range, so [`Advisory::patched`] stays empty here and no remedy is shown —
/// guessing "update to 4.17.21" out of `<4.17.21` would be inventing the one
/// number that has to be right.
#[derive(Debug, Deserialize)]
struct BunAdvisory {
    #[serde(default)]
    severity: String,
}

/// One line of `yarn npm audit --json`.
///
/// Measured against yarn 4.18.0: newline-separated JSON, one object per
/// advisory, in a shape of its own rather than npm's — `value` names the
/// package, `children` carries the rest under capitalised keys:
///
/// ```json
/// {"value":"lodash","children":{"ID":1106913,"Severity":"high",
///  "Vulnerable Versions":"<4.17.21","Dependents":["app@workspace:."]}}
/// ```
///
/// As with bun, no patched range is reported — only how far back the
/// vulnerability reaches — so [`Advisory::patched`] stays empty here too.
#[derive(Debug, Deserialize)]
struct YarnAdvisory {
    value: String,
    children: YarnChildren,
}

#[derive(Debug, Deserialize)]
struct YarnChildren {
    #[serde(rename = "Severity", default)]
    severity: String,
}

/// Runs the package manager's audit and reads what it found.
pub fn run(manager: PackageManager, root: &Path) -> Result<Audit, AuditError> {
    let mut command = Command::new(manager.program());
    // Yarn's audit lives under its `npm` namespace, not as a bare subcommand,
    // and needs telling to cover every workspace and every transitive
    // dependency — the other managers do both without being asked.
    if matches!(manager, PackageManager::Yarn) {
        command.args(["npm", "audit", "--all", "--recursive", "--json"]);
    } else {
        command.args(["audit", "--json"]);
    }

    let output = command
        .current_dir(root)
        .output()
        .map_err(|error| AuditError::Failed(format!("could not run {manager}: {error}")))?;

    // A non-zero exit means findings, not a broken run, so the output is read
    // either way.
    let text = String::from_utf8_lossy(&output.stdout);

    if text.trim().is_empty() {
        // Yarn prints one line per advisory (see `read_yarn`) and nothing at
        // all when there are none, so silence on both streams is its clean
        // case — the same distinction `outdated::run` makes for npm and
        // pnpm. Every other manager here always prints at least an empty
        // report on a clean run — `{}` for bun, a full object with zero
        // counts for npm and pnpm (see the tests below) — so for them stdout
        // alone being empty already means the run did not really finish; a
        // network error or a broken lockfile explains itself on stderr
        // instead, the same shape `cargo audit` handles below.
        let silent = String::from_utf8_lossy(&output.stderr).trim().is_empty();
        if matches!(manager, PackageManager::Yarn) && silent {
            return Ok(Audit::default());
        }
        return Err(AuditError::Failed(format!(
            "{manager} audit: {}",
            stderr_reason(&output.stderr)
        )));
    }

    // The shapes are told apart once, here, by which manager was asked — not
    // by trying one parser and falling back to another, which would turn a
    // malformed report into a confusing error about the wrong format.
    let mut advisories = match manager {
        PackageManager::Bun => read_bun(&text),
        PackageManager::Yarn => read_yarn(&text),
        PackageManager::Npm | PackageManager::Pnpm => read_npm(&text),
    }
    .map_err(|error| {
        AuditError::Failed(format!("could not read {manager}'s audit output: {error}"))
    })?;

    condense(&mut advisories);
    Ok(Audit { advisories })
}

/// The last line of `stderr`, trimmed — what a package manager's own error
/// explanation usually ends with. `"no output"` when stderr is empty too, so
/// a message built from this is never blank.
fn stderr_reason(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .last()
        .unwrap_or("no output")
        .trim()
        .to_owned()
}

/// Worst first, and one line per package and severity.
///
/// bun reports every advisory separately — five for lodash in the fixture —
/// and the severity is what decides what to do about them, so the rest is
/// repetition. Shared with the tests rather than restated there, since a test
/// that condenses differently from `run` proves nothing.
fn condense(advisories: &mut Vec<Advisory>) {
    advisories.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| a.module.cmp(&b.module))
    });
    advisories.dedup_by(|a, b| a.module == b.module && a.severity == b.severity);
}

/// Reads the shape npm and pnpm share.
fn read_npm(text: &str) -> Result<Vec<Advisory>, serde_json::Error> {
    let report: AdvisoryReport = serde_json::from_str(text)?;
    Ok(report
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
        .collect())
}

/// Reads bun's shape.
fn read_bun(text: &str) -> Result<Vec<Advisory>, serde_json::Error> {
    // An audit with nothing to report prints `{}`, which parses to an empty
    // map on its own.
    let report: BTreeMap<String, Vec<BunAdvisory>> = serde_json::from_str(text)?;
    Ok(report
        .into_iter()
        .flat_map(|(module, found)| {
            found.into_iter().filter_map(move |raw| {
                Some(Advisory {
                    module: module.clone(),
                    severity: Severity::parse(&raw.severity)?,
                    patched: None,
                })
            })
        })
        .collect())
}

/// Reads yarn's shape: one JSON object per line, not a single document.
///
/// A malformed line is a malformed report, not one advisory among many —
/// the same reasoning `read_npm` and `read_bun` already apply, unlike
/// `outdated::read_cargo`'s per-member lines, where one crate failing to
/// serialise must not cost the others.
fn read_yarn(text: &str) -> Result<Vec<Advisory>, serde_json::Error> {
    let lines: Vec<YarnAdvisory> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;

    Ok(lines
        .into_iter()
        .filter_map(|raw| {
            Some(Advisory {
                module: raw.value,
                severity: Severity::parse(&raw.children.severity)?,
                patched: None,
            })
        })
        .collect())
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
        return Err(AuditError::Failed(format!(
            "cargo audit: {}",
            stderr_reason(&output.stderr)
        )));
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
        read_npm(json).expect("parse")
    }

    /// bun's shape, condensed the way `run` condenses it.
    fn parse_bun(json: &str) -> Vec<Advisory> {
        let mut found = read_bun(json).expect("parse");
        condense(&mut found);
        found
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

    /// Trimmed from a real `bun audit --json` run (bun 1.3.3) against
    /// lodash@4.17.20 and minimist@1.2.0. A map to **lists**, which is what
    /// npm's shape cannot express, and no patched range anywhere.
    const BUN: &str = r#"{"lodash":[
        {"id":1106913,"title":"Command Injection in lodash","severity":"high",
         "vulnerable_versions":"<4.17.21","cwe":["CWE-77"]},
        {"id":1108258,"title":"ReDoS in lodash","severity":"moderate",
         "vulnerable_versions":">=4.0.0 <4.17.21","cwe":["CWE-400"]},
        {"id":1120370,"title":"Prototype Pollution","severity":"moderate",
         "vulnerable_versions":">=4.0.0 <=4.17.22","cwe":["CWE-1321"]},
        {"id":1115806,"title":"Code Injection via _.template","severity":"high",
         "vulnerable_versions":">=4.0.0 <=4.17.23","cwe":["CWE-94"]},
        {"id":1115810,"title":"Prototype Pollution via array path","severity":"moderate",
         "vulnerable_versions":"<=4.17.23","cwe":["CWE-1321"]}],
      "minimist":[
        {"id":1097678,"title":"Prototype Pollution in minimist","severity":"critical",
         "vulnerable_versions":"<0.2.1","cwe":["CWE-1321"]},
        {"id":1096466,"title":"Prototype Pollution in minimist","severity":"moderate",
         "vulnerable_versions":"<1.2.6","cwe":["CWE-1321"]}]}"#;

    #[test]
    fn the_bun_shape_is_read() {
        let found = parse_bun(BUN);
        assert!(found.iter().any(|a| a.module == "lodash"));
        assert!(found.iter().any(|a| a.module == "minimist"));
    }

    #[test]
    fn several_advisories_for_one_package_condense_to_one_per_severity() {
        // Seven advisories in, four lines out: lodash is high and moderate,
        // minimist critical and moderate. The severity is what decides what to
        // do; the rest is repetition.
        let found = parse_bun(BUN);
        let lines: Vec<(&str, Severity)> = found
            .iter()
            .map(|a| (a.module.as_str(), a.severity))
            .collect();
        assert_eq!(
            lines,
            [
                ("minimist", Severity::Critical),
                ("lodash", Severity::High),
                ("lodash", Severity::Moderate),
                ("minimist", Severity::Moderate),
            ]
        );
    }

    #[test]
    fn bun_names_no_patched_range_so_none_is_shown() {
        // It reports `vulnerable_versions` and nothing else. Deriving
        // "update to 4.17.21" from "<4.17.21" would be inventing the one
        // number that has to be right.
        assert!(parse_bun(BUN).iter().all(|a| a.patched.is_none()));
    }

    #[test]
    fn a_clean_bun_audit_reports_nothing() {
        assert!(parse_bun("{}").is_empty());
    }

    /// Two lines from a real `yarn npm audit --all --recursive --json` run
    /// (yarn 4.18.0) against lodash@4.17.20, reformatted from its actual
    /// single-line-per-advisory shape for readability here.
    const YARN: &str = concat!(
        r#"{"value":"lodash","children":{"ID":1106913,"Severity":"high","#,
        r#""Vulnerable Versions":"<4.17.21","Dependents":["app@workspace:."]}}"#,
        "\n",
        r#"{"value":"minimist","children":{"ID":1097678,"Severity":"critical","#,
        r#""Vulnerable Versions":"<0.2.1","Dependents":["app@workspace:."]}}"#,
    );

    fn parse_yarn(text: &str) -> Vec<Advisory> {
        read_yarn(text).expect("parse")
    }

    #[test]
    fn the_yarn_shape_is_read() {
        let found = parse_yarn(YARN);
        assert_eq!(found.len(), 2);
        assert!(
            found
                .iter()
                .any(|a| a.module == "lodash" && a.severity == Severity::High)
        );
        assert!(
            found
                .iter()
                .any(|a| a.module == "minimist" && a.severity == Severity::Critical)
        );
    }

    #[test]
    fn yarn_names_no_patched_range_so_none_is_shown() {
        // Like bun, it reports how far back a vulnerability reaches and
        // nothing else — no version that has to be right to guess at.
        assert!(parse_yarn(YARN).iter().all(|a| a.patched.is_none()));
    }

    #[test]
    fn a_clean_yarn_audit_reports_nothing() {
        // Yarn prints one line per advisory and nothing at all when there
        // are none — unlike bun's `{}`, this is genuinely empty text.
        assert!(parse_yarn("").is_empty());
    }

    #[test]
    fn a_malformed_yarn_line_is_a_malformed_report() {
        // Unlike `outdated::read_cargo`'s per-member lines, one bad line here
        // is not "the other advisories are still fine" — the same strictness
        // `read_npm` and `read_bun` already apply to their own shapes.
        assert!(read_yarn("not json").is_err());
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
    fn stderr_reason_takes_the_last_line() {
        // A package manager's own explanation is usually its final line;
        // warnings ahead of it are noise the way pnpm's outdated warnings are.
        assert_eq!(
            stderr_reason(b"npm warn config deprecated\nnpm error network timeout"),
            "npm error network timeout"
        );
    }

    #[test]
    fn stderr_reason_falls_back_when_stderr_is_empty() {
        // An error message built from this must never end up blank.
        assert_eq!(stderr_reason(b""), "no output");
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
