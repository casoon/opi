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
//! Windows has no `exec`, which is why `opi` is Unix-only. Supervising the
//! child instead would mean a second execution model that nothing here tests,
//! and a `Ctrl-C` that kills the wrapper rather than the dev server.

use std::io;
use std::process::Command;

use crate::project::PackageManager;

/// Runs `script` through `manager`, forwarding `args` to it.
///
/// Returns only when the command could not be started at all; on success this
/// process has been replaced by the script.
pub fn execute(
    manager: PackageManager,
    script: &str,
    workspace: Option<&str>,
    args: &[String],
) -> io::Error {
    use std::os::unix::process::CommandExt;

    let mut command = Command::new(manager.program());
    command.args(manager.run_args(script, workspace, args));
    // Returns only on failure.
    command.exec()
}
