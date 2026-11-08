//! Installs the hooks that run opi's checks.

use std::path::Path;
use std::process::ExitCode;

use crate::workflow;
use crate::write_block;
use runemark::{ColorMode, Console, ErrorBlock, Tone};

/// Installs `opi --check push` as the repository's pre-push hook.
pub(crate) fn hooks(root: &Path) -> ExitCode {
    let console = Console::stdout(ColorMode::Auto);
    let (message, hook) = match workflow::install_hook(root) {
        Ok(workflow::HookInstall::Created(hook)) => ("Installed the pre-push hook", hook),
        Ok(workflow::HookInstall::Appended(hook)) => ("Added opi to the pre-push hook", hook),
        Ok(workflow::HookInstall::Present(hook)) => ("The pre-push hook already runs opi", hook),
        Err(error) => {
            let block = match error {
                workflow::HookError::NotARepository => ErrorBlock::new("Not a git repository")
                    .with_remedy("Hooks belong to a repository; run git init first."),
                workflow::HookError::Io(error) => {
                    ErrorBlock::new("Cannot write the hook").with_explanation(format!("{error}"))
                }
            };
            write_block(&block, Console::stderr(ColorMode::Auto));
            return ExitCode::FAILURE;
        }
    };
    let shown = hook.strip_prefix(root).map_or_else(
        |_| hook.display().to_string(),
        |path| path.display().to_string(),
    );
    println!(
        "{} {}  {}",
        console.paint(Tone::Success, "✓"),
        console.paint(Tone::Success, message),
        console.paint(Tone::Muted, shown)
    );
    println!(
        "{}",
        console.paint(
            Tone::Muted,
            "Every push now runs `opi --check push`; `git push --no-verify` skips it."
        )
    );
    ExitCode::SUCCESS
}
