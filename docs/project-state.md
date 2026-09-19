# Project state

## What `opi` is

`opi` — **Operations Interface** — is an installable binary that answers one
question in any repository: *what can and what should I do with this project
right now?*

Its primary function is to make a project's `package.json` scripts immediately
usable: grouped, labelled, keyboard-driven, with the package manager detected
rather than typed. Maintenance areas — health, dependency updates, clean,
security — sit behind hotkeys, deliberately secondary to that.

`opi` does not reimplement any of the tools it surfaces. It orchestrates
established ones (`tsc`, ESLint, Biome, Vitest, Knip, No Secrets, `pnpm audit`,
Taze), parses their output and presents it consistently. It is a UX layer over
proven tooling.

## Current status

`opi` finds the nearest `package.json`, groups its scripts, adds any workspace
members' scripts, and runs the one you pick — from an interactive list, by name,
or by filtering with `/`.

Beside that it offers five areas, each reachable by a hotkey in the list and by
a flag: health, security, updates, clean, and the `commit` and `release`
workflows. None of the underlying tools are reimplemented; `opi` detects which
one a project uses, runs it and reports what came back.

**Not built:** per-script metadata in the `opi` key, and ecosystems other than
npm.

`0.0.1` was a placeholder that held the crate name; everything since has been
functional.

See [architecture.md](architecture.md) for how the pieces fit.

The build plan lives in the gitignored `plan/` directory. The first functional
release (`0.1.0`) is scoped to project detection, the grouped script list,
interactive selection and direct execution — nothing else.

## Target stack

| | |
| --- | --- |
| Language | Rust, edition 2024 |
| Distribution | crates.io as `opi`, installed via `cargo install opi` |
| Terminal presentation | [runemark](https://github.com/casoon/runemark) `0.4` with its `select` feature — the only presentation dependency |
| Project input | `package.json` — no separate config file |
| Platforms | Unix only; building on Windows fails with an explicit message |

## Documentation in this directory

- [constraints.md](constraints.md) — the hard boundaries the implementation must
  respect
- [decisions.md](decisions.md) — the decisions currently in force, and why

- [architecture.md](architecture.md) — the modules that exist and how they fit
- [conventions.md](conventions.md) — the rules future changes follow
