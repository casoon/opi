//! Command line parsing.
//!
//! Deliberately hand-rolled and tiny. The command line has one shape —
//! `opi [flags] [script] [script args…]` — and the interesting rule is not
//! parsing but precedence: **a project's scripts win over anything built in.**
//! A project with a script called `health` or `clean` is entirely plausible,
//! and `opi` must not break in exactly the kind of project it exists to improve.
//! Built-in actions are therefore flags and hotkeys, never bare words.

/// What the user asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Invocation {
    /// No script named: show the list.
    List,
    /// Run `name`, passing `args` on to it.
    Run {
        name: String,
        args: Vec<String>,
        /// A confirmation was given in advance, with `--yes`.
        confirmed: bool,
    },
    Help,
    Version,
    /// Run the project's checks.
    Health,
    /// Show and remove build artefacts.
    Clean,
    /// Scan for secrets and vulnerable dependencies.
    Security,
    /// Run a named workflow.
    Workflow(String),
    /// Show dependencies with newer versions.
    Updates,
    /// Install the push workflow as the pre-push hook.
    Hooks,
    /// A flag `opi` does not know, before any script name.
    UnknownFlag(String),
}

/// Parses arguments, excluding the executable name.
///
/// The first bare word is the script. Everything after it belongs to the
/// script, including anything that looks like a flag — `opi build --verbose`
/// passes `--verbose` to `build`, it does not configure `opi`. A literal `--`
/// before the script args is accepted and dropped, since that is the habit
/// `npm run` teaches.
pub fn parse<I, S>(args: I) -> Invocation
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let mut confirmed = false;

    // `--yes` is the one flag that leaves the rest of the line still addressed
    // to opi; everything else either is the script or terminates the parse.
    let first = loop {
        let Some(arg) = args.next() else {
            return Invocation::List;
        };
        if arg == "--yes" || arg == "-y" {
            confirmed = true;
            continue;
        }
        break arg;
    };

    match first.as_str() {
        "-h" | "--help" => Invocation::Help,
        "-V" | "--version" => Invocation::Version,
        // A flag, not a bare word: a project with a script called "health"
        // must keep "opi health" meaning its own.
        "--health" => Invocation::Health,
        "--clean" => Invocation::Clean,
        "--security" => Invocation::Security,
        "--updates" => Invocation::Updates,
        "--hooks" => Invocation::Hooks,
        // The name follows the flag rather than standing alone, so a project
        // script called "commit" or "release" keeps its word.
        "--check" => args
            .next()
            .map_or(Invocation::UnknownFlag("--check".to_owned()), |name| {
                Invocation::Workflow(name)
            }),
        _ if first.starts_with('-') => Invocation::UnknownFlag(first),
        _ => {
            let mut rest: Vec<String> = args.collect();
            if rest.first().is_some_and(|arg| arg == "--") {
                rest.remove(0);
            }
            Invocation::Run {
                name: first,
                args: rest,
                confirmed,
            }
        }
    }
}

/// The `--help` text.
pub fn help(version: &str) -> String {
    format!(
        "opi {version} — Operations Interface

Usage:
  opi                     List what this project can do
  opi <script> [args…]    Run a script, passing args on to it
  opi <member>/<script>   Run a workspace member's script

In the list, ↑↓ move, Enter runs, / filters, H checks, C cleans,
S scans for secrets and vulnerable dependencies, U shows updates.

Options:
  -y, --yes               Answer a script's confirmation in advance
      --health            Run the project's checks
      --clean             Show and remove build artefacts
      --security          Scan for secrets and vulnerable dependencies
      --updates           Show dependencies with newer versions
      --check <workflow>  Run a workflow: commit, push or release
      --hooks             Run the push workflow as the pre-push hook
  -h, --help              Show this help
  -V, --version           Show the version

Entries come from package.json and Cargo.toml. A project's own scripts
always take precedence over built-in names, so a project with a script
called \"clean\" keeps working — every built-in area is a flag or a
hotkey instead."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(name: &str, args: &[&str]) -> Invocation {
        Invocation::Run {
            name: name.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            confirmed: false,
        }
    }

    fn confirmed(name: &str) -> Invocation {
        Invocation::Run {
            name: name.to_owned(),
            args: Vec::new(),
            confirmed: true,
        }
    }

    #[test]
    fn no_arguments_lists() {
        assert_eq!(parse(Vec::<String>::new()), Invocation::List);
    }

    #[test]
    fn a_bare_word_is_a_script() {
        assert_eq!(parse(["dev"]), run("dev", &[]));
    }

    #[test]
    fn arguments_after_the_script_belong_to_the_script() {
        assert_eq!(
            parse(["build", "--verbose", "--out", "dist"]),
            run("build", &["--verbose", "--out", "dist"])
        );
    }

    #[test]
    fn a_separator_before_script_args_is_dropped() {
        assert_eq!(
            parse(["build", "--", "--verbose"]),
            run("build", &["--verbose"])
        );
    }

    #[test]
    fn only_the_first_separator_is_dropped() {
        assert_eq!(
            parse(["build", "--", "--", "-x"]),
            run("build", &["--", "-x"])
        );
    }

    #[test]
    fn a_script_named_like_a_flag_word_still_wins() {
        // "help" as a script name must reach the project, not opi.
        assert_eq!(parse(["help"]), run("help", &[]));
        assert_eq!(parse(["version"]), run("version", &[]));
    }

    #[test]
    fn help_and_version_are_recognised() {
        assert_eq!(parse(["--help"]), Invocation::Help);
        assert_eq!(parse(["-h"]), Invocation::Help);
        assert_eq!(parse(["--version"]), Invocation::Version);
        assert_eq!(parse(["-V"]), Invocation::Version);
    }

    #[test]
    fn yes_answers_in_advance_and_leaves_the_script_alone() {
        assert_eq!(parse(["--yes", "deploy"]), confirmed("deploy"));
        assert_eq!(parse(["-y", "deploy"]), confirmed("deploy"));
        assert_eq!(parse(["deploy"]), run("deploy", &[]));
    }

    #[test]
    fn yes_after_the_script_belongs_to_the_script() {
        assert_eq!(parse(["deploy", "--yes"]), run("deploy", &["--yes"]));
    }

    #[test]
    fn yes_alone_still_lists() {
        assert_eq!(parse(["--yes"]), Invocation::List);
    }

    #[test]
    fn a_workflow_is_named_after_its_flag() {
        assert_eq!(
            parse(["--check", "release"]),
            Invocation::Workflow("release".to_owned())
        );
        // "release" is a script name in real projects; the bare word is theirs.
        assert_eq!(parse(["release"]), run("release", &[]));
    }

    #[test]
    fn a_workflow_flag_without_a_name_is_rejected() {
        assert_eq!(
            parse(["--check"]),
            Invocation::UnknownFlag("--check".to_owned())
        );
    }

    #[test]
    fn built_in_areas_are_flags_so_a_script_can_own_the_word() {
        assert_eq!(parse(["--health"]), Invocation::Health);
        assert_eq!(parse(["--clean"]), Invocation::Clean);
        assert_eq!(parse(["--security"]), Invocation::Security);
        assert_eq!(parse(["--updates"]), Invocation::Updates);
        assert_eq!(parse(["--hooks"]), Invocation::Hooks);
        // "clean" is a script name in 41 of 133 measured projects.
        assert_eq!(parse(["health"]), run("health", &[]));
        assert_eq!(parse(["clean"]), run("clean", &[]));
    }

    #[test]
    fn an_unknown_leading_flag_is_reported() {
        assert_eq!(
            parse(["--nope"]),
            Invocation::UnknownFlag("--nope".to_owned())
        );
    }

    #[test]
    fn quoted_arguments_survive_as_single_values() {
        assert_eq!(
            parse(["test", "--grep", "two words"]),
            run("test", &["--grep", "two words"])
        );
    }
}
