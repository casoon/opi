# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/casoon/opi/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/casoon/opi/releases/tag/v0.1.1
[0.1.0]: https://github.com/casoon/opi/releases/tag/v0.1.0
[0.0.1]: https://github.com/casoon/opi/releases/tag/v0.0.1
