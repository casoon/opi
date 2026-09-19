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
use crate::task::{Exec, Task};

/// Runs `script` through `manager`, forwarding `args` to it.
///
/// Returns only when the command could not be started at all; on success this
/// process has been replaced by the script.
pub fn execute(task: &Task, manager: PackageManager, args: &[String]) -> io::Error {
    use std::os::unix::process::CommandExt;

    let mut command = match &task.exec {
        Exec::Script => {
            let mut command = Command::new(manager.program());
            command.args(manager.run_args(&task.name, task.workspace.as_deref(), args));
            command
        }
        Exec::Direct {
            program,
            args: fixed,
        } => {
            let mut command = Command::new(program);
            command.args(fixed).args(args);
            command
        }
    };
    // Returns only on failure.
    command.exec()
}
