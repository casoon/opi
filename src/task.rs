//! The task model.
//!
//! One entry the user can act on. Scripts populate it today; built-in actions
//! and tool checks join it later without a second type, because the interactive
//! list, the command line and the search are all meant to be views onto this
//! one abstraction rather than three parallel code paths.
//!
//! The rule this module exists to enforce: **the script name is not the user
//! interface.** A flat alphabetical list of raw names is what `opi` is for.

use crate::manifest::Manifest;
use crate::workspace::Member;

/// The section a task appears under.
///
/// Ordering is derived, so it follows the declaration order below — meaning
/// first, unknown prefixes after them, the catch-all last. Alphabetical
/// ordering is exactly the arrangement that makes a script list unusable.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Development,
    Build,
    Preview,
    Quality,
    /// An unrecognised prefix, carrying the prefix as its label.
    Custom(String),
    /// Scripts with no prefix and no recognised meaning.
    Other,
    /// A workspace member's scripts, under the member's name.
    ///
    /// Last, because the root's own scripts are what someone standing in the
    /// root reached for.
    Workspace(String),
}

impl Group {
    /// The heading shown above the group.
    ///
    /// Custom prefixes are capitalised for display so a `deploy` group does not
    /// sit visually below `Development` as lowercase text.
    pub fn label(&self) -> String {
        match self {
            Self::Development => "Development".to_owned(),
            Self::Build => "Build".to_owned(),
            Self::Preview => "Preview".to_owned(),
            Self::Quality => "Quality".to_owned(),
            Self::Custom(prefix) => capitalize(prefix),
            Self::Other => "Other".to_owned(),
            // A package name is an identifier, not prose; shown unchanged.
            Self::Workspace(name) => name.clone(),
        }
    }

    /// Classifies a script by the segment before its first `:`.
    ///
    /// `build:landings:watch` belongs to Build — only the first segment counts.
    ///
    /// `siblings` is every script name in the project, which is what lets a
    /// standalone `deploy` join the `deploy:*` group instead of being stranded
    /// in the catch-all away from its own variants.
    fn of<'a>(script: &str, siblings: impl Iterator<Item = &'a str>) -> Self {
        let base = script.split(':').next().unwrap_or(script);
        match base {
            "dev" | "start" | "serve" | "watch" => Self::Development,
            "build" | "bundle" | "compile" => Self::Build,
            "preview" => Self::Preview,
            "check" | "lint" | "format" | "fmt" | "test" | "audit" | "typecheck" | "type-check"
            | "types" => Self::Quality,
            // An unknown prefix is a deliberate grouping by the author.
            _ if script.contains(':') => Self::Custom(base.to_owned()),
            // A standalone script that heads a family belongs with that family.
            _ if siblings.into_iter().any(|other| {
                other
                    .strip_prefix(base)
                    .is_some_and(|rest| rest.starts_with(':'))
            }) =>
            {
                Self::Custom(base.to_owned())
            }
            // Otherwise: a one-entry group per stray script is worse than a
            // shared catch-all.
            _ => Self::Other,
        }
    }
}

/// Uppercases the first character, leaving the rest alone.
fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// One entry the user can run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// The script name, shown as-is — `dev:landings`, not a prettified form.
    pub name: String,
    /// From `scripts-info`. Absent stays absent: the raw command must never be
    /// shown here, since that reintroduces the technical surface being removed.
    pub description: Option<String>,
    /// The underlying command, for execution — not for display.
    pub command: String,
    pub group: Group,
    /// The workspace member this belongs to, if any. Running it needs the
    /// package manager to be pointed at that member.
    pub workspace: Option<String>,
}

impl Task {
    /// Builds the task list from a manifest's scripts.
    ///
    /// Sorted by group, then by name. Within a group that puts the base script
    /// ahead of its variants on its own, because `build` sorts before
    /// `build:landings`.
    pub fn from_manifest(manifest: &Manifest) -> Vec<Self> {
        let mut tasks: Vec<Self> = manifest
            .scripts
            .iter()
            .filter(|(name, _)| !is_implicit_lifecycle(name, manifest))
            .map(|(name, command)| Self {
                name: name.clone(),
                description: manifest.description(name).map(str::to_owned),
                command: command.clone(),
                group: Group::of(name, manifest.scripts.keys().map(String::as_str)),
                workspace: None,
            })
            .collect();

        tasks.sort_by(|a, b| a.group.cmp(&b.group).then_with(|| a.name.cmp(&b.name)));
        tasks
    }

    /// Builds the task list from a manifest and its workspace members.
    ///
    /// Member scripts keep their own names and are grouped under the member,
    /// so `dev` appears once per package rather than being renamed into
    /// something like `blog:dev` that matches nothing in that package.
    pub fn from_workspace(manifest: &Manifest, members: &[Member]) -> Vec<Self> {
        let mut tasks = Self::from_manifest(manifest);

        for member in members {
            let mut member_tasks: Vec<Self> = member
                .manifest
                .scripts
                .iter()
                .filter(|(name, _)| !is_implicit_lifecycle(name, &member.manifest))
                .map(|(name, command)| Self {
                    name: name.clone(),
                    description: member.manifest.description(name).map(str::to_owned),
                    command: command.clone(),
                    group: Group::Workspace(member.name.clone()),
                    workspace: Some(member.name.clone()),
                })
                .collect();
            member_tasks.sort_by(|a, b| a.name.cmp(&b.name));
            tasks.extend(member_tasks);
        }

        tasks
    }
}

/// Whether a script is a lifecycle hook npm runs on its own.
///
/// `prebuild` alongside `build` is machinery, not a menu entry — listing it
/// invites running it directly, which is never what someone wants.
///
/// The guard against a recognised group matters more than it looks: `preview`
/// strips to `view`, so a project with a `view` script would otherwise lose its
/// preview entry.
fn is_implicit_lifecycle(name: &str, manifest: &Manifest) -> bool {
    let siblings = manifest.scripts.keys().map(String::as_str);
    if !matches!(Group::of(name, siblings), Group::Custom(_) | Group::Other) {
        return false;
    }

    ["pre", "post"].iter().any(|hook| {
        name.strip_prefix(hook)
            .is_some_and(|base| !base.is_empty() && manifest.scripts.contains_key(base))
    })
}

/// Finds the task `query` names.
///
/// A bare name is matched exactly, and the root's own scripts win — a project
/// with a `dev` script must keep `opi dev` meaning that one.
///
/// `member/script` addresses a workspace member. Every workspace repository
/// measured had root and member scripts sharing names, so without a way to
/// say which one is meant, a member's `dev` would be unreachable from the
/// command line. No script name among 382 real ones contained a `/`, and an
/// exact match is tried first regardless, so the syntax takes nothing away.
pub fn find<'a>(tasks: &'a [Task], query: &str) -> Option<&'a Task> {
    if let Some(task) = tasks.iter().find(|task| task.name == query) {
        return Some(task);
    }

    // Split at the last slash: a scoped package name contains one of its own,
    // so "@casoon/blog/dev" is the member "@casoon/blog" and the script "dev".
    let (member, script) = query.rsplit_once('/')?;
    tasks.iter().find(|task| {
        task.name == script
            && task.workspace.as_deref().is_some_and(|name| {
                // Scoped packages are addressable by their short name too:
                // "ui/build" reaches "@casoon/ui".
                name == member
                    || name
                        .rsplit_once('/')
                        .is_some_and(|(_, short)| short == member)
            })
    })
}

/// Task names close enough to `query` to be worth offering, best first.
///
/// Substring matches come first — someone typing `land` for `build:landings`
/// wants that offered, and edit distance alone would never surface it.
pub fn suggestions(tasks: &[Task], query: &str) -> Vec<String> {
    // An addressed query is compared on its script part. Measuring "blog/previw"
    // against "preview" counts the member name as six typos and offers nothing.
    let query = query
        .rsplit_once('/')
        .map_or(query, |(_, script)| script)
        .to_lowercase();
    // A short name tolerates fewer typos than a long one.
    let tolerance = (query.chars().count() / 3).max(1);

    // A member's script is only runnable in its addressed form, so that is
    // what a suggestion has to offer.
    let addressed = |task: &Task| match &task.workspace {
        Some(member) => format!("{member}/{}", task.name),
        None => task.name.clone(),
    };

    let mut scored: Vec<(usize, String)> = tasks
        .iter()
        .filter_map(|task| {
            let name = task.name.to_lowercase();
            if name.contains(&query) || query.contains(&name) {
                return Some((0, addressed(task)));
            }
            let distance = edit_distance(&name, &query);
            (distance <= tolerance).then_some((distance, addressed(task)))
        })
        .collect();

    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    scored.dedup_by(|a, b| a.1 == b.1);
    scored.into_iter().take(3).map(|(_, name)| name).collect()
}

/// Optimal string alignment distance.
///
/// Levenshtein alone is the wrong measure here: it charges 2 for a
/// transposition, so `biuld` scores as far from `build` as a two-letter
/// mistake — and transposing two letters is the typo people actually make.
/// This counts an adjacent swap as one edit.
///
/// Script names are short, so the full matrix is cheaper than being clever.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut distance = vec![vec![0usize; b.len() + 1]; a.len() + 1];

    for (i, row) in distance.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in distance[0].iter_mut().enumerate() {
        *cell = j;
    }

    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (distance[i - 1][j] + 1)
                .min(distance[i][j - 1] + 1)
                .min(distance[i - 1][j - 1] + cost);

            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(distance[i - 2][j - 2] + 1);
            }

            distance[i][j] = best;
        }
    }

    distance[a.len()][b.len()]
}

/// Groups a sorted task list into its sections, preserving order.
pub fn by_group(tasks: &[Task]) -> Vec<(&Group, &[Task])> {
    let mut sections = Vec::new();
    let mut rest = tasks;

    while let Some(first) = rest.first() {
        let len = rest
            .iter()
            .take_while(|task| task.group == first.group)
            .count();
        let (section, remainder) = rest.split_at(len);
        sections.push((&first.group, section));
        rest = remainder;
    }

    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(json: &str) -> Manifest {
        serde_json::from_str(json).expect("parse fixture")
    }

    fn names(tasks: &[Task]) -> Vec<&str> {
        tasks.iter().map(|task| task.name.as_str()).collect()
    }

    #[test]
    fn prefix_before_the_colon_becomes_the_group() {
        let tasks = Task::from_manifest(&manifest(
            r#"{"scripts":{"dev":"x","dev:landings":"x","build":"x"}}"#,
        ));
        let groups: Vec<String> = tasks.iter().map(|task| task.group.label()).collect();
        assert_eq!(groups, ["Development", "Development", "Build"]);
    }

    #[test]
    fn only_the_first_segment_counts() {
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{"build:landings:watch":"x"}}"#));
        assert_eq!(tasks[0].group, Group::Build);
    }

    #[test]
    fn groups_come_in_meaning_order_not_alphabetical() {
        let tasks = Task::from_manifest(&manifest(
            r#"{"scripts":{"check":"x","build":"x","dev":"x","preview":"x"}}"#,
        ));
        assert_eq!(names(&tasks), ["dev", "build", "preview", "check"]);
    }

    #[test]
    fn base_script_sorts_ahead_of_its_variants() {
        let tasks = Task::from_manifest(&manifest(
            r#"{"scripts":{"build:landings":"x","build":"x","build:app":"x"}}"#,
        ));
        assert_eq!(names(&tasks), ["build", "build:app", "build:landings"]);
    }

    #[test]
    fn a_standalone_script_joins_its_own_family() {
        // Found against a real project: "deploy" sat in the catch-all while
        // "deploy:blog" and "deploy:starter" formed a group without it.
        let tasks = Task::from_manifest(&manifest(
            r#"{"scripts":{"deploy":"x","deploy:blog":"x","deploy:starter":"x"}}"#,
        ));
        assert!(
            tasks
                .iter()
                .all(|task| task.group == Group::Custom("deploy".to_owned()))
        );
        assert_eq!(names(&tasks), ["deploy", "deploy:blog", "deploy:starter"]);
        assert_eq!(by_group(&tasks).len(), 1);
    }

    #[test]
    fn a_standalone_script_without_a_family_stays_in_the_catch_all() {
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{"deploy":"x","dev":"x"}}"#));
        let deploy = tasks
            .iter()
            .find(|task| task.name == "deploy")
            .expect("deploy");
        assert_eq!(deploy.group, Group::Other);
    }

    #[test]
    fn a_partial_name_match_does_not_pull_a_script_into_a_family() {
        // "dep" must not be captured by "deploy:blog" — the sibling has to
        // continue with a colon, not just start with the same letters.
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{"dep":"x","deploy:blog":"x"}}"#));
        let dep = tasks.iter().find(|task| task.name == "dep").expect("dep");
        assert_eq!(dep.group, Group::Other);
    }

    #[test]
    fn custom_group_labels_are_capitalised() {
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{"secrets:scan":"x"}}"#));
        assert_eq!(tasks[0].group.label(), "Secrets");
    }

    #[test]
    fn unknown_prefix_forms_its_own_group() {
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{"db:migrate":"x"}}"#));
        assert_eq!(tasks[0].group, Group::Custom("db".to_owned()));
        assert_eq!(tasks[0].group.label(), "Db");
    }

    #[test]
    fn unknown_standalone_scripts_share_the_catch_all() {
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{"prepare":"x","release":"x"}}"#));
        assert!(tasks.iter().all(|task| task.group == Group::Other));
        assert_eq!(by_group(&tasks).len(), 1, "one shared group, not two");
    }

    #[test]
    fn catch_all_sorts_last() {
        let tasks = Task::from_manifest(&manifest(
            r#"{"scripts":{"prepare":"x","db:seed":"x","dev":"x"}}"#,
        ));
        assert_eq!(names(&tasks), ["dev", "db:seed", "prepare"]);
    }

    #[test]
    fn lifecycle_hooks_with_a_main_script_are_hidden() {
        let tasks = Task::from_manifest(&manifest(
            r#"{"scripts":{"build":"x","prebuild":"x","postbuild":"x"}}"#,
        ));
        assert_eq!(names(&tasks), ["build"]);
    }

    #[test]
    fn lifecycle_hooks_without_a_main_script_are_kept() {
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{"postinstall":"x"}}"#));
        assert_eq!(names(&tasks), ["postinstall"]);
        assert_eq!(tasks[0].group, Group::Other);
    }

    #[test]
    fn preview_survives_a_view_script() {
        // "preview" strips to "view"; a naive hook check would drop it.
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{"preview":"x","view":"x"}}"#));
        assert!(names(&tasks).contains(&"preview"));
    }

    #[test]
    fn descriptions_come_from_scripts_info() {
        let tasks = Task::from_manifest(&manifest(
            r#"{"scripts":{"dev":"astro dev"},"scripts-info":{"dev":"Start dev server"}}"#,
        ));
        assert_eq!(tasks[0].description.as_deref(), Some("Start dev server"));
    }

    #[test]
    fn a_missing_description_stays_empty_rather_than_showing_the_command() {
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{"dev":"astro dev --verbose"}}"#));
        assert_eq!(tasks[0].description, None);
        assert_eq!(tasks[0].command, "astro dev --verbose");
    }

    #[test]
    fn descriptions_without_a_script_are_ignored() {
        let tasks = Task::from_manifest(&manifest(
            r#"{"scripts":{"dev":"x"},"scripts-info":{"dev":"Start","gone":"Stale"}}"#,
        ));
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn no_scripts_yields_no_tasks() {
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{}}"#));
        assert!(tasks.is_empty());
        assert!(by_group(&tasks).is_empty());
    }

    fn workspace_tasks() -> Vec<Task> {
        let root = manifest(r#"{"scripts":{"dev":"x","build":"x"}}"#);
        let member = Member {
            name: "@casoon/blog".to_owned(),
            manifest: manifest(r#"{"scripts":{"dev":"x","preview":"x"}}"#),
        };
        Task::from_workspace(&root, &[member])
    }

    #[test]
    fn a_bare_name_reaches_the_root_script() {
        let tasks = workspace_tasks();
        let found = find(&tasks, "dev").expect("dev");
        assert_eq!(found.workspace, None, "the root's own script wins");
    }

    #[test]
    fn an_addressed_name_reaches_the_member() {
        let tasks = workspace_tasks();
        let found = find(&tasks, "@casoon/blog/dev").expect("member dev");
        assert_eq!(found.workspace.as_deref(), Some("@casoon/blog"));
    }

    #[test]
    fn a_scoped_member_answers_to_its_short_name() {
        let tasks = workspace_tasks();
        let found = find(&tasks, "blog/dev").expect("short form");
        assert_eq!(found.workspace.as_deref(), Some("@casoon/blog"));
    }

    #[test]
    fn a_member_only_script_needs_no_address() {
        let tasks = workspace_tasks();
        let found = find(&tasks, "preview").expect("preview");
        assert_eq!(
            found.workspace.as_deref(),
            Some("@casoon/blog"),
            "nothing at the root shadows it"
        );
    }

    #[test]
    fn an_unknown_address_finds_nothing() {
        let tasks = workspace_tasks();
        assert!(find(&tasks, "shop/dev").is_none());
        assert!(find(&tasks, "blog/nope").is_none());
    }

    #[test]
    fn member_scripts_are_grouped_under_the_member() {
        let tasks = workspace_tasks();
        let member_group = Group::Workspace("@casoon/blog".to_owned());
        assert_eq!(
            tasks
                .iter()
                .filter(|task| task.group == member_group)
                .count(),
            2
        );
        // Workspace groups come after everything the root owns.
        assert_eq!(tasks.last().expect("last").group, member_group);
    }

    #[test]
    fn suggestions_offer_the_runnable_form_of_a_member_script() {
        let tasks = workspace_tasks();
        assert!(
            suggestions(&tasks, "previw").contains(&"@casoon/blog/preview".to_owned()),
            "suggesting a bare name that does not run would be useless"
        );
    }

    #[test]
    fn a_typo_in_an_addressed_name_is_still_caught() {
        let tasks = workspace_tasks();
        assert!(
            suggestions(&tasks, "blog/previw").contains(&"@casoon/blog/preview".to_owned()),
            "the member name must not be counted as part of the typo"
        );
    }

    #[test]
    fn suggestions_offer_a_near_miss() {
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{"build":"x","dev":"x"}}"#));
        assert_eq!(suggestions(&tasks, "biuld"), ["build"]);
    }

    #[test]
    fn a_transposition_counts_as_one_edit() {
        assert_eq!(edit_distance("biuld", "build"), 1);
        assert_eq!(edit_distance("build", "build"), 0);
        assert_eq!(edit_distance("build", "bulid"), 1);
    }

    #[test]
    fn suggestions_offer_substring_matches_edit_distance_would_miss() {
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{"build:landings":"x"}}"#));
        assert_eq!(suggestions(&tasks, "land"), ["build:landings"]);
    }

    #[test]
    fn suggestions_stay_silent_when_nothing_is_close() {
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{"build":"x","dev":"x"}}"#));
        assert!(suggestions(&tasks, "qqqqqqqq").is_empty());
    }

    #[test]
    fn suggestions_are_capped() {
        let tasks = Task::from_manifest(&manifest(
            r#"{"scripts":{"test:a":"x","test:b":"x","test:c":"x","test:d":"x"}}"#,
        ));
        assert_eq!(suggestions(&tasks, "test").len(), 3);
    }

    #[test]
    fn suggestions_ignore_case() {
        let tasks = Task::from_manifest(&manifest(r#"{"scripts":{"Build":"x"}}"#));
        assert_eq!(suggestions(&tasks, "build"), ["Build"]);
    }

    #[test]
    fn sections_cover_every_task_exactly_once() {
        let tasks = Task::from_manifest(&manifest(
            r#"{"scripts":{"dev":"x","dev:api":"x","build":"x","db:seed":"x","prepare":"x"}}"#,
        ));
        let sections = by_group(&tasks);
        assert_eq!(sections.len(), 4);
        assert_eq!(
            sections.iter().map(|(_, tasks)| tasks.len()).sum::<usize>(),
            tasks.len()
        );
    }
}
