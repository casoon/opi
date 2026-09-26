---
title: Conventions
description: "Rules future changes follow: validation, releasing, modules, output, errors and tests."
order: 4
---

Rules future changes follow. Everything here is visible in the code as it
stands; where a rule is a decision rather than an observation, it is in
[decisions.md](decisions.md) instead.

## Validation

The gates, matching runemark's:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo package --locked
```

CI runs these plus `cargo doc` with warnings denied, on `1.85` and stable
across Linux and macOS.

Run `cargo package` last. Its verification build shares `target/debug` and
rewrites the binary's dep-info to point at the packaged copy of the sources,
so afterwards cargo considers `target/debug/opi` fresh however `src/` changes —
the unit tests still rebuild, the binary and the process tests in `tests/` do
not. `cargo clean -p opi` puts it right.

## Releasing

The release workflow publishes. It verifies the tag against the crate version,
builds the binaries, publishes through crates.io trusted publishing — a
short-lived token from GitHub OIDC, none stored — and then creates the GitHub
release. The publish job runs in the `crates-io` environment and waits for a
required reviewer, and it comes after the binaries because a publish cannot be
undone.

The binaries are `aarch64`/`x86_64` macOS and `aarch64`/`x86_64` Linux, the
Linux pair built against musl so one binary is not tied to the glibc of the
runner that produced it. Each is a `.tar.gz` with a `.sha256` beside it, named
`opi-<version>-<target>`, which is exactly what `install.sh` reconstructs and
verifies — the archive names are part of that contract, not a detail of the
workflow.

## Modules

One module per layer, named for what it produces:

| | |
| --- | --- |
| `cli` | arguments → an `Invocation` |
| `manifest` | `package.json` → a `Manifest` |
| `cargo` | `Cargo.toml` → a Rust project and its commands |
| `workspace` | workspace patterns → member manifests |
| `project` | manifest and directory → name and package manager |
| `task` | manifests → the one list everything operates on |
| `run` | a task → the running process |
| `check` | a project's tools → concurrent results |
| `audit` | a package manager's audit → parsed advisories |
| `supply` | signatures, release age, licenses → asked of the package manager |
| `outdated` | a package manager's `outdated` → updates by semver jump |
| `clean` | removable paths, measured |
| `workflow` | named sequences of checks, plus repository gates |

Each carries a module-level doc comment saying what it owns and, where it is
not obvious, what it deliberately does not.

`main.rs` holds the wiring and **all** user-facing output. No other module
prints.

## Output

Every user-facing string is produced by a runemark type — `Console`, `Menu`,
`ErrorBlock` — before it is written. No raw ANSI, no hand-rolled colour.

Errors go to stderr as an `ErrorBlock` with an explanation and a remedy, and
exit non-zero. The remedy names something the user can actually do.

## Errors

Each failure mode that needs a different remedy is its own enum variant, with
`Display` and `std::error::Error` implemented by hand. `ManifestError`
distinguishes missing, unreadable and malformed rather than collapsing them,
because those are three different problems for the person running the tool.

Detection never guesses silently. Where evidence is absent, that is a
distinguishable state — `Source::Fallback` — not an unmarked default.

## Tests

Colocated in a `#[cfg(test)] mod tests` at the foot of the module they cover.
Names are sentences about behaviour, not about methods:
`a_standalone_script_joins_its_own_family`, not `test_group_of`.

A test that pins a bug carries a comment saying what the bug was. Several here
were found by driving a pseudo terminal or by running against real
repositories, and that is worth recording where the test lives.

Prefer a test that asserts an invariant over one that asserts a snapshot — for
example, that a viewport never draws more lines than it was given, at every
cursor position and several heights.

The process boundary has its own suite in `tests/process.rs`, run against the
built binary: the argv a package manager receives, the directory it starts
in, the exit code that comes back, and what `opi` makes of a missing
executable or an error written only to stderr. The package managers there are
`sh` stand-ins on a `PATH` holding nothing else, recording how they were
called and answering with output trimmed from real runs — the boundary is
real, the registry is not, so the suite needs no network and no installed
package manager. A behaviour that depends on a terminal (the menus, bun's
interactive update) is covered from the pipe side only: that nothing runs
without one.

## Comments

Explain why, not what. A comment that restates the line below it is noise; a
comment saying which package manager needs `--` and which would pass it
through to the script is not.

## Dependencies

Four, each with a stated reason: `runemark` for all presentation, `serde` and
`serde_json` for `package.json` and the JSON the package managers emit, `glob`
for workspace patterns. A fifth needs an argument that a small adapter over an
existing focused crate cannot be avoided.

`Cargo.toml` is read without a TOML parser for that reason: two facts are
wanted, and a dependency to learn them would cost more than they are worth.

## Measure before deciding

Several rules here came from counting rather than reasoning, and three times the count
contradicted the plan:

- The plan named Knip as a dead-code adapter. Knip was present in 1 of 133 real
  projects; `fallow`, absent from the plan, in 11%.
- The plan called for a `taze` adapter for dependency updates. `taze` was
  present in **none** of the 133, while npm and pnpm both ship `outdated` and
  emit the same JSON. bun and yarn do not, and are told so rather than parsed.
- The `member/script` syntax, because every workspace repository measured had
  root and member scripts sharing names, and none of 382 script names contained
  a slash.
- Dropping the Node version line, because it was most of the startup time.
- The Deploy and Maintenance groups, because 273 of 1923 entries were landing in
  the catch-all and they were a handful of names repeated everywhere.
- Building Rust support and not .NET, because `Cargo.toml` appeared in 51 of
  231 directories and a standalone .NET marker in none.

Where a question is answerable by looking at real projects, look — and where the
answer contradicts the plan, the plan was a guess and the count is not.

## Commands come from real projects

A tool's invocation is taken from what projects already write in their scripts,
not from its documentation. That is how `vitest run` got chosen over a bare
`vitest`, which would have sat in watch mode and never returned.
