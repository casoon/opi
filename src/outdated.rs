//! Dependencies with newer versions.
//!
//! The plan for this called for a `taze` adapter. Measured across 133 real
//! projects, `taze` appeared in none of them, while every package manager
//! ships `outdated` and emits JSON — so that is what this reads.
//!
//! The value here is not the list, which the package manager already prints.
//! It is the separation: "four safe, one major" is a decision, a column of
//! version numbers is homework.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::project::PackageManager;

/// How far a version moves, and therefore how much it can break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Jump {
    Patch,
    Minor,
    Major,
}

impl Jump {
    pub fn label(self) -> &'static str {
        match self {
            Self::Patch => "patch",
            Self::Minor => "minor",
            Self::Major => "major",
        }
    }

    /// Whether this is the kind that reliably breaks something.
    ///
    /// Major is never offered as part of a bulk update for exactly this
    /// reason; it is a decision per package.
    pub fn breaking(self) -> bool {
        self == Self::Major
    }

    /// Classifies the move from `current` to `latest`.
    ///
    /// A version that does not parse as three numbers is treated as major:
    /// unknown is the cautious answer, not the optimistic one.
    fn between(current: &str, latest: &str) -> Self {
        let (Some(from), Some(to)) = (parts(current), parts(latest)) else {
            return Self::Major;
        };
        // A pre-1.0 minor bump breaks as readily as a major one, which is how
        // the ecosystem actually treats 0.x.
        if from.0 != to.0 || (from.0 == 0 && from.1 != to.1) {
            Self::Major
        } else if from.1 != to.1 {
            Self::Minor
        } else {
            Self::Patch
        }
    }
}

/// Splits a version into its three numbers, ignoring any pre-release suffix.
fn parts(version: &str) -> Option<(u64, u64, u64)> {
    let core = version
        .trim_start_matches(['^', '~', 'v', '='])
        .split(['-', '+'])
        .next()?;
    let mut numbers = core.split('.').map(str::parse::<u64>);
    Some((
        numbers.next()?.ok()?,
        numbers.next().transpose().ok()?.unwrap_or(0),
        numbers.next().transpose().ok()?.unwrap_or(0),
    ))
}

/// One package that has moved on.
#[derive(Debug, Clone)]
pub struct Update {
    pub name: String,
    pub current: String,
    pub latest: String,
    pub jump: Jump,
}

#[derive(Debug, Deserialize)]
struct RawEntry {
    #[serde(default)]
    current: String,
    #[serde(default)]
    latest: String,
}

/// Why the list could not be produced.
#[derive(Debug)]
pub enum OutdatedError {
    Failed(String),
}

impl std::fmt::Display for OutdatedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed(reason) => f.write_str(reason),
        }
    }
}

/// Asks the package manager what is out of date.
pub fn run(manager: PackageManager, root: &Path) -> Result<Vec<Update>, OutdatedError> {
    let output = Command::new(manager.program())
        .args(["outdated", "--json"])
        .current_dir(root)
        .output()
        .map_err(|error| OutdatedError::Failed(format!("could not run {manager}: {error}")))?;

    let text = String::from_utf8_lossy(&output.stdout);
    // Every manager exits non-zero when something is outdated, so the status
    // says nothing; an empty body is the real "nothing to report".
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    let raw: BTreeMap<String, RawEntry> = serde_json::from_str(&text).map_err(|error| {
        OutdatedError::Failed(format!("could not read {manager}'s output: {error}"))
    })?;

    let mut updates: Vec<Update> = raw
        .into_iter()
        .filter(|(_, entry)| !entry.current.is_empty() && entry.current != entry.latest)
        .map(|(name, entry)| Update {
            jump: Jump::between(&entry.current, &entry.latest),
            name,
            current: entry.current,
            latest: entry.latest,
        })
        .collect();

    // Riskiest last: the safe ones are what someone acts on.
    updates.sort_by(|a, b| a.jump.cmp(&b.jump).then_with(|| a.name.cmp(&b.name)));
    Ok(updates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patch_is_told_from_a_minor_and_a_major() {
        assert_eq!(Jump::between("1.2.3", "1.2.4"), Jump::Patch);
        assert_eq!(Jump::between("1.2.3", "1.3.0"), Jump::Minor);
        assert_eq!(Jump::between("1.2.3", "2.0.0"), Jump::Major);
    }

    #[test]
    fn a_pre_one_minor_counts_as_breaking() {
        // 0.x minors break as readily as majors, which is how the ecosystem
        // treats them in practice.
        assert_eq!(Jump::between("0.44.2", "0.46.0"), Jump::Major);
        assert_eq!(Jump::between("0.44.2", "0.44.3"), Jump::Patch);
    }

    #[test]
    fn an_unparseable_version_is_treated_as_breaking() {
        // Unknown is the cautious answer, not the optimistic one.
        assert_eq!(Jump::between("next", "1.0.0"), Jump::Major);
        assert_eq!(Jump::between("1.0.0", "canary"), Jump::Major);
    }

    #[test]
    fn prefixes_and_pre_releases_do_not_confuse_the_comparison() {
        assert_eq!(Jump::between("^1.2.3", "~1.2.9"), Jump::Patch);
        assert_eq!(Jump::between("1.2.3", "1.2.4-beta.1"), Jump::Patch);
        assert_eq!(parts("v2.1"), Some((2, 1, 0)));
    }

    #[test]
    fn only_major_counts_as_breaking() {
        assert!(Jump::Major.breaking());
        assert!(!Jump::Minor.breaking());
        assert!(!Jump::Patch.breaking());
    }

    #[test]
    fn the_real_pnpm_shape_is_read() {
        // Taken from a real `pnpm outdated --json` run.
        let json = r#"{"@biomejs/biome":{"current":"2.5.13","latest":"2.5.14","wanted":"2.5.13",
            "dependencyType":"devDependencies"},
            "fallow":{"current":"3.24.1","latest":"3.27.0","wanted":"3.24.1"}}"#;
        let raw: BTreeMap<String, RawEntry> = serde_json::from_str(json).expect("parse");
        assert_eq!(raw.len(), 2);
        assert_eq!(
            Jump::between(
                &raw["@biomejs/biome"].current,
                &raw["@biomejs/biome"].latest
            ),
            Jump::Patch
        );
        assert_eq!(
            Jump::between(&raw["fallow"].current, &raw["fallow"].latest),
            Jump::Minor
        );
    }
}
