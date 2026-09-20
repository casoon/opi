//! Dependencies with newer versions.
//!
//! The plan for this called for a `taze` adapter. Measured across 133 real
//! projects, `taze` appeared in none of them, while npm and pnpm both ship
//! `outdated` and emit the same JSON map — so that is what this reads.
//!
//! The other two do not: bun ignores `--json` and prints a table, and yarn
//! emits its own line-delimited shape where it still has the command at all.
//!
//! Rust arrives through `cargo outdated`, which answers the same question and
//! fills the same [`Update`] — a second source, not a second model. [`Jump`]
//! never learns where a version came from.
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
    /// Whether the declared range already allows something newer — npm's
    /// `wanted` against its `current`.
    ///
    /// This is the choice the user actually makes, and it is not the same as
    /// safe against breaking. Measured on one project: three dependencies,
    /// two of them "safe", and `pnpm update` moved none of them, because an
    /// exact pin and a `~` range each already had what they asked for. As a
    /// line to retype nobody notices; as a key it would be an action that does
    /// nothing and reports success.
    ///
    /// Always false for Rust: `cargo update` writes the lockfile, so nothing
    /// is offered there and the question is never asked.
    pub in_range: bool,
}

/// One entry of `npm`/`pnpm outdated --json`.
///
/// `-r` adds a `dependentPackages` list to each; everything beyond these two
/// fields is ignored, so the parser is the same either way.
///
/// **pnpm's JSON keys by package name, so it can hold one entry per name.**
/// A crate at two versions in two members — `glob` 9 in one, 10 in the other —
/// loses one of them here, though `pnpm outdated -r` prints both rows in its
/// table. Which packages need attention is still right, and `pnpm update -r`
/// still moves both; only the version shown is one of two. Not worked around,
/// because naming the one member the JSON happens to carry would claim the
/// other is fine.
#[derive(Debug, Deserialize)]
struct RawEntry {
    #[serde(default)]
    current: String,
    #[serde(default)]
    latest: String,
    /// The newest version the declared range allows.
    #[serde(default)]
    wanted: String,
}

/// Why the list could not be produced.
#[derive(Debug)]
pub enum OutdatedError {
    /// There is no `package.json`, so there is nothing here to be out of date.
    ///
    /// Never returned by `run`; see `audit::AuditError::NoManifest` for why it
    /// lives here anyway.
    NoManifest,
    /// The package manager has no `outdated` output `opi` knows how to read.
    Unsupported(PackageManager),
    /// `cargo outdated` is not installed.
    ///
    /// An external subcommand like `cargo audit`, so its absence is the
    /// ordinary case rather than a broken setup.
    NotInstalled,
    Failed(String),
}

impl std::fmt::Display for OutdatedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Only about this section. Rust is answered by the one below it,
            // so claiming opi checks npm alone stopped being true.
            Self::NoManifest => f.write_str("no package.json here"),
            Self::Unsupported(manager) => {
                write!(f, "opi cannot read {manager}'s outdated output")
            }
            Self::NotInstalled => {
                f.write_str("cargo outdated is not installed; cargo install cargo-outdated adds it")
            }
            Self::Failed(reason) => f.write_str(reason),
        }
    }
}

/// Asks the package manager what is out of date.
///
/// `workspace` says whether the project declares one, which only pnpm needs
/// told: without `-r` it answers for the root `package.json` alone. Measured
/// on a real repository, that was 2 of 11 outdated packages — and where the
/// root declares no dependencies of its own it answers `{}`, so `--updates`
/// said "everything is current" over nine stale ones. A false acquittal is
/// worse than no answer.
///
/// npm needs nothing: it walks the installed tree rather than the manifests,
/// so it already sees every member. Measured both ways on a two-member
/// workspace, with identical results.
pub fn run(
    manager: PackageManager,
    root: &Path,
    workspace: bool,
) -> Result<Vec<Update>, OutdatedError> {
    // bun prints a table whether or not `--json` is passed, and yarn reports
    // line by line in a shape of its own — yarn 2 and newer dropped the
    // command altogether. Reading either as npm's map fails, and a parse
    // error reads like a broken project rather than a missing feature.
    if !matches!(manager, PackageManager::Npm | PackageManager::Pnpm) {
        return Err(OutdatedError::Unsupported(manager));
    }

    let mut args = vec!["outdated"];
    if workspace && matches!(manager, PackageManager::Pnpm) {
        args.push("-r");
    }
    args.push("--json");

    let output = Command::new(manager.program())
        .args(&args)
        .current_dir(root)
        .output()
        .map_err(|error| OutdatedError::Failed(format!("could not run {manager}: {error}")))?;

    let text = String::from_utf8_lossy(&output.stdout);
    // Every manager exits non-zero when something is outdated, so the status
    // says nothing; an empty body is the real "nothing to report".
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    let raw: BTreeMap<String, RawEntry> =
        serde_json::from_str(json_from(&text)).map_err(|error| {
            OutdatedError::Failed(format!("could not read {manager}'s output: {error}"))
        })?;

    let mut updates: Vec<Update> = raw
        .into_iter()
        .filter(|(_, entry)| !entry.current.is_empty() && entry.current != entry.latest)
        .map(|(name, entry)| Update {
            jump: Jump::between(&entry.current, &entry.latest),
            in_range: !entry.wanted.is_empty() && entry.wanted != entry.current,
            name,
            current: entry.current,
            latest: entry.latest,
        })
        .collect();

    // Riskiest last: the safe ones are what someone acts on.
    updates.sort_by(|a, b| a.jump.cmp(&b.jump).then_with(|| a.name.cmp(&b.name)));
    Ok(updates)
}

/// One line of `cargo outdated --format json`.
///
/// A workspace emits **one object per member**, newline separated, so the
/// output is not a single JSON document and is read line by line. Measured on
/// cargo-outdated 0.19.0 against a two-member workspace.
#[derive(Debug, Deserialize)]
struct CargoReport {
    #[serde(default)]
    dependencies: Vec<CargoEntry>,
}

#[derive(Debug, Deserialize)]
struct CargoEntry {
    #[serde(default)]
    name: String,
    /// What the lockfile resolved to. Named `project`, not `current`.
    #[serde(default)]
    project: String,
    #[serde(default)]
    latest: String,
}

/// Asks `cargo outdated` what has moved on.
///
/// `--root-deps-only` is what makes this the same question npm answers. Without
/// it every entry is a transitive crate — measured on one real project, all 17
/// were, named `parent->child` and none of them in any manifest. A list nobody
/// can act on is worse than no list.
///
/// `--workspace` covers the members, because `opi` treats the workspace root as
/// the project. A plain package is unaffected by it.
///
/// `compat`, the latest semver-compatible version, is deliberately ignored.
/// It reports what the *requirement* allows — a pinned `=1.0.100` shows `---`
/// though 1.0.151 is compatible — while [`Jump`] answers the question actually
/// being asked, and answers it the same way for both ecosystems.
pub fn cargo(root: &Path) -> Result<Vec<Update>, OutdatedError> {
    if !crate::cargo::has_subcommand("outdated") {
        return Err(OutdatedError::NotInstalled);
    }

    let output = Command::new("cargo")
        .args([
            "outdated",
            "--workspace",
            "--root-deps-only",
            "--format",
            "json",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| OutdatedError::Failed(format!("could not run cargo outdated: {error}")))?;

    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() {
        // It copies the project to a temporary directory and resolves there,
        // so a path dependency pointing outside the workspace stops it — seen
        // on a real repository here. That goes to stderr with nothing on
        // stdout, and relaying it beats a parse error about nothing.
        let reason = String::from_utf8_lossy(&output.stderr);
        let reason = reason
            .lines()
            .find(|line| line.trim_start().starts_with("error"))
            .or_else(|| reason.lines().next())
            .unwrap_or("no output")
            .trim()
            .to_owned();
        if reason.is_empty() || reason == "no output" {
            return Ok(Vec::new());
        }
        return Err(OutdatedError::Failed(format!("cargo outdated: {reason}")));
    }

    Ok(read_cargo(&text))
}

/// The document inside output that may carry warnings ahead of it.
///
/// pnpm writes its own warnings to **stdout**, not stderr — a slow registry
/// produces `‼ WARN‼ Request took 11084ms: …` in front of the JSON, and
/// parsing the whole text then fails with "expected value at line 1 column 1".
/// Seen on a real repository, and only when the network was slow enough, which
/// is what made it survive the first round of testing.
///
/// The first line that starts a document wins; everything before it is the
/// noise. A warning that itself began with `{` would still break this, and
/// would be reported the way it is today.
fn json_from(text: &str) -> &str {
    text.char_indices()
        .find(|(_, character)| *character == '{')
        .map_or(text, |(at, _)| &text[at..])
}

/// How an update is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Apply {
    /// Take what the declared ranges already allow. Writes no `package.json`.
    InRange,
    /// Raise the ranges for the named packages.
    ///
    /// Named, never blanket: `pnpm update --latest` with no names ignores the
    /// range *and* the risk, and pulled a 5 → 6 major in the measurement. The
    /// safe ones are passed by name so the majors stay out, as
    /// [`Jump::breaking`] requires.
    Latest,
}

/// Whether this manager can raise ranges without rewriting what it was not
/// asked to.
///
/// pnpm's `update --latest <names>` keeps the operator — exact stays exact,
/// `~` stays `~`. npm has no such command: `npm install <name>@<version>`
/// turns `2.1.2` into `^2.1.3` and `~1.0.0` into `^1.1.1`. An exact pin is a
/// statement, and rewriting it in passing is the silent edit this whole area
/// refuses to make, so the option is not offered there rather than offered
/// badly.
pub fn can_raise(manager: PackageManager) -> bool {
    matches!(manager, PackageManager::Pnpm)
}

/// Runs the package manager's update and lets it speak for itself.
///
/// `opi` writes nothing — not `package.json`, not a lockfile, not
/// `pnpm-workspace.yaml`. Catalogs, overrides and `workspace:` protocols stay
/// the problem of the tool that understands them, which is the whole reason
/// this is allowed to exist at all.
///
/// The child inherits the terminal, so a peer-dependency conflict arrives as
/// the manager's own message rather than as a shrug. A non-zero exit is
/// returned rather than translated into "done".
pub fn apply(
    manager: PackageManager,
    root: &Path,
    workspace: bool,
    mode: Apply,
    names: &[String],
) -> Result<(), OutdatedError> {
    let mut args = vec!["update".to_owned()];
    // Without it a package that hangs only in a member is not reached, and
    // pnpm still says "Already up to date" — measured.
    if workspace && matches!(manager, PackageManager::Pnpm) {
        args.push("-r".to_owned());
    }
    if mode == Apply::Latest {
        args.push("--latest".to_owned());
        args.extend_from_slice(names);
    }

    let status = Command::new(manager.program())
        .args(&args)
        .current_dir(root)
        .status()
        .map_err(|error| OutdatedError::Failed(format!("could not run {manager}: {error}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(OutdatedError::Failed(format!(
            "{manager} update exited with {status}"
        )))
    }
}

/// Reads the newline-separated reports into updates.
fn read_cargo(text: &str) -> Vec<Update> {
    let mut updates: Vec<Update> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<CargoReport>(line).ok())
        .flat_map(|report| report.dependencies)
        .filter(|entry| {
            !entry.name.is_empty()
                && !entry.project.is_empty()
                && !entry.latest.is_empty()
                && entry.project != entry.latest
        })
        .map(|entry| Update {
            jump: Jump::between(&entry.project, &entry.latest),
            // Nothing is applied for Rust, so the question never arises.
            in_range: false,
            name: entry.name,
            current: entry.project,
            latest: entry.latest,
        })
        .collect();

    // A crate several members depend on is reported once per member. Sorting
    // by name first puts the copies together so they can be dropped; the
    // display order is restored below.
    updates.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.current.cmp(&b.current))
            .then_with(|| a.latest.cmp(&b.latest))
    });
    updates.dedup_by(|a, b| a.name == b.name && a.current == b.current && a.latest == b.latest);

    // Riskiest last, as on the npm side.
    updates.sort_by(|a, b| a.jump.cmp(&b.jump).then_with(|| a.name.cmp(&b.name)));
    updates
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

    /// Trimmed from a real `cargo outdated --workspace --root-deps-only
    /// --format json` run (cargo-outdated 0.19.0) on a two-member workspace.
    /// One object per member, newline separated — not one document.
    const CARGO_WORKSPACE: &str = concat!(
        r#"{"crate_name":"a","dependencies":[{"name":"serde_json","project":"1.0.100","compat":"---","latest":"1.0.151","kind":"Normal","platform":null}]}"#,
        "\n",
        r#"{"crate_name":"b","dependencies":[{"name":"glob","project":"0.3.0","compat":"---","latest":"0.3.4","kind":"Normal","platform":null},{"name":"serde_json","project":"1.0.100","compat":"---","latest":"1.0.151","kind":"Normal","platform":null}]}"#,
        "\n",
    );

    #[test]
    fn a_warning_printed_ahead_of_the_json_does_not_break_the_read() {
        // The bug this pins: pnpm writes its warnings to stdout, so a slow
        // registry put `‼ WARN‼ Request took 11084ms: …` in front of the
        // document and --updates answered "could not read pnpm's output".
        let noisy = "\u{203c} WARN\u{203c} Request took 11084ms: https://registry.npmjs.org/x\n{\"ms\":{\"current\":\"2.1.2\",\"wanted\":\"2.1.3\",\"latest\":\"2.1.3\"}}";
        let raw: BTreeMap<String, RawEntry> =
            serde_json::from_str(json_from(noisy)).expect("parse past the warning");
        assert_eq!(raw["ms"].latest, "2.1.3");
    }

    #[test]
    fn clean_output_is_untouched() {
        let json = r#"{"ms":{"current":"2.1.2","wanted":"2.1.3","latest":"2.1.3"}}"#;
        assert_eq!(json_from(json), json);
    }

    #[test]
    fn in_range_is_wanted_against_current_not_the_semver_jump() {
        // The distinction the menu rests on. Measured on a real project:
        // three dependencies, two of them "safe", and `pnpm update` moved
        // none — an exact pin and a `~` range each already had what they
        // asked for.
        let json = r#"{
            "ms": {"current":"2.1.2","wanted":"2.1.2","latest":"2.1.3"},
            "picocolors": {"current":"1.0.0","wanted":"1.1.1","latest":"1.1.1"}
        }"#;
        let raw: BTreeMap<String, RawEntry> = serde_json::from_str(json).expect("parse");
        let mut seen: Vec<(String, bool, Jump)> = raw
            .into_iter()
            .map(|(name, entry)| {
                (
                    name,
                    !entry.wanted.is_empty() && entry.wanted != entry.current,
                    Jump::between(&entry.current, &entry.latest),
                )
            })
            .collect();
        seen.sort();

        assert_eq!(seen[0].0, "ms");
        assert!(!seen[0].1, "a patch that the exact pin forbids");
        assert_eq!(seen[0].2, Jump::Patch, "safe, and still not applicable");

        assert_eq!(seen[1].0, "picocolors");
        assert!(seen[1].1, "inside the declared range");
    }

    #[test]
    fn only_pnpm_is_asked_to_raise_ranges() {
        // npm has no command that keeps the operator: `npm install x@1.1.1`
        // turns an exact 2.1.2 into ^2.1.3. An exact pin is a statement.
        assert!(can_raise(PackageManager::Pnpm));
        assert!(!can_raise(PackageManager::Npm));
        assert!(!can_raise(PackageManager::Yarn));
        assert!(!can_raise(PackageManager::Bun));
    }

    #[test]
    fn a_cargo_update_is_never_offered() {
        // `cargo update` writes the lockfile, which is the line this area does
        // not cross, so the in-range question is never asked for Rust.
        let text = r#"{"crate_name":"a","dependencies":[{"name":"glob","project":"0.3.0","compat":"---","latest":"0.3.4","kind":"Normal","platform":null}]}"#;
        assert!(!read_cargo(text)[0].in_range);
    }

    #[test]
    fn a_workspace_report_is_read_line_by_line() {
        // The bug this pins: cargo outdated emits one object per member rather
        // than one document, so reading the whole text as JSON fails on every
        // workspace.
        let updates = read_cargo(CARGO_WORKSPACE);
        assert_eq!(updates.len(), 2);
        assert!(updates.iter().any(|u| u.name == "glob"));
        assert!(updates.iter().any(|u| u.name == "serde_json"));
    }

    #[test]
    fn a_crate_two_members_share_is_listed_once() {
        let updates = read_cargo(CARGO_WORKSPACE);
        let named: Vec<&str> = updates
            .iter()
            .filter(|u| u.name == "serde_json")
            .map(|u| u.name.as_str())
            .collect();
        assert_eq!(named.len(), 1, "once, not once per member");
    }

    #[test]
    fn a_cargo_entry_fills_the_same_update_as_npm() {
        // The point of the second source: no second model, and Jump never
        // learns where the versions came from.
        let updates = read_cargo(CARGO_WORKSPACE);
        let glob = updates.iter().find(|u| u.name == "glob").unwrap();
        assert_eq!(glob.current, "0.3.0");
        assert_eq!(glob.latest, "0.3.4");
        assert_eq!(glob.jump, Jump::Patch);

        // 51 patch releases is still a patch jump. The number's size is not
        // the question; which position moved is.
        let json = updates.iter().find(|u| u.name == "serde_json").unwrap();
        assert_eq!(json.jump, Jump::Patch);
    }

    #[test]
    fn a_cargo_report_with_nothing_outdated_yields_nothing() {
        assert!(read_cargo(r#"{"crate_name":"opi","dependencies":[]}"#).is_empty());
    }

    #[test]
    fn an_unreadable_cargo_line_is_skipped_rather_than_fatal() {
        // One member failing to serialise must not cost the others.
        let text = format!("not json\n{CARGO_WORKSPACE}");
        assert_eq!(read_cargo(&text).len(), 2);
    }

    #[test]
    fn a_cargo_zero_x_minor_is_breaking_here_too() {
        // The risk the plan raised: cargo treats 0.12 → 0.13 as breaking. The
        // rule was already in Jump and already applies to both ecosystems, so
        // nothing needed changing — this pins that it stays that way.
        let text = r#"{"crate_name":"llmux","dependencies":[{"name":"reqwest","project":"0.12.28","compat":"---","latest":"0.13.5","kind":"Normal","platform":null}]}"#;
        assert_eq!(read_cargo(text)[0].jump, Jump::Major);
    }

    #[test]
    fn only_major_counts_as_breaking() {
        assert!(Jump::Major.breaking());
        assert!(!Jump::Minor.breaking());
        assert!(!Jump::Patch.breaking());
    }

    #[test]
    fn a_manager_opi_cannot_read_is_refused_rather_than_run() {
        // The guard has to sit in front of the process: a table parsed as JSON
        // would be reported as a broken project instead of a missing feature.
        for manager in [PackageManager::Bun, PackageManager::Yarn] {
            let error = run(manager, Path::new("."), false).expect_err("unsupported");
            assert!(
                matches!(error, OutdatedError::Unsupported(_)),
                "for {manager}"
            );
        }
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
