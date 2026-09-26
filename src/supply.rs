//! Supply-chain questions beside the vulnerability audit.
//!
//! Three, each answered by the package manager itself: whether the installed
//! packages carry valid registry signatures, which licenses they come under,
//! and whether freshly published versions are held back. Nothing here is
//! computed by `opi`; it asks, parses the documented JSON, and reports.
//!
//! Only a broken signature is a failure. The other two describe a policy the
//! project may have chosen deliberately, so they are reported, never judged.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::project::PackageManager;

/// What `npm audit signatures` found wrong, as `name@version`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Signatures {
    /// A signature that does not verify: the package is not what the
    /// registry published.
    pub invalid: Vec<String>,
    /// No signature where the registry should have one.
    pub missing: Vec<String>,
}

/// Verifies the installed packages' registry signatures.
///
/// Asked of npm whatever the package manager: it reads `node_modules`, which
/// pnpm and bun lay out too, and measured on a pnpm workspace it verified 411
/// packages in two seconds. Yarn's Plug'n'Play has no `node_modules`, and npm
/// then says it found nothing to audit — that arrives as the error.
pub fn signatures(root: &Path) -> Result<Signatures, String> {
    let output = Command::new("npm")
        .args(["audit", "signatures", "--json"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("could not run npm: {error}"))?;
    parse_signatures(&String::from_utf8_lossy(&output.stdout))
}

fn parse_signatures(json: &str) -> Result<Signatures, String> {
    let value: Value = serde_json::from_str(json).map_err(|_| "npm gave no report".to_owned())?;
    if let Some(error) = value.get("error") {
        return Err(error
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("npm could not verify signatures")
            .to_owned());
    }
    let packages = |key: &str| -> Vec<String> {
        value
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let name = entry.get("name")?.as_str()?;
                Some(match entry.get("version").and_then(Value::as_str) {
                    Some(version) => format!("{name}@{version}"),
                    None => name.to_owned(),
                })
            })
            .collect()
    };
    Ok(Signatures {
        invalid: packages("invalid"),
        missing: packages("missing"),
    })
}

/// The production dependencies' licenses.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Licenses {
    /// Each license with how many packages use it, most common first.
    pub counts: Vec<(String, usize)>,
    /// Packages whose license asks more of the project than attribution, as
    /// `(license, package)`.
    pub notable: Vec<(String, String)>,
}

/// Lists the production dependencies' licenses, where pnpm can.
///
/// pnpm is the only package manager with a license report of its own — npm,
/// bun and Cargo need a separate tool — and it answers offline in well under a
/// second. Development dependencies are left out: they do not ship.
pub fn licenses(root: &Path) -> Result<Licenses, String> {
    let output = Command::new("pnpm")
        .args(["licenses", "list", "--json", "--prod"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("could not run pnpm: {error}"))?;
    parse_licenses(&String::from_utf8_lossy(&output.stdout))
}

fn parse_licenses(json: &str) -> Result<Licenses, String> {
    let value: Value = serde_json::from_str(json).map_err(|_| "pnpm gave no report".to_owned())?;
    let Some(map) = value.as_object() else {
        return Err("pnpm gave no report".to_owned());
    };
    if let Some(error) = map.get("error") {
        // The message names one package's store entry and a hash; the cause
        // is almost always that nothing is installed yet.
        return Err(match error.get("code").and_then(Value::as_str) {
            Some("ERR_PNPM_MISSING_PACKAGE_INDEX_FILE") => {
                "dependencies not installed — run pnpm install"
            }
            _ => error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("pnpm could not list licenses"),
        }
        .to_owned());
    }

    let mut licenses = Licenses::default();
    for (license, packages) in map {
        let packages = packages.as_array().map_or(&[][..], Vec::as_slice);
        licenses.counts.push((license.clone(), packages.len()));
        if notable(license) {
            licenses.notable.extend(
                packages
                    .iter()
                    .filter_map(|package| package.get("name")?.as_str())
                    .map(|name| (license.clone(), name.to_owned())),
            );
        }
    }
    licenses
        .counts
        .sort_by(|(a, count_a), (b, count_b)| count_b.cmp(count_a).then_with(|| a.cmp(b)));
    Ok(licenses)
}

/// Whether a license asks more than attribution, or says nothing at all.
///
/// Copyleft of any strength, the source-available ones, and a missing or
/// proprietary declaration. A choice (`MIT OR GPL-3.0`) is notable only if
/// every option is.
fn notable(license: &str) -> bool {
    license
        .trim_matches(|c| c == '(' || c == ')')
        .split(" OR ")
        .all(|option| {
            let option = option.trim().to_ascii_uppercase();
            option.is_empty()
                || matches!(option.as_str(), "UNKNOWN" | "UNLICENSED")
                || [
                    "GPL", "MPL", "EPL", "EUPL", "CDDL", "SSPL", "BUSL", "CC-BY-SA",
                ]
                .iter()
                .any(|family| option.contains(family))
        })
}

/// Whether versions younger than some age are held back from installs.
#[derive(Debug, PartialEq, Eq)]
pub enum ReleaseAge {
    /// Held back, for this long.
    Set(String),
    /// A version is installed the moment it is published.
    Unset,
}

/// Asks the package manager for its effective minimum release age.
///
/// Asked rather than read from a file, because the setting can live in the
/// workspace file, a project `.npmrc` or the user's global config — measured
/// here, three workspaces carried an exclude list while the age itself was set
/// in none of them. `None` where the package manager has no such setting to
/// ask about.
pub fn release_age(manager: PackageManager, root: &Path) -> Option<ReleaseAge> {
    // pnpm counts minutes, npm days.
    let (key, minutes_per_unit) = match manager {
        PackageManager::Pnpm => ("minimumReleaseAge", 1),
        PackageManager::Npm => ("min-release-age", 24 * 60),
        PackageManager::Yarn | PackageManager::Bun => return None,
    };
    let output = Command::new(manager.program())
        .args(["config", "get", key])
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| parse_release_age(&String::from_utf8_lossy(&output.stdout), minutes_per_unit))
}

fn parse_release_age(value: &str, minutes_per_unit: u64) -> ReleaseAge {
    match value.trim().parse::<u64>() {
        Ok(0) | Err(_) => ReleaseAge::Unset,
        Ok(units) => ReleaseAge::Set(duration(units * minutes_per_unit)),
    }
}

fn duration(minutes: u64) -> String {
    let (count, unit) = if minutes % (24 * 60) == 0 {
        (minutes / (24 * 60), "day")
    } else if minutes % 60 == 0 {
        (minutes / 60, "hour")
    } else {
        (minutes, "minute")
    };
    format!("{count} {unit}{}", if count == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_signatures_parse_to_nothing() {
        let parsed = parse_signatures(r#"{"invalid": [], "missing": []}"#).expect("parsed");
        assert_eq!(parsed, Signatures::default());
    }

    #[test]
    fn broken_signatures_are_named_with_their_version() {
        let parsed = parse_signatures(
            r#"{"invalid": [{"name": "left-pad", "version": "1.3.0", "code": "EINTEGRITYSIGNATURE"}],
                "missing": [{"name": "odd"}]}"#,
        )
        .expect("parsed");
        assert_eq!(parsed.invalid, ["left-pad@1.3.0"]);
        assert_eq!(parsed.missing, ["odd"]);
    }

    #[test]
    fn a_signature_error_carries_npms_summary() {
        let error = parse_signatures(
            r#"{"error": {"summary": "found no dependencies to audit that were installed from a supported registry", "detail": ""}}"#,
        )
        .expect_err("an error");
        assert!(error.starts_with("found no dependencies"));
    }

    #[test]
    fn licenses_are_counted_and_the_notable_named() {
        let parsed = parse_licenses(
            r#"{
                "MIT": [{"name": "a"}, {"name": "b"}],
                "LGPL-3.0-or-later": [{"name": "c"}],
                "MIT OR Apache-2.0": [{"name": "d"}],
                "UNLICENSED": [{"name": "e"}]
            }"#,
        )
        .expect("parsed");
        assert_eq!(parsed.counts[0], ("MIT".to_owned(), 2));
        assert_eq!(parsed.counts.len(), 4);
        assert_eq!(
            parsed.notable,
            [
                ("LGPL-3.0-or-later".to_owned(), "c".to_owned()),
                ("UNLICENSED".to_owned(), "e".to_owned()),
            ]
        );
    }

    #[test]
    fn a_choice_is_notable_only_if_every_option_is() {
        assert!(!notable("MIT OR GPL-3.0"));
        assert!(!notable("(MIT OR Apache-2.0)"));
        assert!(notable("GPL-2.0 OR GPL-3.0"));
        assert!(notable("MPL-2.0"));
        assert!(!notable("BSD-3-Clause"));
        assert!(!notable("ISC"));
    }

    #[test]
    fn missing_installs_are_named_as_such() {
        let error = parse_licenses(
            r#"{"error": {"code": "ERR_PNPM_MISSING_PACKAGE_INDEX_FILE", "message": "Failed to find package index file for x"}}"#,
        )
        .expect_err("an error");
        assert_eq!(error, "dependencies not installed — run pnpm install");
    }

    #[test]
    fn release_age_reads_each_managers_unit() {
        // pnpm in minutes, npm in days.
        assert_eq!(
            parse_release_age("4320\n", 1),
            ReleaseAge::Set("3 days".to_owned())
        );
        assert_eq!(
            parse_release_age("90", 1),
            ReleaseAge::Set("90 minutes".to_owned())
        );
        assert_eq!(
            parse_release_age("1", 24 * 60),
            ReleaseAge::Set("1 day".to_owned())
        );
    }

    #[test]
    fn an_absent_release_age_is_unset() {
        assert_eq!(parse_release_age("undefined\n", 1), ReleaseAge::Unset);
        assert_eq!(parse_release_age("null\n", 24 * 60), ReleaseAge::Unset);
        assert_eq!(parse_release_age("0", 1), ReleaseAge::Unset);
    }
}
