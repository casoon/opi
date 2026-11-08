//! Recorded tool output, end to end.
//!
//! `audit.rs`, `outdated.rs` and `osv.rs` read what npm, pnpm, cargo and
//! osv-scanner print, and that changes between versions. The cost of a package
//! manager changing its JSON is a silently empty report, not a crash — so the
//! real binary is run against stand-ins on a `PATH` that answer with the files
//! in `tests/fixtures/`, and the report is what is asserted.
//!
//! The stand-ins are `sh` scripts that touch nothing but a local temp
//! directory; there is no network. Each logs every call as one line of
//! arguments, and answers by matching the arguments it was given.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};

struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temp dir");
        for dir in ["bin", "log", "project"] {
            fs::create_dir_all(root.path().join(dir)).expect("mkdir");
        }
        Self { root }
    }

    fn project(&self) -> PathBuf {
        self.root.path().join("project")
    }

    fn file(&self, path: &str, contents: &str) -> &Self {
        let full = self.project().join(path);
        fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        fs::write(full, contents).expect("write");
        self
    }

    /// A stand-in for `name` on `PATH`. `answers` are tried in order against
    /// the call's arguments joined by spaces (a shell `case` pattern); the
    /// first match prints its fixture on stdout and exits with its status.
    /// Anything unmatched exits 0 in silence.
    fn tool(&self, name: &str, answers: &[Answer]) -> &Self {
        let mut cases = String::new();
        for answer in answers {
            let print = match answer.fixture {
                Some(file) => format!(
                    "emit '{}/tests/fixtures/{file}'; ",
                    env!("CARGO_MANIFEST_DIR")
                ),
                None => String::new(),
            };
            let said = match answer.stderr {
                Some(text) => format!("echo '{text}' >&2; "),
                None => String::new(),
            };
            let pattern = match answer.args.strip_suffix('*') {
                Some(prefix) => format!("\"{prefix}\"*"),
                None => format!("\"{}\"", answer.args),
            };
            cases.push_str(&format!(
                "  {pattern}) {print}{said}exit {};;\n",
                answer.status
            ));
        }
        // Builtins only: `PATH` holds nothing but the stand-ins.
        let script = format!(
            "#!/bin/sh\n\
             printf '%s\\n' \"$*\" >> \"$OPI_TEST_LOG/{name}\"\n\
             emit() {{ while IFS= read -r line || [ -n \"$line\" ]; do printf '%s\\n' \"$line\"; done < \"$1\"; }}\n\
             case \"$*\" in\n{cases}esac\n\
             exit 0\n"
        );
        let path = self.root.path().join("bin").join(name);
        fs::write(&path, script).expect("write script");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
        self
    }

    /// Every call `name` received, as its arguments joined by spaces.
    fn calls(&self, name: &str) -> Vec<String> {
        fs::read_to_string(self.root.path().join("log").join(name))
            .map(|text| text.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    fn opi(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_opi"))
            .args(args)
            .current_dir(self.project())
            .env_clear()
            .env("PATH", self.root.path().join("bin"))
            .env("HOME", self.root.path())
            .env("OPI_TEST_LOG", self.root.path().join("log"))
            .output()
            .expect("opi runs")
    }
}

struct Answer {
    args: &'static str,
    fixture: Option<&'static str>,
    stderr: Option<&'static str>,
    status: i32,
}

/// `args` answers with `fixture` and `status`.
fn says(args: &'static str, fixture: &'static str, status: i32) -> Answer {
    Answer {
        args,
        fixture: Some(fixture),
        stderr: None,
        status,
    }
}

/// `args` fails with `text` on stderr and nothing on stdout.
fn fails(args: &'static str, text: &'static str) -> Answer {
    Answer {
        args,
        fixture: None,
        stderr: Some(text),
        status: 1,
    }
}

/// `args` ends with `status` and nothing printed.
fn exits(args: &'static str, status: i32) -> Answer {
    Answer {
        args,
        fixture: None,
        stderr: None,
        status,
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn npm_project(fixture: &Fixture) {
    fixture
        .file("package.json", r#"{"name":"app"}"#)
        .file("package-lock.json", "{}");
}

fn cargo_project(fixture: &Fixture) {
    fixture
        .file(
            "Cargo.toml",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .file("Cargo.lock", "");
}

// --- Security ---

#[test]
fn an_npm_audit_is_read_with_the_fix_that_npm_names() {
    let fixture = Fixture::new();
    npm_project(&fixture);
    fixture
        .tool("npm", &[says("audit --json", "npm-audit.json", 1)])
        .tool("osv-scanner", &[exits("*", 128)]);

    let output = fixture.opi(&["--security"]);
    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{printed}");
    assert!(printed.contains("lodash"), "{printed}");
    assert!(printed.contains("update to 4.18.1"), "{printed}");
    assert!(printed.contains("minimist"), "{printed}");
    // The fix for minimist is a newer mkdirp, and says so.
    assert!(printed.contains("update to mkdirp@1.0.4"), "{printed}");
    assert!(printed.contains("critical"), "{printed}");
    assert!(!printed.contains("Nothing to act on"), "{printed}");
    assert!(fixture.calls("npm").contains(&"audit --json".to_owned()));
}

#[test]
fn a_clean_npm_audit_is_clean() {
    let fixture = Fixture::new();
    npm_project(&fixture);
    fixture
        .tool("npm", &[says("audit --json", "npm-audit-clean.json", 0)])
        .tool("osv-scanner", &[exits("*", 128)]);

    let output = fixture.opi(&["--security"]);
    let printed = stdout(&output);
    assert!(output.status.success(), "{printed}");
    assert!(printed.contains("no known vulnerabilities"), "{printed}");
    assert!(printed.contains("Nothing to act on"), "{printed}");
}

#[test]
fn a_pnpm_audit_is_read_with_its_patched_ranges() {
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"name":"app"}"#)
        .file("pnpm-lock.yaml", "")
        .tool("pnpm", &[says("audit --json", "pnpm-audit.json", 1)])
        .tool("osv-scanner", &[exits("*", 128)]);

    let output = fixture.opi(&["--security"]);
    let printed = stdout(&output);
    // trim-newlines is high, so the run fails — and names the patched range.
    assert_eq!(output.status.code(), Some(1), "{printed}");
    assert!(printed.contains("trim-newlines"), "{printed}");
    assert!(printed.contains("update to >=3.0.1"), "{printed}");
    assert!(printed.contains("minimist"), "{printed}");
    assert!(printed.contains("moderate"), "{printed}");
}

#[test]
fn an_audit_that_could_not_reach_the_registry_says_so_instead_of_passing() {
    let fixture = Fixture::new();
    npm_project(&fixture);
    fixture
        .tool(
            "npm",
            &[fails(
                "audit --json",
                "npm error audit endpoint returned an error",
            )],
        )
        .tool("osv-scanner", &[exits("*", 128)]);

    let output = fixture.opi(&["--security"]);
    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{printed}");
    assert!(
        printed.contains("audit endpoint returned an error"),
        "{printed}"
    );
    assert!(!printed.contains("Nothing to act on"), "{printed}");
}

#[test]
fn cargo_audit_reports_the_advisory_by_its_id() {
    let fixture = Fixture::new();
    cargo_project(&fixture);
    fixture
        .tool("cargo", &[says("audit --json", "cargo-audit.json", 1)])
        .tool("cargo-audit", &[])
        .tool("osv-scanner", &[exits("*", 128)]);

    let output = fixture.opi(&["--security"]);
    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{printed}");
    assert!(printed.contains("quick-xml 0.38.4"), "{printed}");
    assert!(printed.contains("RUSTSEC-2026-0195"), "{printed}");
    assert!(printed.contains("update to >=0.38.5"), "{printed}");
}

#[test]
fn osv_adds_only_what_the_native_audit_did_not_report() {
    // lodash is in both reports; word-wrap only in osv-scanner's. The second
    // is the one to show under osv, with its CVSS band.
    let fixture = Fixture::new();
    npm_project(&fixture);
    fixture
        .tool("npm", &[says("audit --json", "npm-audit.json", 1)])
        .tool("osv-scanner", &[says("*", "osv-scanner.json", 1)]);

    let output = fixture.opi(&["--security"]);
    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{printed}");
    let osv = printed
        .split("Dependencies (osv)")
        .nth(1)
        .unwrap_or_else(|| panic!("no osv section: {printed}"));
    assert!(osv.contains("word-wrap 1.2.3"), "{printed}");
    assert!(osv.contains("update to >=1.2.4"), "{printed}");
    assert!(!osv.contains("lodash"), "{printed}");
    assert_eq!(
        fixture.calls("osv-scanner").len(),
        1,
        "{:?}",
        fixture.calls("osv-scanner")
    );
    assert!(
        fixture.calls("osv-scanner")[0].starts_with("scan --format json -r "),
        "{:?}",
        fixture.calls("osv-scanner")
    );
}

#[test]
fn osv_findings_stand_alone_when_the_native_audit_is_clean() {
    let fixture = Fixture::new();
    npm_project(&fixture);
    fixture
        .tool("npm", &[says("audit --json", "npm-audit-clean.json", 0)])
        .tool("osv-scanner", &[says("*", "osv-scanner.json", 1)]);

    let output = fixture.opi(&["--security"]);
    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{printed}");
    assert!(printed.contains("lodash 4.17.20"), "{printed}");
    assert!(printed.contains("word-wrap 1.2.3"), "{printed}");
}

#[test]
fn a_missing_osv_scanner_is_named_and_not_a_failure() {
    let fixture = Fixture::new();
    npm_project(&fixture);
    fixture.tool("npm", &[says("audit --json", "npm-audit-clean.json", 0)]);

    let output = fixture.opi(&["--security"]);
    let printed = stdout(&output);
    assert!(output.status.success(), "{printed}");
    assert!(
        printed.contains("osv-scanner is not installed"),
        "{printed}"
    );
}

// --- Updates (listing only) ---

#[test]
fn npm_outdated_is_split_into_safe_and_major() {
    let fixture = Fixture::new();
    npm_project(&fixture);
    fixture.tool("npm", &[says("outdated --json", "npm-outdated.json", 1)]);

    let output = fixture.opi(&["--updates"]);
    let printed = stdout(&output);
    assert!(output.status.success(), "{printed}{}", stderr(&output));
    assert!(printed.contains("eslint 8.57.0 → 9.14.0"), "{printed}");
    assert!(printed.contains("ms 2.1.2 → 2.1.3"), "{printed}");
    assert!(printed.contains("typescript 5.4.5 → 5.6.3"), "{printed}");
    assert!(printed.contains("Safe to take"), "{printed}");
    assert!(printed.contains("A decision each"), "{printed}");
    assert!(!printed.contains("everything is current"), "{printed}");
    // Listing only: nothing is updated without a terminal and a choice.
    assert!(
        fixture
            .calls("npm")
            .iter()
            .all(|call| !call.starts_with("update")),
        "{:?}",
        fixture.calls("npm")
    );
}

#[test]
fn pnpm_outdated_survives_a_warning_ahead_of_the_json_and_covers_the_workspace() {
    // pnpm writes its warnings to stdout, in front of the document.
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"name":"root","workspaces":["apps/*"]}"#)
        .file("pnpm-lock.yaml", "")
        .file("apps/blog/package.json", r#"{"name":"blog"}"#)
        .tool(
            "pnpm",
            &[says("outdated -r --json", "pnpm-outdated.json", 1)],
        );

    let output = fixture.opi(&["--updates"]);
    let printed = stdout(&output);
    assert!(output.status.success(), "{printed}{}", stderr(&output));
    assert!(printed.contains("astro 4.16.0 → 5.1.2"), "{printed}");
    assert!(printed.contains("zod 3.23.8 → 3.24.1"), "{printed}");
    assert_eq!(fixture.calls("pnpm"), ["outdated -r --json"]);
}

#[test]
fn cargo_outdated_is_read_per_member_and_deduplicated() {
    let fixture = Fixture::new();
    cargo_project(&fixture);
    fixture
        .tool(
            "cargo",
            &[says(
                "outdated --workspace --root-deps-only --format json",
                "cargo-outdated.jsonl",
                0,
            )],
        )
        .tool("cargo-outdated", &[]);

    let output = fixture.opi(&["--updates"]);
    let printed = stdout(&output);
    assert!(output.status.success(), "{printed}{}", stderr(&output));
    assert!(printed.contains("clap 3.2.25 → 4.5.20"), "{printed}");
    assert_eq!(
        printed.matches("serde 1.0.200 → 1.0.228").count(),
        1,
        "two members, one line: {printed}"
    );
    // regex is current, so it is not listed.
    assert!(!printed.contains("regex"), "{printed}");
}

// --- Workflows ---

/// `cargo fmt --check` as rustfmt prints it when a file needs formatting.
const RUSTFMT_DIFF: &str = "Diff in /project/src/main.rs:1:\n-fn main(){}\n+fn main() {}";

fn failing_format(fixture: &Fixture) {
    fixture
        .tool(
            "cargo",
            &[Answer {
                args: "fmt --check --all",
                fixture: None,
                stderr: Some(RUSTFMT_DIFF),
                status: 1,
            }],
        )
        .tool("cargo-fmt", &[]);
}

#[test]
fn the_push_workflow_runs_the_cargo_checks_and_fails_on_the_format_diff() {
    let fixture = Fixture::new();
    cargo_project(&fixture);
    failing_format(&fixture);

    let output = fixture.opi(&["--check", "push"]);
    let printed = stdout(&output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{printed}{}",
        stderr(&output)
    );
    assert!(printed.contains("Format"), "{printed}");
    assert!(printed.contains("Diff in"), "{printed}");
    let calls = fixture.calls("cargo");
    for expected in [
        "fmt --check --all",
        "clippy --all-targets --all-features --workspace -- -D warnings",
        "test --all-features --workspace",
    ] {
        assert!(calls.iter().any(|call| call == expected), "{calls:?}");
    }
    // The build comes after the checks, and is part of push.
    assert!(
        calls.iter().any(|call| call.starts_with("build")),
        "{calls:?}"
    );
}

#[test]
fn the_commit_workflow_leaves_out_tests_and_the_build() {
    let fixture = Fixture::new();
    cargo_project(&fixture);
    fixture
        .tool("cargo", &[exits("*", 0)])
        .tool("cargo-fmt", &[]);

    let output = fixture.opi(&["--check", "commit"]);
    let printed = stdout(&output);
    assert!(output.status.success(), "{printed}{}", stderr(&output));
    let calls = fixture.calls("cargo");
    assert!(
        calls.iter().any(|call| call.starts_with("fmt")),
        "{calls:?}"
    );
    assert!(
        calls.iter().all(|call| !call.starts_with("test")),
        "{calls:?}"
    );
    assert!(
        calls.iter().all(|call| !call.starts_with("build")),
        "{calls:?}"
    );
}

#[test]
fn a_green_push_for_npm_is_green_and_builds() {
    let fixture = Fixture::new();
    fixture
        .file(
            "package.json",
            r#"{"name":"app","scripts":{"build":"tsc","test":"vitest run"}}"#,
        )
        .file("package-lock.json", "{}")
        .tool("npm", &[exits("*", 0)]);

    let output = fixture.opi(&["--check", "push"]);
    let printed = stdout(&output);
    assert!(output.status.success(), "{printed}{}", stderr(&output));
    let calls = fixture.calls("npm");
    assert!(calls.iter().any(|call| call == "run build"), "{calls:?}");
    assert!(
        calls.iter().any(|call| call.starts_with("ci --dry-run")),
        "{calls:?}"
    );
}
