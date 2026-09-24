//! The process boundary, tested against the real binary.
//!
//! The unit tests cover what `opi` decides; these cover what actually crosses
//! into another process — the argv, the working directory, the exit code, and
//! what `opi` makes of stdout, stderr and a missing executable.
//!
//! Package managers are stand-ins: small `sh` scripts on a `PATH` that holds
//! nothing else, each recording how it was called and answering with output
//! trimmed from real runs. The boundary is real, the network is not — a test
//! that needs the registry is a test that fails on a train.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(root.path().join("bin")).expect("mkdir bin");
        fs::create_dir_all(root.path().join("log")).expect("mkdir log");
        fs::create_dir_all(root.path().join("project")).expect("mkdir project");
        Self { root }
    }

    fn project(&self) -> PathBuf {
        self.root.path().join("project")
    }

    /// Writes a file under the project.
    fn file(&self, path: &str, contents: &str) -> &Self {
        let full = self.project().join(path);
        fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        fs::write(full, contents).expect("write");
        self
    }

    /// An executable at `path` under the project, recording its calls.
    fn tool(&self, path: &str, body: &str) -> &Self {
        let name = Path::new(path).file_name().expect("name").to_string_lossy();
        let full = self.project().join(path);
        fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        write_script(&full, &name, body);
        self
    }

    /// A stand-in for `name` on `PATH`, recording its calls.
    fn shim(&self, name: &str, body: &str) -> &Self {
        write_script(&self.root.path().join("bin").join(name), name, body);
        self
    }

    /// How `name` was called: its working directory, then one line per
    /// argument. `None` if it never ran.
    fn calls(&self, name: &str) -> Option<Vec<String>> {
        let text = fs::read_to_string(self.root.path().join("log").join(name)).ok()?;
        Some(text.lines().map(str::to_owned).collect())
    }

    /// Runs `opi` in `dir` (relative to the project) with nothing on `PATH`
    /// but the stand-ins.
    fn opi(&self, dir: &str, args: &[&str]) -> Output {
        let dir = self.project().join(dir);
        fs::create_dir_all(&dir).expect("mkdir cwd");
        Command::new(env!("CARGO_BIN_EXE_opi"))
            .args(args)
            .current_dir(dir)
            .env_clear()
            .env("PATH", self.root.path().join("bin"))
            .env("HOME", self.root.path())
            .env("OPI_TEST_LOG", self.root.path().join("log"))
            .output()
            .expect("opi runs")
    }
}

fn write_script(path: &Path, name: &str, body: &str) {
    // Builtins only: `PATH` holds nothing but the stand-ins, so `cat` or `env`
    // would not be found.
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$(pwd -P)\" \"$@\" > \"$OPI_TEST_LOG/{name}\"\n{body}\n"
    );
    fs::write(path, script).expect("write script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn canonical(path: PathBuf) -> String {
    path.canonicalize()
        .expect("canonicalize")
        .to_string_lossy()
        .into_owned()
}

// --- Running a script ---

#[test]
fn a_script_runs_through_the_detected_manager_with_its_arguments() {
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"scripts":{"build":"x"}}"#)
        .file("package-lock.json", "{}")
        .shim("npm", "exit 0");

    let output = fixture.opi(".", &["build", "--verbose"]);
    assert!(output.status.success(), "{}", stderr(&output));

    let calls = fixture.calls("npm").expect("npm ran");
    assert_eq!(
        calls[1..],
        ["run", "build", "--", "--verbose"],
        "npm needs `--` before the script's own flags"
    );
}

#[test]
fn a_member_script_is_addressed_the_way_the_manager_expects() {
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"name":"root","workspaces":["apps/*"]}"#)
        .file("pnpm-lock.yaml", "")
        .file(
            "apps/blog/package.json",
            r#"{"name":"blog","scripts":{"dev":"x"}}"#,
        )
        .shim("pnpm", "exit 0");

    let output = fixture.opi(".", &["blog/dev"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        fixture.calls("pnpm").expect("pnpm ran")[1..],
        ["--filter", "blog", "run", "dev"]
    );
}

#[test]
fn the_scripts_exit_code_and_output_are_its_own() {
    // `exec`, not supervision: what the shell sees is the script's status and
    // the script's stdout, untouched.
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"scripts":{"test":"x"}}"#)
        .file("pnpm-lock.yaml", "")
        .shim("pnpm", "echo 'from the script'\nexit 7");

    let output = fixture.opi(".", &["test"]);
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(stdout(&output), "from the script\n");
}

#[test]
fn a_script_starts_where_opi_was_started() {
    // No directory is set: the package manager finds its own root by walking
    // up, as it would if typed by hand, and `cargo run` depends on this.
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"scripts":{"dev":"x"}}"#)
        .file("package-lock.json", "{}")
        .shim("npm", "exit 0");

    let output = fixture.opi("src/pages", &["dev"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        fixture.calls("npm").expect("npm ran")[0],
        canonical(fixture.project().join("src/pages"))
    );
}

#[test]
fn a_missing_package_manager_is_named_rather_than_crashed_on() {
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"scripts":{"build":"x"}}"#)
        .file("pnpm-lock.yaml", "");

    let output = fixture.opi(".", &["build"]);
    assert_eq!(output.status.code(), Some(1));
    let said = stderr(&output);
    assert!(said.contains("Cannot run pnpm"), "{said}");
    assert!(said.contains("installed and on your PATH"), "{said}");
}

#[test]
fn a_cargo_task_covers_the_whole_workspace_from_inside_a_member() {
    // A root that is also a package: without --workspace, cargo would test
    // the root package alone.
    let fixture = Fixture::new();
    fixture
        .file(
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/*\"]\n\n[package]\nname = \"repo\"\n",
        )
        .file("crates/app/Cargo.toml", "[package]\nname = \"app\"\n")
        .shim("cargo", "exit 0");

    let output = fixture.opi("crates/app", &["cargo test"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let calls = fixture.calls("cargo").expect("cargo ran");
    assert_eq!(calls[1..], ["test", "--all-features", "--workspace"]);
}

// --- Piped rather than read ---

#[test]
fn a_piped_list_is_plain_text() {
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"scripts":{"dev":"x","build":"y"}}"#)
        .file("package-lock.json", "{}");

    let output = fixture.opi(".", &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    let printed = stdout(&output);
    assert!(
        printed.contains("dev") && printed.contains("build"),
        "{printed}"
    );
    assert!(!printed.contains('\u{1b}'), "no escape codes into a pipe");
}

// --- Health ---

#[test]
fn a_members_tool_runs_in_the_member_even_without_scripts() {
    // A library with no scripts still has something to lint, and its config
    // lives beside its sources, not at the root.
    let fixture = Fixture::new();
    fixture
        .file(
            "package.json",
            r#"{"name":"root","workspaces":["packages/*"]}"#,
        )
        .file("package-lock.json", "{}")
        .file(
            "packages/lib/package.json",
            r#"{"name":"lib","devDependencies":{"eslint":"9.0.0"}}"#,
        )
        .tool("node_modules/.bin/eslint", "echo '1 problem'\nexit 1");

    let output = fixture.opi(".", &["--health"]);
    assert_eq!(output.status.code(), Some(1));
    let printed = stdout(&output);
    assert!(printed.contains("Lint (lib)"), "{printed}");
    assert!(printed.contains("1 problem"), "{printed}");
    assert_eq!(
        fixture.calls("eslint").expect("eslint ran")[0],
        canonical(fixture.project().join("packages/lib"))
    );
}

#[test]
fn a_yarn_pnp_project_is_checked_through_yarn() {
    let fixture = Fixture::new();
    fixture
        .file(
            "package.json",
            r#"{"name":"pnp","devDependencies":{"eslint":"9.0.0"}}"#,
        )
        .file("yarn.lock", "")
        .shim("yarn", "exit 0");

    let output = fixture.opi(".", &["--health"]);
    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(
        fixture.calls("yarn").expect("yarn ran")[1..],
        ["run", "eslint", "."]
    );
}

#[test]
fn a_lockfile_out_of_step_with_its_manifest_fails_health() {
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"name":"app","dependencies":{"ms":"2.1.3"}}"#)
        .file("pnpm-lock.yaml", "")
        .shim(
            "pnpm",
            "echo ' ERR_PNPM_OUTDATED_LOCKFILE  Cannot install with \"frozen-lockfile\" because pnpm-lock.yaml is not up to date with package.json' >&2\nexit 1",
        );

    let output = fixture.opi(".", &["--health"]);
    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{printed}");
    assert!(printed.contains("Lockfile"), "{printed}");
    assert!(printed.contains("ERR_PNPM_OUTDATED_LOCKFILE"), "{printed}");
    assert_eq!(
        fixture.calls("pnpm").expect("pnpm ran")[1..],
        [
            "install",
            "--frozen-lockfile",
            "--lockfile-only",
            "--ignore-scripts",
            "--offline"
        ]
    );
}

#[test]
fn the_commit_workflow_asks_about_the_lockfile() {
    // A dependency added to package.json and never locked is exactly what a
    // commit should not carry, and the check is offline and fast enough to
    // sit there.
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"name":"app"}"#)
        .file("package-lock.json", "{}")
        .shim("npm", "exit 0");

    let output = fixture.opi(".", &["--check", "commit"]);
    let printed = stdout(&output);
    assert!(output.status.success(), "{printed}");
    assert!(printed.contains("Lockfile"), "{printed}");
    assert_eq!(
        fixture.calls("npm").expect("npm ran")[1..],
        ["ci", "--dry-run", "--ignore-scripts"]
    );
}

// --- Security ---

#[test]
fn a_failed_audit_is_not_reported_as_clean() {
    // The false acquittal: no report on stdout, the reason on stderr. Read as
    // "no findings", this said "Nothing to act on." and exited 0.
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"name":"app"}"#)
        .file("package-lock.json", "{}")
        .shim(
            "npm",
            "echo 'npm error code ENOTFOUND registry.npmjs.org' >&2\nexit 1",
        );

    let output = fixture.opi(".", &["--security"]);
    let printed = stdout(&output);
    assert_ne!(output.status.code(), Some(0), "{printed}");
    assert!(printed.contains("ENOTFOUND"), "{printed}");
    assert!(!printed.contains("no known vulnerabilities"), "{printed}");
    assert!(!printed.contains("Nothing to act on"), "{printed}");
}

#[test]
fn a_clean_yarn_audit_prints_nothing_and_is_clean() {
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"name":"app"}"#)
        .file("yarn.lock", "")
        .shim("yarn", "exit 0");

    let output = fixture.opi(".", &["--security"]);
    let printed = stdout(&output);
    assert!(output.status.success(), "{printed}");
    assert!(printed.contains("no known vulnerabilities"), "{printed}");
    assert_eq!(
        fixture.calls("yarn").expect("yarn ran")[1..],
        ["npm", "audit", "--all", "--recursive", "--json"]
    );
}

#[test]
fn a_yarn_advisory_is_reported_and_fails_the_run() {
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"name":"app"}"#)
        .file("yarn.lock", "")
        .shim(
            "yarn",
            r#"printf '%s\n' '{"value":"lodash","children":{"ID":1106913,"Severity":"high","Vulnerable Versions":"<4.17.21","Dependents":["app@workspace:."]}}'
exit 1"#,
        );

    let output = fixture.opi(".", &["--security"]);
    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{printed}");
    assert!(printed.contains("lodash"), "{printed}");
}

// --- Updates ---

#[test]
fn a_failed_outdated_is_not_everything_is_current() {
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"name":"app"}"#)
        .file("package-lock.json", "{}")
        .shim("npm", "echo 'npm error network request failed' >&2\nexit 1");

    let output = fixture.opi(".", &["--updates"]);
    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{printed}");
    assert!(printed.contains("network request failed"), "{printed}");
    assert!(!printed.contains("everything is current"), "{printed}");
}

#[test]
fn a_silent_outdated_is_everything_is_current() {
    // npm prints nothing at all when nothing is outdated — measured — so
    // silence on both streams is the clean case, not a failure.
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"name":"app"}"#)
        .file("package-lock.json", "{}")
        .shim("npm", "exit 0");

    let output = fixture.opi(".", &["--updates"]);
    let printed = stdout(&output);
    assert!(output.status.success(), "{printed}");
    assert!(printed.contains("everything is current"), "{printed}");
}

#[test]
fn bun_is_never_asked_to_update_without_a_terminal() {
    // The handover to `bun update --interactive` is an offer; a pipe gets the
    // offer's absence explained, and bun is not started at all.
    let fixture = Fixture::new();
    fixture
        .file("package.json", r#"{"name":"app"}"#)
        .file("bun.lock", "")
        .shim("bun", "exit 0");

    let output = fixture.opi(".", &["--updates"]);
    let printed = stdout(&output);
    assert!(output.status.success(), "{printed}");
    assert!(printed.contains("in a terminal"), "{printed}");
    assert!(
        fixture.calls("bun").is_none(),
        "bun update ran without asking"
    );
}
