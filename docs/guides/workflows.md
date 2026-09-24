---
title: Workflows
description: The fast checks before a commit, everything plus two git gates before a release.
order: 4
---

A workflow defines no work of its own. It names checks that already exist in
[Health](../health/) and adds the questions about repository state that only make sense
before a commit or a release.

```sh
opi --check commit
opi --check release
```

The name follows the flag, so a project script called `commit` or `release` keeps its word.

| Workflow | Checks | Gates |
| --- | --- | --- |
| `commit` | Every detected check except Tests and Dead code | — |
| `release` | Every detected check | Working tree clean, version not yet tagged |

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

A workflow exits non-zero when it is not ready. It is a convenience before pushing, not a
substitute for CI: what is binding stays in CI, where it cannot be skipped.
