//! Adapter over nosecrets' scanning engine; no detection rules live in opi.

use std::path::Path;

use nosecrets_core::{Detector, collect_files};
use nosecrets_filter::{Config, Filter, load_ignore_file};
use nosecrets_rules::load_builtin_rules;
use runemark::Verdict;

pub fn scan(dir: &Path, staged: bool) -> (Verdict, String) {
    match scan_inner(dir, staged) {
        Ok(result) => result,
        Err(error) => (Verdict::ActionRequired, format!("nosecrets: {error}")),
    }
}

fn scan_inner(dir: &Path, staged: bool) -> Result<(Verdict, String), Box<dyn std::error::Error>> {
    let repository = crate::staged::repository(dir);
    let root = repository.as_deref().unwrap_or(dir);
    let config = Config::load_from_dir(root)?;
    let entropy = config
        .as_ref()
        .map(|c| c.entropy.clone())
        .unwrap_or_default();
    let filter = Filter::from_config(config, load_ignore_file(&root.join(".nosecretsignore"))?)?;
    // Filter before scanning: the engine reads each supplied file before
    // applying path ignores, including potentially large build artefacts.
    let files = if staged {
        Vec::new()
    } else {
        collect_files(root, &[dir.to_path_buf()])?
            .into_iter()
            .filter(|path| !filter.is_path_ignored(path.strip_prefix(root).unwrap_or(path)))
            .collect()
    };
    let detector = Detector::with_entropy(load_builtin_rules()?, filter, entropy)?;
    let mut findings = if staged {
        let root = repository
            .as_deref()
            .ok_or("staged scanning requires a Git repository")?;
        detector.scan_staged_files(root)?
    } else {
        detector.scan_files(root, &files)?
    };
    findings.sort_by(|a, b| {
        (&a.path, a.line, a.column, &a.rule_id).cmp(&(&b.path, b.line, b.column, &b.rule_id))
    });
    let verdict = if findings.iter().any(|finding| finding.severity.blocks()) {
        Verdict::Failed
    } else {
        Verdict::Passed
    };
    // Do not include previews: findings should never copy credentials into logs.
    let output = findings
        .iter()
        .map(|finding| {
            format!(
                "{}:{}:{} [{}] {} ({}) — {}",
                finding.path,
                finding.line,
                finding.column,
                finding.severity.as_str(),
                finding.rule_name,
                finding.rule_id,
                finding.fingerprint,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok((verdict, output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};

    fn git(root: &Path, args: &[&str]) {
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

    fn private_key() -> String {
        ["-----BEGIN RSA ", "PRIVATE KEY-----\nexample\n"].concat()
    }

    #[test]
    fn staged_secrets_cannot_be_hidden_by_an_unstaged_fix() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        fs::write(root.join("key.pem"), private_key()).unwrap();
        git(root, &["add", "key.pem"]);
        fs::write(root.join("key.pem"), "clean\n").unwrap();
        let (verdict, output) = scan(root, true);
        assert_eq!(verdict, Verdict::Failed, "{output}");
        assert!(output.contains("key.pem:1:"));
        assert!(!output.contains("-----BEGIN"));
        assert_eq!(scan(root, false).0, Verdict::Passed);
    }

    #[test]
    fn unstaged_secrets_do_not_block_a_clean_index() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        fs::write(root.join("key.pem"), "clean\n").unwrap();
        git(root, &["add", "key.pem"]);
        fs::write(root.join("key.pem"), private_key()).unwrap();
        assert_eq!(scan(root, true).0, Verdict::Passed);
        assert_eq!(scan(root, false).0, Verdict::Failed);
    }

    #[test]
    fn repository_configuration_and_fingerprint_ignores_are_respected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("key.pem"), private_key()).unwrap();
        let (_, output) = scan(root, false);
        let fingerprint = output.split_whitespace().last().unwrap();
        fs::write(root.join(".nosecretsignore"), fingerprint).unwrap();
        assert_eq!(scan(root, false).0, Verdict::Passed);
        fs::remove_file(root.join(".nosecretsignore")).unwrap();
        fs::write(
            root.join(".nosecrets.toml"),
            "[ignore]\npaths = ['*.pem']\n",
        )
        .unwrap();
        assert_eq!(scan(root, false).0, Verdict::Passed);
        fs::write(root.join(".nosecrets.toml"), "invalid [").unwrap();
        assert_eq!(scan(root, false).0, Verdict::ActionRequired);
    }
}
