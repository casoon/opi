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
    },
    Help,
    Version,
    /// Run the project's checks.
    Health,
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

    // Only the first argument can address opi itself; there are no opi flags
    // that leave the rest of the line still addressed to opi.
    let Some(first) = args.next() else {
        return Invocation::List;
    };

    match first.as_str() {
        "-h" | "--help" => Invocation::Help,
        "-V" | "--version" => Invocation::Version,
        // A flag, not a bare word: a project with a script called "health"
        // must keep "opi health" meaning its own.
        "--health" => Invocation::Health,
        _ if first.starts_with('-') => Invocation::UnknownFlag(first),
        _ => {
            let mut rest: Vec<String> = args.collect();
            if rest.first().is_some_and(|arg| arg == "--") {
                rest.remove(0);
            }
            Invocation::Run {
                name: first,
                args: rest,
            }
        }
    }
}

/// The `--help` text.
pub fn help(version: &str) -> String {
    format!(
        "opi {version} — Operations Interface

Usage:
  opi                     List the project's scripts
  opi <script> [args…]    Run a script, passing args on to it

In the list, ↑↓ move, Enter runs, / filters, H checks the project.

Options:
      --health            Run the project's checks
  -h, --help              Show this help
  -V, --version           Show the version

Scripts come from package.json and always take precedence over built-in
names, so a project with a script called \"help\" keeps working."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(name: &str, args: &[&str]) -> Invocation {
        Invocation::Run {
            name: name.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
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
    fn health_is_a_flag_so_a_script_can_own_the_word() {
        assert_eq!(parse(["--health"]), Invocation::Health);
        assert_eq!(
            parse(["health"]),
            run("health", &[]),
            "a project script named health still wins"
        );
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
