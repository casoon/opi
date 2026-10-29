---
title: Workflows
description: Project and staged-file checks before a commit, everything plus the build before a push or a release, and the pre-push hook.
order: 4
---

A workflow defines no work of its own. It names checks that already exist in
[Health](../health/) and adds the questions about repository state that only make sense
before a commit, a push, a release or a deployment.

```sh
opi --check commit
opi --check push
opi --check release
opi --check deploy
```

The name follows the flag, so a project script called `commit`, `push` or `release` keeps its word.

| Workflow | Checks | Gates |
| --- | --- | --- |
| `commit` | Every detected check except Tests and Dead code, plus staged-file checks | — |
| `push` | Every detected check, then the build | Working tree clean, not behind upstream |
| `release` | Every detected check, then the build | Working tree clean, version not yet tagged |
| `deploy` | Every detected check, then the build | Working tree clean |

These are the defaults. `opi --checks` or `opi --checks push` / `opi --checks deploy`
lets you select checks and the build for each workflow, saved in `.opi/checks.json`.
The Git gates remain unchanged; [Configuration](../configuration/#workflow-checks)
describes the file and reset behaviour. `opi --check deploy` performs checks only;
`opi deploy` continues to run the project's own deploy script.

The build runs after the other checks and one at a time, since a build beside them would slow
every one of them down. It is the project's own `build` script — or, in a workspace whose root
has none, each member's — and `cargo build --workspace` for Rust, with `--locked` where `Cargo.lock` is committed.

"Not behind upstream" compares with the last fetched state of the upstream branch rather than
asking the remote, so the hook never waits on the network. A branch without an upstream skips
it.

The release gate on the version looks for a `v<version>` tag matching the `version` in
`package.json`. Without a version there it is skipped rather than failed.

Run on `opi`'s own repository:

```
opi  before commit

✓ Lockfile (rust)  cargo
✓ Docs (rust)      cargo
✓ Format (rust)    cargo
✓ Lint (rust)      cargo
✓ Secrets          nosecrets
✓ Staged files     git

Ready to commit.
```

The lockfile check belongs in `commit` on purpose: a dependency added to `package.json` and
never locked is exactly what a commit should not carry, and the check is offline and takes
well under a second.

In a Git repository, `commit` reads staged contents for the bundled nosecrets scanner
and detected nosecrets commands (`scan --staged`). This catches secrets even when an
unstaged edit has already removed them. Other project tools retain their normal scope.
Outside Git, the default scanner checks working-tree files.

The additional **Staged files** check uses `git diff --cached --check` to find whitespace
errors and conflict markers in added lines. It rejects newly added files larger than
500 KiB using their index size; Git LFS pointers fit this limit. Existing large files
are unaffected unless staged as additions. No files are rewritten or staged.
This is not a general end-of-file newline validator or a YAML/JSON parser. Formatters
and linters retain responsibility for their supported file formats. Branch names are
not restricted.

The commit workflow still includes project-wide type, lint and documentation checks;
its duration depends on the project. `opi --hooks` installs only the pre-push hook;
[Hooks and CI](../integrations/#pre-commit) shows how to wire up the commit workflow
with Git, Husky, Lefthook or pre-commit.

A workflow exits non-zero when it is not ready.

## The pre-push hook

```sh
opi --hooks
```

installs `opi --check push` as the repository's pre-push hook, so a repository without CI still
cannot push a red state by accident. It writes to `.husky/pre-push` when the repository has a
`.husky` directory — committed, so every clone gets it — and otherwise wherever git keeps its
hooks. An existing hook keeps its lines; running it twice changes nothing. A project in a
subdirectory of the repository is stepped into first.

A failing push workflow blocks the push. The escape hatch is git's own: `git push --no-verify`.
