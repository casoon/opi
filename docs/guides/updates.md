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

✓ Dependencies (rust) — everything is current
```

Where something is outdated, each line carries its jump — `- runemark 0.8.0 → 0.8.1
[patch]` — under `Safe to take` or `A decision each`.

A repository with both manifests gets two sections, `Dependencies` and
`Dependencies (rust)`, because "safe to take" orders within an ecosystem and not across two.
In a pnpm workspace `opi` asks with `-r`, so members' dependencies are included.

## Taking the updates

In a terminal, `--updates` offers to apply what it found — and then runs the
[commit checks](../workflows/) on what comes back. That chain — update, install, check, what is
red now — is the reason the action exists.

Three lines are offered. **Staying inside the declared ranges** against **raising them**
answers in bulk, each with its own count and omitted when that count is zero; neither
includes a major. **Decide per package** lists every finding as a ticked line, majors among
them — for the case the bulk lines cannot reach: one major you have read the release notes
for, beside a dozen minors you have not thought about. The safe ones start ticked and the
majors do not, so confirming without touching anything does what "Raise the ranges" does,
and a major is only ever taken deliberately. Leaving the list without confirming applies
nothing, and an empty selection is a no-op rather than a call with no names.

The bulk split is staying inside the ranges against raising them, rather than the list's own
safe-against-breaking one, because that ordering is the wrong question here. Measured: three
dependencies, two of them "safe", and `pnpm update` moved none — an exact pin and a `~` range
each already had what they asked for. As a line to retype nobody notices; as a key it would be
an action that does nothing and reports success.

Raising ranges — and with it the per-package list — is offered for pnpm only, because npm
has no command that keeps the range operator: `npm install x@1.1.1` turns an exact `2.1.2`
into `^2.1.3`.

bun's `outdated` prints a table rather than JSON, so `opi` has no list of its own there.
Instead it offers **Choose in bun's own list**, which opens `bun update --interactive` —
with `-r` in a workspace, so members' packages are included — and runs the commit checks
once bun is done. yarn is not offered anything: Yarn Berry has no `outdated` at all.

`opi` writes nothing itself. It calls the package manager with a list of names — no
`package.json`, no lockfile is touched by `opi`. Where two lockfiles disagree and no
`packageManager` field settles it, the offer is withheld and the finding named instead, since
writing with the wrong manager leaves a lockfile the team did not ask for. Without a terminal
nothing is applied at all. Rust's section carries no next step, because `cargo update` writes
the lockfile — the line `--updates` does not cross.

That `opi` applies anything is itself a reversal. The rule used to be that updates are never
applied, because pnpm catalogs put versions outside `package.json`; measured, pnpm rewrites
the catalog correctly, and **45 of 79 workspaces here use one** — the exception was the rule.

## Limits

**A pnpm workspace has to be asked with `-r`.** Asked at the root alone, `pnpm outdated`
found 2 of 11 outdated packages on one real repository — and where a root declares no
dependencies of its own it answers `{}`, which reads as "everything is current" over stale
packages. 79 of 133 npm projects measured here are pnpm workspaces. The same trap applies to
`bun update --interactive`, measured with bun 1.3.3: without `-r` the interactive list leaves
the members' packages off.

**npm needs none of that.** It walks the installed tree rather than the manifests and already
sees every member, measured both ways on a two-member workspace.

**pnpm's JSON is keyed by package name**, so a package present at two versions in two members
shows one of them. `pnpm outdated -r` prints both rows itself.

**`Jump` never learns where a version came from.** Cargo treating `0.12 → 0.13` as breaking
needed no rule of its own: that rule was already there, and already applied to npm.
