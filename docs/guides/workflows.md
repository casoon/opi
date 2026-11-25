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

## What a workflow remembers

Each run is recorded in `.git/opi/checks.jsonl`: the commit, the tree when nothing was
uncommitted, the result of every check, how long it took and which opi ran it. The file lives in
git's own directory, so it is never committed and belongs to the one clone (its worktrees share
it). Deleting it loses nothing but the shortcut below.

**A tree that already passed `push` is not checked again.** When the working tree is clean and
exactly that tree passed the push workflow with the same opi version, the gates still run, and
the checks are reported as passed instead of being repeated:

```
opi  before push

✓ Working tree clean
✓ Not behind upstream
✓ Checks  passed on this tree 4 min ago (fd999c0)

Ready to push.
```

So the second push of the same commits, to another remote or after a rejected push, costs a
second instead of minutes. A new commit, any uncommitted change or a newer opi runs everything
again. With `--json`, the workflow always runs fresh. Dependencies are not part of the tree:
after installing new ones, delete the file to have the next push check again.

The start screen names the last run on the commit checked out, after the entry count:
`✓ push 12 min ago`, or `✗ push: Tests (rust) 3 min ago` with the first check that failed.

## A clean checkout

```sh
opi --check push --clean
```

runs the workflow in a temporary `git worktree` of HEAD instead of the working tree. It checks
what is committed, so a source file you forgot to add, an ignored `.env` the build relies on or
a local edit no longer hide a failure that the next fresh clone would hit. The checkout is
removed again afterwards.

Nothing is installed for it. `node_modules` of the project and its workspace members are linked
in from the working tree, and Rust builds keep the project's `target` directory, so the run costs
a checkout rather than an install. "Working tree clean" is not asked in the fresh checkout, and
"not behind upstream" is asked of your branch before the checkout is made, since the checkout's
HEAD is detached. A tree that already passed is checked anyway: avoiding the working tree is the
point.

It is slower than a normal run, so it is a switch, never the default in the hook. `commit` has
no clean variant, since it checks what is staged.

## Scripts that require a workflow

A script can require that a workflow has passed before it runs:

```json
{
  "opi": { "scripts": { "deploy": { "requires": "push" } } }
}
```

`opi deploy`, or Enter on it in the list, then runs only when exactly the tree checked out has
passed `opi --check push`. Otherwise it stops and names the command to run:

```
[FAIL] deploy requires opi --check push

  fd999c0 has not passed it yet.

  $ opi --check push
```

opi does not start the checks on its own: a deploy that quietly turns into minutes of checks is
opi acting where it should report. Uncommitted changes stop the script too, since what would be
deployed is then not a checked commit. `--yes` answers a `confirm` question, not the
requirement.

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
