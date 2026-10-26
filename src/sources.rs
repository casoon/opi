//! Dependencies that do not come from the registry.
//!
//! A git URL, a tarball URL or a path outside the workspace is installed
//! without the registry's publishing trail, so the lockfile is read for them.
//! Nothing is run: the lockfiles record where each package was resolved from.
//! A path inside the workspace is the project's own code and not notable.
//! `bun.lockb`, bun's binary lockfile, is not read.

use std::path::Path;

use serde_json::Value;

/// A dependency resolved from somewhere other than the registry.
#[derive(Debug, PartialEq, Eq)]
pub struct Foreign {
    pub name: String,
    /// Where the lockfile says it came from.
    pub source: String,
}

/// Every non-registry dependency in the lockfiles found in `dir`.
pub fn scan(dir: &Path) -> Vec<Foreign> {
    let read = |file: &str| std::fs::read_to_string(dir.join(file)).ok();
    let mut found = Vec::new();
    if let Some(text) = read("package-lock.json") {
        found.extend(npm(&text));
    }
    if let Some(text) = read("pnpm-lock.yaml") {
        found.extend(pnpm(&text));
    }
    if let Some(text) = read("yarn.lock") {
        found.extend(yarn(&text));
    }
    if let Some(text) = read("bun.lock") {
        found.extend(bun(&text));
    }
    if let Some(text) = read("Cargo.lock") {
        found.extend(cargo(&text));
    }
    found.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.source.cmp(&b.source)));
    found.dedup();
    found
}

/// Whether a resolved location or version spec is outside the registry.
fn outside_registry(spec: &str) -> bool {
    let spec = spec.trim();
    if [
        "git+",
        "git:",
        "git@",
        "ssh:",
        "github:",
        "gitlab:",
        "bitbucket:",
        "gist:",
    ]
    .iter()
    .any(|prefix| spec.starts_with(prefix))
    {
        return true;
    }
    if spec.starts_with("http://") || spec.starts_with("https://") {
        // Registry tarballs are `<name>/-/<file>`; a git host or a plain file
        // server does not use that shape.
        return !spec.contains("/-/");
    }
    ["file:", "link:", "portal:"]
        .iter()
        .find_map(|prefix| spec.strip_prefix(prefix))
        .is_some_and(|path| path.starts_with("..") || path.starts_with('/'))
}

fn foreign(name: &str, source: &str) -> Foreign {
    Foreign {
        name: name.to_owned(),
        source: source.to_owned(),
    }
}

fn npm(json: &str) -> Vec<Foreign> {
    let Ok(value) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(packages) = value.get("packages").and_then(Value::as_object) else {
        return Vec::new();
    };
    packages
        .iter()
        .filter_map(|(path, entry)| {
            let name = path.rsplit_once("node_modules/")?.1;
            let resolved = entry.get("resolved")?.as_str()?;
            let source = if entry.get("link").and_then(Value::as_bool) == Some(true) {
                // A symlink to a workspace member or to a folder beside it.
                if !resolved.starts_with("..") {
                    return None;
                }
                format!("link:{resolved}")
            } else if outside_registry(resolved) {
                resolved.to_owned()
            } else {
                return None;
            };
            Some(foreign(name, &source))
        })
        .collect()
}

/// pnpm's lockfile is YAML, read by line: a package key, then its `resolution`.
fn pnpm(yaml: &str) -> Vec<Foreign> {
    let mut found = Vec::new();
    let mut name = String::new();
    for line in yaml.lines() {
        let trimmed = line.trim();
        if let Some(key) = trimmed
            .strip_suffix(':')
            .filter(|_| line.starts_with("  ") && !line.starts_with("   "))
        {
            let key = key.trim_matches(|c| c == '\'' || c == '"');
            name = key_name(key).to_owned();
        } else if let Some(spec) = trimmed.strip_prefix("resolution:") {
            let spec = spec.trim().trim_start_matches('{').trim_end_matches('}');
            let field = |key: &str| {
                spec.split(',')
                    .find_map(|part| part.trim().strip_prefix(key))
                    .map(|value| value.trim().trim_matches(|c| c == '\'' || c == '"'))
            };
            let source = if let Some(url) = field("tarball:") {
                Some(url.to_owned())
            } else if field("type:") == Some("git") {
                field("repo:").map(|repo| format!("git+{repo}"))
            } else {
                field("directory:")
                    .filter(|path| path.starts_with(".."))
                    .map(|path| format!("file:{path}"))
            };
            if let Some(source) = source.filter(|s| s.starts_with("git+") || outside_registry(s)) {
                found.push(foreign(&name, &source));
            }
        } else if let Some(version) = trimmed.strip_prefix("version:") {
            // An importer's dependency linked from outside the workspace.
            let version = version.trim().trim_matches(|c| c == '\'' || c == '"');
            if version.starts_with("link:") && outside_registry(version) {
                found.push(foreign(&name, version));
            }
        } else if line.starts_with("      ")
            && !line.starts_with("       ")
            && trimmed.ends_with(':')
        {
            // Importer dependency name, e.g. `      shared:`.
            name = trimmed
                .trim_end_matches(':')
                .trim_matches(|c| c == '\'' || c == '"')
                .to_owned();
        }
    }
    found
}

/// The package name of a `name@spec` key, scope included.
fn key_name(key: &str) -> &str {
    let key = key.strip_prefix('/').unwrap_or(key);
    match key.get(1..).and_then(|rest| rest.find('@')) {
        Some(at) => &key[..at + 1],
        None => key,
    }
}

/// Yarn classic (`resolved "url"`) and berry (`resolution: "name@spec"`).
fn yarn(text: &str) -> Vec<Foreign> {
    let mut found = Vec::new();
    let mut name = String::new();
    for line in text.lines() {
        if !line.starts_with(' ') && line.ends_with(':') && !line.starts_with('#') {
            let first = line.trim_end_matches(':').split(", ").next().unwrap_or("");
            name = key_name(first.trim_matches('"')).to_owned();
            continue;
        }
        let trimmed = line.trim();
        let source = if let Some(url) = trimmed.strip_prefix("resolved ") {
            Some(
                url.trim_matches('"')
                    .split('#')
                    .next()
                    .unwrap_or("")
                    .to_owned(),
            )
            .filter(|url| outside_registry(url) || url.contains(".git"))
        } else if let Some(resolution) = trimmed.strip_prefix("resolution:") {
            let resolution = resolution.trim().trim_matches('"');
            let spec = resolution
                .get(1..)
                .and_then(|rest| rest.find('@'))
                .map(|at| &resolution[at + 2..]);
            spec.filter(|spec| outside_registry(spec) || spec.contains(".git"))
                .map(str::to_owned)
        } else {
            None
        };
        if let Some(source) = source {
            found.push(foreign(&name, &source));
        }
    }
    found
}

/// `bun.lock` is JSONC: JSON with trailing commas.
fn bun(text: &str) -> Vec<Foreign> {
    let Ok(value) = serde_json::from_str::<Value>(&without_trailing_commas(text)) else {
        return Vec::new();
    };
    let Some(packages) = value.get("packages").and_then(Value::as_object) else {
        return Vec::new();
    };
    packages
        .values()
        .filter_map(|entry| {
            let ident = entry.get(0)?.as_str()?;
            let at = ident.get(1..)?.find('@')? + 1;
            let (name, spec) = (&ident[..at], &ident[at + 1..]);
            (outside_registry(spec) || spec.contains(".git")).then(|| foreign(name, spec))
        })
        .collect()
}

fn without_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let (mut in_string, mut escaped) = (false, false);
    for (i, &c) in chars.iter().enumerate() {
        if in_string {
            escaped = !escaped && c == '\\';
            if c == '"' && !escaped {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
        } else if c == ','
            && chars[i + 1..]
                .iter()
                .find(|c| !c.is_whitespace())
                .is_some_and(|next| matches!(next, '}' | ']'))
        {
            continue;
        }
        out.push(c);
    }
    out
}

/// `Cargo.lock`: a package without `source` is a path dependency, which is the
/// project's own code; a `git+` source is not.
fn cargo(toml: &str) -> Vec<Foreign> {
    let mut found = Vec::new();
    let mut name = "";
    for line in toml.lines() {
        let value = |key: &str| {
            line.strip_prefix(key)
                .and_then(|rest| rest.strip_prefix(" = \""))
                .and_then(|rest| rest.strip_suffix('"'))
        };
        if let Some(n) = value("name") {
            name = n;
        } else if let Some(source) = value("source").filter(|s| s.starts_with("git+")) {
            found.push(foreign(name, source));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(found: Vec<Foreign>) -> Vec<(String, String)> {
        found.into_iter().map(|f| (f.name, f.source)).collect()
    }

    #[test]
    fn npm_lockfile_names_git_tarball_and_outside_links() {
        let found = npm(r#"{"packages": {
                "": {"name": "app"},
                "node_modules/ok": {"version": "1.0.0", "resolved": "https://registry.npmjs.org/ok/-/ok-1.0.0.tgz"},
                "node_modules/@s/ok": {"resolved": "https://registry.npmjs.org/@s/ok/-/ok-1.0.0.tgz"},
                "node_modules/gitdep": {"resolved": "git+ssh://git@github.com/o/gitdep.git#abc"},
                "node_modules/tar": {"resolved": "https://example.com/tar-1.0.0.tgz"},
                "node_modules/inner": {"resolved": "packages/inner", "link": true},
                "node_modules/outer": {"resolved": "../outer", "link": true}
            }}"#);
        assert_eq!(
            pairs(found),
            [
                (
                    "gitdep".to_owned(),
                    "git+ssh://git@github.com/o/gitdep.git#abc".to_owned()
                ),
                ("outer".to_owned(), "link:../outer".to_owned()),
                (
                    "tar".to_owned(),
                    "https://example.com/tar-1.0.0.tgz".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn pnpm_lockfile_names_git_tarball_and_outside_links() {
        let found = pnpm(
            r#"lockfileVersion: '9.0'
importers:
  .:
    dependencies:
      shared:
        specifier: link:../shared
        version: link:../shared
      inner:
        specifier: link:packages/inner
        version: link:packages/inner
packages:
  ok@1.0.0:
    resolution: {integrity: sha512-abc}
  '@s/tar@https://example.com/tar-1.0.0.tgz':
    resolution: {tarball: https://example.com/tar-1.0.0.tgz}
  gitdep@https://codeload.github.com/o/gitdep/tar.gz/abc:
    resolution: {type: git, repo: https://github.com/o/gitdep.git, commit: abc}
"#,
        );
        assert_eq!(
            pairs(found),
            [
                ("shared", "link:../shared"),
                ("@s/tar", "https://example.com/tar-1.0.0.tgz"),
                ("gitdep", "git+https://github.com/o/gitdep.git"),
            ]
            .map(|(n, s)| (n.to_owned(), s.to_owned()))
        );
    }

    #[test]
    fn yarn_classic_and_berry_lockfiles_are_read() {
        let classic = yarn(
            "ok@^1.0.0:\n  version \"1.0.0\"\n  resolved \"https://registry.yarnpkg.com/ok/-/ok-1.0.0.tgz#abc\"\n\n\
\"gitdep@github:o/gitdep\":\n  version \"1.0.0\"\n  resolved \"https://codeload.github.com/o/gitdep/tar.gz/abc\"\n",
        );
        assert_eq!(
            pairs(classic),
            [(
                "gitdep".to_owned(),
                "https://codeload.github.com/o/gitdep/tar.gz/abc".to_owned()
            )]
        );
        let berry = yarn(
            "\"ok@npm:^1.0.0\":\n  version: 1.0.0\n  resolution: \"ok@npm:1.0.0\"\n\n\
\"outer@file:../outer::locator=app%40workspace%3A.\":\n  version: 0.0.0-use.local\n  resolution: \"outer@file:../outer#../outer::hash=1&locator=app%40workspace%3A.\"\n\n\
\"inner@workspace:packages/inner\":\n  resolution: \"inner@workspace:packages/inner\"\n\n\
\"gitdep@https://github.com/o/gitdep.git#commit=abc\":\n  resolution: \"gitdep@https://github.com/o/gitdep.git#commit=abc\"\n",
        );
        let names: Vec<_> = berry.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["outer", "gitdep"]);
    }

    #[test]
    fn bun_lockfile_tolerates_trailing_commas() {
        let found = bun(r#"{
              "lockfileVersion": 1,
              "packages": {
                "ok": ["ok@1.0.0", "", {}, "sha512-abc"],
                "gitdep": ["gitdep@github:o/gitdep#abc", {}, "o-gitdep-abc"],
                "outer": ["outer@file:../outer", {}],
                "inner": ["inner@workspace:packages/inner"],
              },
            }"#);
        assert_eq!(
            pairs(found),
            [
                ("gitdep", "github:o/gitdep#abc"),
                ("outer", "file:../outer")
            ]
            .map(|(n, s)| (n.to_owned(), s.to_owned()))
        );
    }

    #[test]
    fn cargo_lockfile_names_git_sources_only() {
        let found = cargo(
            "[[package]]\nname = \"local\"\nversion = \"0.1.0\"\n\n\
[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n\n\
[[package]]\nname = \"forked\"\nversion = \"0.2.0\"\nsource = \"git+https://github.com/o/forked?branch=x#abc\"\n",
        );
        assert_eq!(
            pairs(found),
            [(
                "forked".to_owned(),
                "git+https://github.com/o/forked?branch=x#abc".to_owned()
            )]
        );
    }
}
