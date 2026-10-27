//! GitHub Actions references that a tag or branch can move.
//!
//! `uses: owner/repo@v4` runs whatever that tag points to on the day the
//! workflow runs; only a full 40-character commit SHA names one commit for
//! good. This reads `.github/workflows` line by line — a `uses:` line is not
//! ambiguous enough to need a YAML parser — and names each reference that is
//! not pinned. Local actions (`./…`) and Docker references (`docker://…`)
//! are not versions of a remote repository and are left out.

use std::fs;
use std::path::Path;

/// A `uses:` reference whose ref is not a commit SHA.
#[derive(Debug, PartialEq, Eq)]
pub struct Unpinned {
    /// Workflow file name, e.g. `ci.yml`.
    pub file: String,
    /// 1-based line of the `uses:` key.
    pub line: usize,
    /// The reference as written, e.g. `actions/checkout@v4`.
    pub reference: String,
}

/// Every unpinned `uses:` reference in the project's workflows.
///
/// A project without `.github/workflows` has none.
pub fn unpinned(root: &Path) -> Vec<Unpinned> {
    let Ok(entries) = fs::read_dir(root.join(".github/workflows")) else {
        return Vec::new();
    };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("yml" | "yaml")
            )
        })
        .collect();
    files.sort();

    files
        .iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().into_owned();
            Some(scan(&name, &fs::read_to_string(path).ok()?))
        })
        .flatten()
        .collect()
}

fn scan(file: &str, text: &str) -> Vec<Unpinned> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let reference = reference(line)?;
            let (_, git_ref) = reference.rsplit_once('@')?;
            let pinned = git_ref.len() == 40 && git_ref.bytes().all(|b| b.is_ascii_hexdigit());
            (!pinned).then(|| Unpinned {
                file: file.to_owned(),
                line: index + 1,
                reference,
            })
        })
        .collect()
}

/// The remote reference on a `uses:` line, if it has one.
fn reference(line: &str) -> Option<String> {
    let line = line.trim_start();
    let line = line.strip_prefix("- ").unwrap_or(line).trim_start();
    let value = line.strip_prefix("uses:")?;
    let value = value.split(" #").next()?.trim().trim_matches(['"', '\'']);
    (!value.starts_with("./") && !value.starts_with("docker://")).then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "11bd71901bbe5b1630ceea73d27597364c9af683";

    #[test]
    fn tags_and_branches_are_named_with_their_line() {
        let text = format!(
            "jobs:\n  build:\n    steps:\n      - uses: actions/checkout@v4\n      - uses: \"actions/cache@main\"\n      - uses: actions/setup-node@{SHA} # v4\n"
        );
        let found = scan("ci.yml", &text);
        assert_eq!(
            found,
            vec![
                Unpinned {
                    file: "ci.yml".into(),
                    line: 4,
                    reference: "actions/checkout@v4".into()
                },
                Unpinned {
                    file: "ci.yml".into(),
                    line: 5,
                    reference: "actions/cache@main".into()
                },
            ]
        );
    }

    #[test]
    fn local_docker_and_pinned_references_are_left_out() {
        let text = format!(
            "steps:\n  - uses: ./.github/actions/build\n  - uses: docker://alpine:3.20\n  - uses: a/b@{SHA}\n    with:\n      uses: nothing\n"
        );
        assert!(scan("ci.yml", &text).is_empty());
    }

    #[test]
    fn a_short_or_non_hex_ref_is_not_a_pin() {
        assert_eq!(scan("ci.yml", "      uses: a/b@11bd719\n").len(), 1);
        assert_eq!(
            scan("ci.yml", &format!("      uses: a/b@{}z\n", &SHA[..39])).len(),
            1
        );
    }

    #[test]
    fn workflows_are_read_from_the_project() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(unpinned(dir.path()).is_empty());
        let workflows = dir.path().join(".github/workflows");
        fs::create_dir_all(&workflows).expect("dirs");
        fs::write(workflows.join("b.yaml"), "- uses: x/y@v1\n").expect("write");
        fs::write(workflows.join("a.yml"), "- uses: x/z@v2\n").expect("write");
        fs::write(workflows.join("notes.txt"), "- uses: x/w@v3\n").expect("write");
        let found = unpinned(dir.path());
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].file, "a.yml");
        assert_eq!(found[1].file, "b.yaml");
    }
}
