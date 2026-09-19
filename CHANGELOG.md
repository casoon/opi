# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/casoon/opi/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/casoon/opi/releases/tag/v0.3.0
[0.2.0]: https://github.com/casoon/opi/releases/tag/v0.2.0
[0.1.1]: https://github.com/casoon/opi/releases/tag/v0.1.1
[0.1.0]: https://github.com/casoon/opi/releases/tag/v0.1.0
[0.0.1]: https://github.com/casoon/opi/releases/tag/v0.0.1
