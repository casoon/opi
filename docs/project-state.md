# Project state

## What `opi` is

`opi` — **Operations Interface** — is an installable binary that answers one
question in any repository: *what can and what should I do with this project
right now?*

Its primary function is to make a project's scripts immediately usable —
grouped, labelled, keyboard-driven, with the package manager detected rather
than typed. Everything else sits behind a hotkey, deliberately secondary.

`opi` reimplements none of the tools it surfaces. It detects which one a
project depends on, runs it, and relays the result.

## What it does

`opi` finds the nearest `package.json` by searching upwards, groups its
scripts, adds any workspace members' scripts, and runs the one you pick — from
an interactive list, by name, or by filtering with `/`.

`Cargo.toml` is read the same way, and a repository may be both at once: the
list, health and clean cover npm and Rust together rather than choosing one.

Past 15 entries or 5 groups the list shows its groups as a row of tabs and
lists only the active one, with the first group preselected so `opi ↵` runs
what it always ran. A line under the heading counts what was found.

Five areas sit beside the list:

| Area | Key | Flag |
| --- | --- | --- |
| Health | `H` | `--health` |
| Security | `S` | `--security` |
| Updates | `U` | `--updates` |
| Clean | `C` | `--clean` |
| Workflows | — | `--check commit`, `--check release` |

A project can say more about a script through the `opi` key in `package.json`:
a description that wins over `scripts-info`, an explicit group, a favourite, a
confirmation before running. All optional — a project with none of it still
gets a usable list, which is the point of the whole arrangement.

## What it does not do

- **Apply updates.** `--updates` reports and stops. See
  [constraints.md](constraints.md).
- **Run checks in a fixing mode.** Nothing writes but clean.
- **.NET.** No standalone marker appeared in the 231 directories measured.
- **Windows.** See [decisions.md](decisions.md).

## Stack

| | |
| --- | --- |
| Language | Rust, edition 2024, MSRV `1.85` |
| Distribution | crates.io as `opi`, installed via `cargo install opi` |
| Terminal presentation | [runemark](https://github.com/casoon/runemark) `0.7` with its `select` feature — the only presentation dependency |
| Project input | `package.json` and `Cargo.toml` — no config file of `opi`'s own |
| Platforms | Unix only; building on Windows fails with an explicit message |
| Dependencies | four: `runemark`, `serde`, `serde_json`, `glob` |

## Documentation in this directory

- [architecture.md](architecture.md) — the modules that exist and how they fit
- [constraints.md](constraints.md) — the boundaries the implementation respects
- [conventions.md](conventions.md) — the rules future changes follow
- [decisions.md](decisions.md) — the decisions in force, and why

The working backlog lives in the gitignored `plan/` directory, which is not
part of this documentation and not published with the crate.
