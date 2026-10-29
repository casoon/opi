//! Git's index checks, without changing files or staging working-tree content.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use runemark::Verdict;

const MAX_BYTES: u64 = 500 * 1024;

pub fn repository(dir: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    output.status.success().then(|| {
        PathBuf::from(OsStr::from_bytes(
            output.stdout.strip_suffix(b"\n").unwrap_or(&output.stdout),
        ))
    })
}

pub fn check(dir: &Path) -> (Verdict, String) {
    match check_inner(dir) {
        Ok(output) if output.is_empty() => (Verdict::Passed, output),
        Ok(output) => (Verdict::Failed, output),
        Err(error) => (Verdict::ActionRequired, format!("git index check: {error}")),
    }
}

fn git(root: &Path, args: &[&str]) -> Result<Output, std::io::Error> {
    Command::new("git").current_dir(root).args(args).output()
}

fn check_inner(dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let root = repository(dir).ok_or("not a Git repository")?;
    let whitespace = git(
        &root,
        &[
            "-c",
            "core.whitespace=blank-at-eol,blank-at-eof,space-before-tab",
            "diff",
            "--cached",
            "--check",
        ],
    )?;
    let mut problems = Vec::new();
    if !whitespace.status.success() {
        if whitespace.status.code() != Some(2) {
            return Err("could not inspect staged whitespace and conflict markers".into());
        }
        problems.push("Staged whitespace errors or conflict markers detected.".to_owned());
        // Git prints the offending source after each diagnostic. Keep locations
        // and explanations, but never reproduce a source line containing secrets.
        problems.extend(
            String::from_utf8_lossy(&whitespace.stdout)
                .lines()
                .filter(|line| !line.starts_with('+'))
                .map(str::to_owned),
        );
    }

    let added = git(
        &root,
        &[
            "diff",
            "--cached",
            "--raw",
            "--no-abbrev",
            "--diff-filter=A",
            "-z",
            "--no-renames",
        ],
    )?;
    if !added.status.success() {
        return Err("could not list staged additions".into());
    }
    let mut entries = added.stdout.split(|byte| *byte == 0);
    while let Some(header) = entries.next().filter(|header| !header.is_empty()) {
        let path = entries.next().ok_or("missing staged path")?;
        let fields: Vec<_> = header.split(|byte| *byte == b' ').collect();
        if fields.get(1) == Some(&b"160000".as_slice()) {
            continue; // Gitlinks point to commits in another repository, not files.
        }
        let object = fields.get(3).ok_or("missing staged object id")?;
        let size = git(&root, &["cat-file", "-s", std::str::from_utf8(object)?])?;
        if !size.status.success() {
            return Err("could not read a staged object's size".into());
        }
        let bytes: u64 = std::str::from_utf8(&size.stdout)?.trim().parse()?;
        if bytes > MAX_BYTES {
            problems.push(format!("{:?}: {bytes} bytes exceeds the 500 KiB limit for new files; use Git LFS or keep generated files outside Git", OsStr::from_bytes(path)));
        }
    }
    Ok(problems.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn git_ok(root: &Path, args: &[&str]) {
        assert!(
            Command::new("git")
                .current_dir(root)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }

    #[test]
    fn checks_staged_whitespace_and_conflicts_without_printing_source() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git_ok(root, &["init", "-q"]);
        fs::write(
            root.join("file.txt"),
            "sensitive-value  \n<<<<<<< ours\na\n=======\nb\n>>>>>>> theirs\n",
        )
        .unwrap();
        git_ok(root, &["add", "file.txt"]);
        fs::write(root.join("file.txt"), "clean\n").unwrap();
        let (verdict, output) = check(root);
        assert_eq!(verdict, Verdict::Failed, "{output}");
        assert!(output.contains("trailing whitespace"));
        assert!(output.contains("conflict marker"));
        assert!(!output.contains("sensitive-value"));
        git_ok(root, &["add", "file.txt"]);
        assert_eq!(check(root).0, Verdict::Passed);
    }

    #[test]
    fn large_files_use_index_sizes_and_handle_unusual_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git_ok(root, &["init", "-q"]);
        let name = "large 'file\n.bin";
        fs::write(root.join(name), vec![0; MAX_BYTES as usize + 1]).unwrap();
        git_ok(root, &["add", "--", name]);
        fs::write(root.join(name), "small\n").unwrap();
        let (verdict, output) = check(root);
        assert_eq!(verdict, Verdict::Failed, "{output}");
        assert!(output.contains("500 KiB"));
        git_ok(root, &["add", "--", name]);
        assert_eq!(check(root).0, Verdict::Passed);
    }

    #[test]
    fn a_gitlink_is_not_treated_as_a_new_large_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git_ok(root, &["init", "-q"]);
        git_ok(
            root,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                "160000,1234567890123456789012345678901234567890,submodule",
            ],
        );
        assert_eq!(check(root).0, Verdict::Passed);
    }
}
