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

**No functionality is implemented yet.** The repository holds a crate skeleton:
`Cargo.toml` with its registry metadata, a `src/main.rs` that prints the version
and a development notice through runemark, the supporting repository files, and
CI and release workflows. That is version `0.0.1`, published to crates.io with
the sole purpose of holding the crate name.

Everything below described as a decision or constraint is therefore a statement
of intent, not yet validated against working code.

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
