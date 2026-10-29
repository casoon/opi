---
title: Project state
description: "What opi is, what it does and does not do, and where it stands."
order: 1
---

## What `opi` is

`opi` — **Operations Interface** — is an installable binary that answers one
question in any repository: *what can and what should I do with this project
right now?*

Its primary function is to make a project's scripts immediately usable —
grouped, labelled, keyboard-driven, with the package manager detected rather
than typed. Everything else sits behind a hotkey, deliberately secondary.

`opi` reimplements none of the tools it surfaces. It detects which one a
project depends on, runs it, and relays the result. A bundled nosecrets engine supplies
secret scanning when the project root has no detected scanner. The commit workflow also
checks Git's staged changes for whitespace errors, conflict markers and oversized additions.

## What it does

`opi` finds the nearest `package.json` by searching upwards, groups its
scripts, adds any workspace members' scripts, and runs the one you pick — from
an interactive list, by name, or by filtering with `/`.

`Cargo.toml` is read the same way, and a repository may be both at once: the
list, health and clean cover npm and Rust together rather than choosing one.

Past 15 entries or 5 groups the list shows its groups as a row of tabs and
lists only the active one, with the first group preselected so `opi ↵` runs
what it always ran. A line under the heading counts what was found.

Every area now answers for both ecosystems where both are present, in a section
each: `cargo audit` and `cargo outdated` beside the package manager's own.
All four package managers are audited — npm and pnpm share a format, bun and
yarn each have their own. `--updates` can also apply what it found by calling
the package manager — the safe ones in one step, or each package ticked on its
own where a major is worth taking — and runs the commit checks on the result.
For bun, whose `outdated` `opi` cannot read, it hands over to
`bun update --interactive` instead.

Five areas sit beside the list:

| Area | Key | Flag |
| --- | --- | --- |
| Health | `H` | `--health` |
| Security | `S` | `--security` |
| Updates | `U` | `--updates` |
| Clean | `C` | `--clean` |
| Workflows | — | `--check commit`, `--check push`, `--check release`, `--check deploy`, `--checks`, `--hooks` |

A project can say more about a script through the `opi` key in `package.json`:
a description that wins over `scripts-info`, an explicit group, a favourite, a
confirmation before running. All optional — a project with none of it still
gets a usable list, which is the point of the whole arrangement.

## What it does not do

- **Write a project's files.** Clean removes what it is told to and
  `--updates` calls the package manager with a list of names; `opi` itself
  writes no manifest, no lockfile and no catalog, and no check runs in a
  fixing mode. Exceptions are hook installation and optional workflow selections:
  `opi --checks` saves `.opi/checks.json` and its Git ignore exceptions. See [Constraints](../constraints/).
- **.NET.** No standalone marker appeared in the 231 directories measured.
- **Windows.** See [Decisions](../decisions/).

## Stack

| | |
| --- | --- |
| Language | Rust, edition 2024, MSRV `1.85` |
| Distribution | `install.sh` fetches the release binary and checks its SHA-256; `cargo binstall opi` fetches the same release binary; crates.io as `opi` via `cargo install opi` for those with a toolchain |
| Terminal presentation | [runemark](https://github.com/casoon/runemark) `0.9` with its `select` feature — the only presentation dependency |
| Project input | `package.json` and `Cargo.toml` for commands; optional `.opi/checks.json` for workflow selections |
| Platforms | Unix only; building on Windows fails with an explicit message |
| Dependencies | `runemark`, `serde`, `serde_json`, `glob`, and the nosecrets core, filter and rules crates |

## Documentation in this directory

- [Architecture](../architecture/) — the modules that exist and how they fit
- [Constraints](../constraints/) — the boundaries the implementation respects
- [Conventions](../conventions/) — the rules future changes follow
- [Decisions](../decisions/) — the decisions in force, and why

The working backlog lives in the gitignored `plan/` directory, which is not
part of this documentation and not published with the crate.

`demo/` is part of the repository but not of the crate: it holds fixture
projects and the scripted scenes the README's recordings are made from, and is
excluded from the tarball together with `assets/`. Nothing in it is reachable
from the binary.
