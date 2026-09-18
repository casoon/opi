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

**Project detection works; nothing else does.** `src/project.rs` establishes the
project name, the package manager and the Node version; `src/main.rs` renders
that and exits. There is no script list, no task execution and no maintenance
area yet.

`0.0.1` is published to crates.io and holds the crate name. It predates the
detection layer and contains no functionality at all.

Most of what is described as a decision or constraint below is therefore still a
statement of intent rather than something validated against working code.

The build plan lives in the gitignored `plan/` directory. The first functional
release (`0.1.0`) is scoped to project detection, the grouped script list,
interactive selection and direct execution — nothing else.

## Target stack

| | |
| --- | --- |
| Language | Rust, edition 2024 |
| Distribution | crates.io as `opi`, installed via `cargo install opi` |
| Terminal presentation | [runemark](https://github.com/casoon/runemark) — the only presentation dependency |
| Project input | `package.json` — no separate config file |

## Documentation in this directory

- [constraints.md](constraints.md) — the hard boundaries the implementation must
  respect
- [decisions.md](decisions.md) — the decisions currently in force, and why

`architecture.md` and `conventions.md` are intentionally absent. Both describe
how code is actually structured and written, and there is no code yet. They are
written alongside the first implementation rather than invented in advance.
