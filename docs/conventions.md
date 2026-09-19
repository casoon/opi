# Conventions

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

## Releasing

`cargo publish` is run locally. The release workflow verifies the tag against
the crate version, confirms the version is on crates.io, and creates the GitHub
release — it never publishes. The registry token stays on a maintainer machine
rather than in repository secrets.

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
  present in **none** of the 133, while every package manager ships `outdated`.
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
