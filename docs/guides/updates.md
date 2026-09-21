---
title: Updates
description: What is outdated, split into safe and major — and the offer to take it, followed by the commit checks.
order: 6
---

`U` in the list, or:

```sh
opi --updates
```

`opi` asks the package manager (`outdated --json`) and, for Rust, `cargo outdated
--root-deps-only`, and sorts the answer by semver jump: what is safe to take, and what is a
major. A `0.x` minor counts as breaking, in both ecosystems.

Run on `opi`'s own repository, piped:

```
opi  updates

– Dependencies — no package.json here

[INFO] Dependencies (rust)

  safe: 1

* Safe to take (1)
  - runemark 0.8.0 → 0.8.1 [patch]
```

A repository with both manifests gets two sections, `Dependencies` and
`Dependencies (rust)`, because "safe to take" orders within an ecosystem and not across two.
In a pnpm workspace `opi` asks with `-r`, so members' dependencies are included.

## Taking the updates

In a terminal, `--updates` offers to apply what it found — and then runs the
[commit checks](../workflows/) on what comes back. That chain — update, install, check, what is
red now — is the reason the action exists.

The choice offered is **staying inside the declared ranges** against **raising them**, each
line with its own count and omitted when that count is zero. Majors are never included.
Raising ranges is offered for pnpm only, because npm has no command that keeps the range
operator.

`opi` writes nothing itself. It calls the package manager with a list of names — no
`package.json`, no lockfile is touched by `opi`. Where two lockfiles disagree and no
`packageManager` field settles it, the offer is withheld and the finding named instead. Rust's
section carries no next step, because `cargo update` writes the lockfile.
