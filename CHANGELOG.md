# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- An optional fifth Cargo check, `docs.rs`: `cargo docs-rs`, where the
  subcommand is installed.

  The `Docs` check added in 0.9.0 asks whether a crate documents cleanly. This
  asks a different question: whether the *registry* will render what the author
  meant. `cargo docs-rs` builds with the features, targets and rustdoc
  arguments out of `[package.metadata.docs.rs]` rather than the ones a local
  `cargo doc` picks, and that metadata is what decides how the published pages
  look. Nothing else in the check list reads it.

  Optional like `cargo audit` and `cargo outdated` — a separate binary, so an
  absent one is a check that does not exist rather than one that failed.

## [0.9.0] - 2026-09-23

### Added

- `opi --updates` offers **Decide per package**: every finding as a ticked
  line, majors among them.

  The two existing lines answer the question in bulk — stay inside the declared
  ranges, or raise them — and both skip the majors on purpose. That leaves the
  case they were built to avoid without an answer: one major you have read the
  release notes for, sitting beside twelve minors you have not thought about at
  all. As a line to retype it is a name you have to copy out of a report; as a
  list it is one keystroke per package.

  The safe ones start ticked and the majors do not, so confirming without
  touching anything does what "Raise the ranges" does, and a major is only ever
  taken deliberately. Leaving the list without confirming applies nothing, and
  an empty selection is a no-op rather than a call with no names.

  Like the bulk lines, it is offered for pnpm only — `npm install x@1.1.1`
  turns an exact `2.1.2` into `^2.1.3`, and there is no npm command that keeps
  the operator. `opi` still writes nothing itself: it calls the package manager
  with a list of names, and the commit checks run on what comes back.

- A fourth Cargo check, `Docs`: `cargo rustdoc --all-features -- -D warnings`.

  rustdoc renders a doc comment as HTML, so a bare `<iframe>` in a comment
  becomes a real element and the page can end right there. docs.rs served a
  truncated page for one of these crates for months before a docs.rs maintainer
  reported it. `rustdoc::invalid_html_tags` had been warning the whole time; it
  was simply not wired to anything that could fail.

  `rustdoc` rather than `doc` because the lint level has to reach the crate
  being documented and nothing else. `cargo doc` would take it through
  `RUSTDOCFLAGS`, an environment variable `CHECKS` cannot carry — it is
  `(name, args)` — while `cargo rustdoc` passes it as an argument, the shape
  every other check here already has. It documents the library target where
  there is one, the same target a registry publishes, and the binary otherwise.

  `-D warnings` rather than the one HTML lint: broken intra-doc links and bare
  URLs are the same class of defect, documentation that is wrong where nobody
  looks.

## [0.8.0] - 2026-09-20

### Added

- `opi --updates` offers to take what it found, and runs the commit checks on
  what comes back.

  The offer is **not** safe against breaking — that is the list's ordering and
  it is the wrong question here. Measured: three dependencies, two of them
  "safe", and `pnpm update` moved none, because an exact pin and a `~` range
  each already had what they asked for. As a line to retype nobody notices; as
  a key it would be an action that does nothing and reports success. The choice
  is staying inside the declared ranges against raising them, each line
  carrying its count and omitted where that count is zero.

  `opi` writes nothing itself — it calls the package manager with a list of
  names. Majors are never in that list. Raising ranges is offered for pnpm
  only, because npm has no command that keeps the operator: `npm install
  x@1.1.1` turns an exact `2.1.2` into `^2.1.3`.

  Where two lockfiles disagree and no `packageManager` field settles it, the
  offer is withheld and the finding named. Writing with the wrong manager
  leaves a lockfile the team did not ask for. Without a terminal nothing is
  applied at all, as with `--clean`.

  This reverses "updates are never applied", whose reason was that pnpm
  catalogs put versions outside `package.json`. Measured, pnpm rewrites the
  catalog correctly, and 45 of 79 workspaces here use one — the exception was
  the rule.

- `opi --updates` answers for Rust as well, through `cargo outdated`. Another
  external subcommand, found on `PATH` the way `cargo audit` is, and named
  along with its `cargo install` where it is missing.

  Two sections rather than one merged list, as `--security` already does. Safe
  against breaking is this area's ordering principle and it holds inside an
  ecosystem; across two it would file `tokio` beside `vite` under "Safe to
  take" with only the name saying which is which. Rust's section carries no
  next step, because `cargo update` writes the lockfile — the line `--updates`
  does not cross.

  `Jump` is unchanged and never learns where a version came from. The rule that
  looked like it would need changing — cargo treating `0.12 → 0.13` as breaking
  — was already there, and already applied to npm too.

- `opi --security` reads bun's audit instead of saying it cannot. Bun has had
  an `audit` of its own for a while, and it emits clean JSON on stdout with its
  banner on stderr — measured against 1.3.3.

  Its shape is a map from package name to a **list** of advisories, which npm's
  object carrying one each cannot express, so it is a second shape behind the
  same call, chosen once by which manager was asked. yarn still says it cannot
  be read.

  Bun names `vulnerable_versions` and never the patched range, so its findings
  carry no remedy — deriving "update to 4.17.21" from `<4.17.21` would be
  inventing the one number that has to be right. Several advisories for one
  package condense to one line per severity, so the counts are of vulnerable
  dependencies rather than of advisories; `bun audit` itself has the full list.

- `install.sh`, and release binaries for it to install:

  ```bash
  curl -fsSL https://raw.githubusercontent.com/casoon/opi/main/install.sh | sh
  ```

  `cargo install opi` builds the binary on the user's machine and needs a Rust
  toolchain for a tool whose point is that you can use it in any repository.
  The release workflow now builds macOS and Linux binaries for `aarch64` and
  `x86_64` and attaches them with their checksums; the installer picks the one
  for the platform, verifies it, and writes it to `~/.local/bin`. The Linux
  builds are static musl, so the distribution does not enter into it.

  Publishing stays local. The binaries come out of CI, the crate does not.

### Fixed

- `opi --updates` sees a pnpm workspace's packages. It asked the root
  `package.json` alone, which on one real repository found 2 of 11 outdated
  packages — and where a root declares no dependencies of its own it answers
  `{}`, so `--updates` reported **"everything is current" over stale
  packages**. A false acquittal is worse than no answer. 79 of 133 npm projects
  measured here are pnpm workspaces.

  npm needed no change: it walks the installed tree rather than the manifests
  and already saw every member, measured both ways on a two-member workspace.

  One limit stays, and it is pnpm's: its JSON keys by package name, so a
  package at two versions in two members shows one of them. `pnpm outdated -r`
  prints both rows itself.

- `--updates` survives a warning printed ahead of the JSON. pnpm writes its
  own warnings to **stdout**, so a slow registry put `‼ WARN‼ Request took
  11084ms: …` in front of the document and the area answered "could not read
  pnpm's output". It only showed up when the network was slow enough, which is
  why the first round of testing missed it.

- `--updates` in a project without a `package.json` no longer says `opi checks
  updates only for npm`, which stopped being true.

- `opi --security` answers the dependency question for Rust as well, through
  `cargo audit`.

  It is an external subcommand rather than part of the toolchain, so `opi` looks
  for a `cargo-audit` executable on `PATH` the way cargo does, and where there
  is none it says so with the `cargo install` that adds it. A repository with
  both manifests gets two sections, `Dependencies` and `Dependencies (rust)`,
  the way health already names its checks.

  No severity is shown, because cargo-audit's JSON does not carry one — the key
  is absent, and only a CVSS vector string is there. Its console output does
  print `Severity: 7.5 (high)`, since it scores the vector itself, so that is
  where the report's next step sends you. Scoring the vector inside `opi` would
  mean reimplementing CVSS, and reading the number out of console text would
  mean a parser on an undocumented format.

- `opi --security` and `opi --updates` no longer run npm in a project that has
  no `package.json`.

  `Project::package_manager` always holds a value, because the absence of every
  signal still produces the npm fallback — so both areas asked it regardless,
  and a Rust-only project got `npm audit --json` run in a directory with no
  manifest, then npm's complaint about that relayed as the result. They now say
  that there is nothing here they can audit or check, which is the one thing an
  empty section cannot say.

- In a Cargo workspace, `opi` started inside a crate now works on the workspace
  rather than on that crate.

  The nearest `Cargo.toml` was taken as the project, so `opi --clean` looked for
  `target/` inside the crate, where it is not — missing the largest directory in
  the repository, 509 MB in the one this was found on, which is the only reason
  anyone starts Clean. The checks covered one crate instead of all of them, and
  the header named the crate rather than the repository.

  Whether `cargo run` is offered is still asked of the directory you are
  standing in, since that is where cargo starts it: a binary crate inside a
  workspace keeps its entry, a virtual workspace root has nothing to run.

- A Bun project is recognised by `bun.lock`, not only by the binary `bun.lockb`
  it stopped writing in 1.2.

  Only the old name was in the lockfile table, so a Bun project without a
  `packageManager` field fell through to the npm fallback — and `opi build` ran
  `npm run build` in a repository that has no npm in it.

- `opi --updates` says that it cannot read bun's or yarn's output, instead of
  failing on it.

  Both were asked for `outdated --json` and their answer parsed as npm's shape.
  bun ignores `--json` and prints a table, yarn 1 answers line by line and
  yarn 2 has no such command at all — so the screen ended in a parse error that
  read like a broken project rather than a missing feature. `--security` had
  this guard from the start; `--updates` now has the same one.

## [0.7.0] - 2026-09-20

### Changed

- Workspace packages share one `Packages` tab instead of taking one each, with
  a divider in front of it. Inside, each package keeps its name as a heading.

  One tab each was measured and does not scale. The largest workspace here has
  52 packages against 6 action groups: a tab row of 58 that is almost entirely
  package names, permanently scrolling, with the digits worthless past the
  ninth. It is seven tabs now, and `web-casoon` is eight instead of ten.

  It also fixes a mix-up the tabs introduced. The action tabs answer "what am
  I doing", the package tabs "where am I doing it", and they stood side by side
  as peers — so "where is the thing that deploys insights?" had two right
  answers, `Deploy` and `@casoon/insights`. Every tab is an action now, except
  one that the divider marks as something else.


## [0.6.1] - 2026-09-20

### Fixed

- `0.6.0` did not compile on Rust `1.85`, the version this crate declares. The
  cause was in runemark `0.7.0`, which used a `let` chain — stable only from
  `1.88` — for the tab digits. Requires runemark `0.7.1`.

## [0.6.0] - 2026-09-20

### Added

- Past 15 entries or 5 groups the start screen shows the groups as a row of
  tabs and lists only the active one. `web-casoon` has 27 scripts in its root
  in 7 groups, which is 34 lines with a heading each — taller than a
  full-screen terminal, so the top scrolled away. `←` `→`, `Tab` and the
  digits `1`–`9` switch groups.

  The 0.1.0 rule holds: the first group is preselected with the cursor on its
  first entry, so `opi ↵` still runs `dev`. A submenu demands a choice before
  anything runnable is on screen; the tabs demand nothing and hide what is not
  needed. `/` still searches every group — a query matching four of them shows
  all four together, which is exactly what tabs are worst at.

  A threshold rather than the terminal's own height, which would have fitted
  more exactly: the height changes while the menu is open, and a list that
  rearranged itself mid-keystroke would move entries under a cursor already on
  its way to one. `opi | cat` lists every group either way.
- A line under the heading says what was found: `43 entries · 10 groups ·
  3 packages`. Two of those three are no longer on screen at once once the
  groups are tabs, and packages are named only where there are any.

### Notes

Both need runemark `0.7` (`Layout::Tabs`, `Menu::with_summary`), where the
interactive path lives.

## [0.5.3] - 2026-09-19

### Fixed

- The audit's severity counts are legible without colour. They carried a tone,
  which is nothing in a pipe or in CI, so `high: 1` read exactly like a clean
  count; they now render as `[FAIL] high: 1` (runemark 0.6).

## [0.5.2] - 2026-09-19

### Changed

- The dependency audit and the update list render as runemark `Report`s. The
  split between safe and breaking updates is now two groups rather than a
  sentence under a flat list, severity counts are metrics, and an advisory's
  fixed version range is a remedy.

### Notes

Health, the workflows and the secret scan keep printing their tool's own
output, which was settled by trying the alternative: a relayed blob in one
`Finding` collapses the tool's structure, and one `Finding` per line flattens
its indentation and counts lines as if they were findings.

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

[Unreleased]: https://github.com/casoon/opi/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/casoon/opi/releases/tag/v0.9.0
[0.8.0]: https://github.com/casoon/opi/releases/tag/v0.8.0
[0.7.0]: https://github.com/casoon/opi/releases/tag/v0.7.0
[0.6.1]: https://github.com/casoon/opi/releases/tag/v0.6.1
[0.6.0]: https://github.com/casoon/opi/releases/tag/v0.6.0
[0.5.3]: https://github.com/casoon/opi/releases/tag/v0.5.3
[0.5.2]: https://github.com/casoon/opi/releases/tag/v0.5.2
[0.5.1]: https://github.com/casoon/opi/releases/tag/v0.5.1
[0.5.0]: https://github.com/casoon/opi/releases/tag/v0.5.0
[0.4.0]: https://github.com/casoon/opi/releases/tag/v0.4.0
[0.3.0]: https://github.com/casoon/opi/releases/tag/v0.3.0
[0.2.0]: https://github.com/casoon/opi/releases/tag/v0.2.0
[0.1.1]: https://github.com/casoon/opi/releases/tag/v0.1.1
[0.1.0]: https://github.com/casoon/opi/releases/tag/v0.1.0
[0.0.1]: https://github.com/casoon/opi/releases/tag/v0.0.1
