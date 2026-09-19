# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.1] - 2026-09-19

### Fixed

- `opi --help` said entries come from `package.json`, which stopped being the
  whole truth when `Cargo.toml` was added in `0.5.0`. It now names both, and
  mentions the `<member>/<script>` form, which was undocumented.

### Notes

A full pass over the README and `docs/` against the code turned up eleven
statements the implementation no longer supported — among them a `taze`
adapter that was never built, a diff preview before writing that does not
exist, an `opi.health` key that is not read, and a claim that raw mode is
restored on a signal, three lines below the sentence explaining that it is not.

## [0.5.0] - 2026-09-19

### Added

- Rust projects. `Cargo.toml` is discovered the same way `package.json` is, and
  its commands join the same list, groups and search. Health gains
  `cargo fmt --check`, `cargo clippy` and `cargo test`; clean gains `target/`.
- A repository can be both at once, and neither wins. Measured across 231
  directories: 133 carry a `package.json`, 51 a `Cargo.toml`, and twelve both.
- A repository with only a `Cargo.toml` is now a project. 39 of them were
  previously turned away with "No package.json found".

### Notes

.NET is deliberately not built: it appeared in none of those directories on its
own, and building for a marker nobody has is what retired Knip and taze earlier.

A Rust project declares no scripts, so its commands are a fixed set — the ones
these repositories actually run in CI. `cargo run` appears only where something
is runnable, since offering it for a library would fail on use.

`Cargo.toml` is read without a TOML parser. Two facts are wanted and a
dependency to learn them would cost more than they are worth.

## [0.4.0] - 2026-09-19

### Added

- Per-script metadata in the `opi` key of `package.json`: `description`,
  `group`, `favorite` and `confirm`. None of it is required — a project with
  neither `opi` nor `scripts-info` still gets a usable list, which is the point.
- `description` there wins over `scripts-info` without invalidating it. Both may
  name the same script; that is how a project says something to `opi` without
  changing what `nr` and `npm-scripts-info` read.
- `group` replaces the group a script's name implies. A name `opi` already
  knows takes that group's fixed place in the order, so configuration refines
  the meaning-first arrangement rather than dropping out of it.
- `favorite` lifts a script out of its group into a Favorites group at the top.
  Sorting it first *within* its group would barely show — measured on a real
  project, five of ten groups held two entries.
- `confirm` asks before running. Without a terminal it refuses rather than
  assuming yes, since skipping the question where it cannot be asked would
  remove the protection in exactly the case it exists for. `--yes` says it out
  loud instead.

### Notes

The `opi` block is held as unparsed JSON and read field by field: a `favorite`
that is a string rather than a boolean costs that one field, not the entry, and
certainly not a `package.json` whose `scripts` are perfectly fine.

A favourite in a workspace member stays with its member. Lifting it into the
root's Favorites would lose which package it belongs to.

## [0.3.0] - 2026-09-19

Five areas beside the script list, each a hotkey in the list and a flag on the
command line — never a bare word, since `health`, `clean` and `release` are
script names in real projects.

### Added

- **Health** (`H`, `--health`) runs every check the project's tools can answer,
  concurrently. `opi` reimplements none of them: it detects the tool, runs it,
  and relays the result. Checks run per workspace member, in the member —
  a monorepo keeps TypeScript and its test runner in the packages, and pnpm does
  not hoist their binaries.
- **Security** (`S`, `--security`) scans for secrets and parses the package
  manager's audit into package, severity and the version that fixes it.
- **Updates** (`U`, `--updates`) lists what is outdated, split into safe and
  major. A pre-1.0 minor counts as breaking, as does a version that does not
  parse.
- **Clean** (`C`, `--clean`) shows removable artefacts with what each costs, and
  removes build output without ever bundling `node_modules` along with it.
- **Workflows** (`--check commit`, `--check release`) run named subsets plus
  repository gates: a clean tree and an untagged version before a release.

### Changed

- `deploy`, `release`, `clean`, `setup` and similar standalone names now have
  Deploy and Maintenance groups instead of the catch-all, which halves it.
- npm's own lifecycle scripts are hidden from the list — `prepare` appeared in
  49 of 133 measured projects — but remain runnable by name.

### Notes

Two adapters in the plan were replaced by what the measurements found: Knip was
present in 1 of 133 projects against fallow's 11%, and `taze` in none at all,
while every package manager ships `outdated`.

Applying updates is deliberately left to the package manager. Rewriting
`package.json` and a lockfile — which with pnpm catalogs may not even hold the
versions — is not a guess worth making.

## [0.2.0] - 2026-09-19

### Added

- `/` filters the list as you type. Groups with nothing left disappear and the
  cursor sits on the best match. In one real project this narrows 29 entries to
  3 for `dep`; `og` finds a workspace member's `generate:og` that would
  otherwise be twenty rows down.
- The footer shows `/ search`, so the key is discoverable rather than folklore.

Matching ranks a name the user is typing towards above a description that
happens to share letters, and a tight run of characters above the same letters
scattered through a longer name (runemark 0.5.1).

## [0.1.1] - 2026-09-19

Both changes come from running `0.1.0` against a real project, where a list of
41 entries with lines up to 101 columns wide was unreadable.

### Fixed

- Entries are fitted to the terminal width instead of wrapping to column zero,
  which destroyed the two-column layout (runemark 0.4.2). Long descriptions are
  shortened with an ellipsis; where the name column leaves no useful room, the
  description is dropped rather than cut to a stub.

### Changed

- A workspace member's script is no longer listed when the root defines one by
  the same name. The root wins that name on the command line anyway, so listing
  both offered a choice the interface could not honour. On one real project
  this removed 12 of 18 member entries, none of which carried a description.
  What remains is what the root cannot reach; a member left with nothing shows
  no group at all.

## [0.1.0] - 2026-09-19

The first release that does something. `opi` makes a project's `package.json`
scripts immediately runnable; the maintenance areas are designed but not built.

### Added

- An interactive list of the project's scripts, grouped by the prefix before
  the first `:` and labelled from `scripts-info`. Group order is meaning-first,
  not alphabetical. A list taller than the terminal scrolls.
- `opi <script>` runs one directly, forwarding any further arguments. Scripts
  take precedence over built-in names, so a project with a script called
  `help` keeps working.
- Package manager detection from the `packageManager` field or a lockfile,
  searching upwards. Where neither exists the npm default is marked as a
  fallback rather than presented as a detection.
- Monorepo support: the nearest `package.json` is found by searching upwards,
  and a workspace root also offers each member's scripts under the member's
  name. `opi blog/dev` addresses a member, since root and member scripts share
  names in practice.
- Suggestions for a mistyped script name, using optimal string alignment so a
  transposition counts as one edit.

### Notes

- Unix only. `opi` runs a script by replacing its own process with `exec`, so
  `Ctrl-C` reaches your dev server and the exit code is the script's. Building
  on Windows fails with that reason rather than producing a degraded binary.
- Lifecycle hooks npm runs on its own are hidden when their main script exists.

## [0.0.1]

Placeholder release. Reserves the crate name on crates.io and establishes the
repository skeleton. The binary prints its version and a development notice; no
functionality is implemented.

[Unreleased]: https://github.com/casoon/opi/compare/v0.5.1...HEAD
[0.5.1]: https://github.com/casoon/opi/releases/tag/v0.5.1
[0.5.0]: https://github.com/casoon/opi/releases/tag/v0.5.0
[0.4.0]: https://github.com/casoon/opi/releases/tag/v0.4.0
[0.3.0]: https://github.com/casoon/opi/releases/tag/v0.3.0
[0.2.0]: https://github.com/casoon/opi/releases/tag/v0.2.0
[0.1.1]: https://github.com/casoon/opi/releases/tag/v0.1.1
[0.1.0]: https://github.com/casoon/opi/releases/tag/v0.1.0
[0.0.1]: https://github.com/casoon/opi/releases/tag/v0.0.1
