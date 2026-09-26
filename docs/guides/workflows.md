---
title: Workflows
description: The fast checks before a commit, everything plus the build before a push or a release, and the pre-push hook that makes it binding.
order: 4
---

A workflow defines no work of its own. It names checks that already exist in
[Health](../health/) and adds the questions about repository state that only make sense
before a commit, a push or a release.

```sh
opi --check commit
opi --check push
opi --check release
```

The name follows the flag, so a project script called `commit`, `push` or `release` keeps its word.

| Workflow | Checks | Gates |
| --- | --- | --- |
| `commit` | Every detected check except Tests and Dead code | — |
| `push` | Every detected check, then the build | Working tree clean, not behind upstream |
| `release` | Every detected check, then the build | Working tree clean, version not yet tagged |

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

Ready to commit.
```

The lockfile check belongs in `commit` on purpose: a dependency added to `package.json` and
never locked is exactly what a commit should not carry, and the check is offline and takes
well under a second.

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
