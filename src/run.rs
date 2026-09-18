//! Running a script.
//!
//! On Unix the child *replaces* `opi` through `exec`, rather than being
//! supervised by it. That settles the awkward parts of running a long-lived
//! process for free: `Ctrl-C` reaches the dev server rather than killing the
//! wrapper and orphaning it, stdio is the child's own (so a dev server's
//! coloured output and prompts are untouched), and the exit code is the
//! child's, so `opi build && …` behaves in a shell chain.
//!
//! It also matches the intended behaviour: once a script runs, `opi` is done
//! and does not return to its list.
//!
//! Windows has no `exec`, so there the child is spawned and waited on.

use std::io;
use std::process::Command;

use crate::project::PackageManager;

/// Runs `script` through `manager`, forwarding `args` to it.
///
/// Returns only when the command could not be started at all; on success this
/// process has been replaced (Unix) or exited with the child's code (Windows).
pub fn execute(manager: PackageManager, script: &str, args: &[String]) -> io::Error {
    let mut command = Command::new(manager.program());
    command.args(manager.run_args(script, args));
    replace(command)
}

#[cfg(unix)]
fn replace(mut command: Command) -> io::Error {
    use std::os::unix::process::CommandExt;
    // Returns only on failure.
    command.exec()
}

#[cfg(not(unix))]
fn replace(mut command: Command) -> io::Error {
    match command.status() {
        // Mirror the child rather than truncating its code into a u8.
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => error,
    }
}
