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

**The script list works; the maintenance areas do not.** `opi` detects the
project, groups its `package.json` scripts and runs the one you pick, either
from an interactive list or by name. Health, dependency updates, clean,
security and the fuzzy search are not built.

`0.0.1` on crates.io is a placeholder that holds the crate name and predates
all of this. Nothing functional has been released yet.

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

`conventions.md` is still absent. It records the rules future changes follow,
and those are better written once the first release has settled what they are.
