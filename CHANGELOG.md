# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html); below `1.0.0` a
minor release may change existing behaviour.

## [Unreleased]

### Added

- **Security:** the license line of `opi --security` also covers npm and bun projects
  (from `npm sbom`) and Cargo projects (from `cargo metadata`), production dependencies
  only, with the same notable licenses as for pnpm.
- **Rust:** `cargo rustdoc` runs a second time with `--all-features` where the
  manifest has a feature to turn on. A link from an always compiled item to one
  behind a feature resolves in the one feature set and not in the other — on
  `runemark`, `--all-features` passed while the default set reported three
  errors.
- **Rust:** a package below the project root that belongs to no workspace, tied
  to the root by a path dependency, gets a test run of its own under its package
  name. Every workspace flag missed `runemark`'s Node binding, so its tests ran
  in CI and nowhere else. A manifest wired to nothing stays out, like `opi`'s
  own `demo/fixtures/pulse`.
- **Docs:** the pages describing the codebase — project state, architecture,
  constraints, conventions, decisions — are on the project page, under **The
  project**, instead of being linked as raw markdown on GitHub. They already
  carried page frontmatter; the content glob never matched a file sitting
  directly in `docs/`. Details:
  [Project state](https://casoon.github.io/opi/docs/project-state/).

### Changed

- **Docs:** the flow in Architecture is a chartlet diagram rather than a mermaid
  block, which the theme renders as highlighted source. It comes with a focus
  control, a data table and a light and a dark variant. Details:
  [Architecture](https://casoon.github.io/opi/docs/architecture/#flow).
- **Docs:** the recordings on the project page's start page are readable at
  their size. The hero recording is made at about 55 columns for its column and
  at 43 for phones instead of shrinking the README's 95-column GIF, and the
  scenes under **One key away** have 50-column variants for phones.
- **Contributors:** the site builds with chartlet `0.21.0` instead of `0.19.0`,
  whose flow charts took neither `sameRank` nor `notes` — the two manifest
  discoveries stand side by side now, and the note on parsing sits beside them.
- **Contributors:** `demo/record.sh` records every scene in a fresh copy of the
  fixtures, so no scene sees the Cargo.lock or pre-push hook an earlier one
  left behind; it also finds the binary under a shared cargo target directory
  and pins castwright `0.5.0`. All recordings are made again with opi `0.11.0`.

### Fixed

- **Docs:** the README describes the health recording as five checks, the
  number it shows, instead of six.
- **Docs:** the changelog shows its whole content on the project page again.
  The theme's parser ends an entry at the first blank line, so every entry
  written as several paragraphs lost all but the first — the measurements, the
  reasoning and the limits, which is most of what the entries said. Each entry
  is one paragraph now, and nothing is left outside a list item.

## [0.11.0] - 2026-09-26

### Added

- **Workflows:** a **push** workflow, `opi --check push`: every detected check,
  then the build, on a working tree that is clean and not behind its upstream
  branch. The build is the project's own `build` script (each member's in a
  workspace whose root has none) and `cargo build --workspace --locked`, run
  after the other checks and one at a time. `release` builds too; `commit`
  stays fast.
- **Workflows:** `opi --hooks` installs `opi --check push` as the pre-push hook
  — in `.husky/pre-push` where the repository has husky, otherwise wherever git
  keeps its hooks. Idempotent; an existing hook keeps its lines.
- **List:** task runner files beside the manifest are named in the line under
  the heading (`also Makefile`) — `Makefile`, `justfile`, `Taskfile.yml`, a
  `mise.toml` with tasks. Named only, never read or run.
- **Security:** three supply-chain lines for npm projects: registry signatures
  (`npm audit signatures`, fails the run on a mismatch), the minimum release
  age (`minimumReleaseAge` / `min-release-age`, asked with `config get`), and a
  license overview for pnpm (`pnpm licenses list --prod`) naming copyleft and
  unlicensed packages. The last two only report.

## [0.10.0] - 2026-09-25

### Added

- **Health:** a **Lockfile** check, in `opi --health` and both workflows: whether
  the lockfile still matches the manifests, asked of the package manager that
  wrote it with a command measured to change nothing. yarn gets none. Details:
  [What is detected](https://casoon.github.io/opi/docs/guides/health/#what-is-detected).
- **Updates:** bun projects are offered **Choose in bun's own list**, which hands
  over to `bun update --interactive` and runs the commit checks afterwards.
  `bun outdated` prints a table and ignores `--json`, so there was nothing to
  offer before. Details:
  [Taking the updates](https://casoon.github.io/opi/docs/guides/updates/#taking-the-updates).
- **Security:** yarn's audit output is read — a third shape beside npm/pnpm's and
  bun's, and the one manager that prints nothing at all when it finds nothing.
  Like bun it names only the vulnerable range, so its findings carry no remedy.
  Details:
  [Three shapes behind one question](https://casoon.github.io/opi/docs/guides/security/#three-shapes-behind-one-question).
- **Updates:** yarn is still not read here. Yarn Berry dropped the `outdated`
  command entirely, so there is nothing to parse.
- **Rust:** an optional fifth Cargo check, `docs.rs`: `cargo docs-rs`, where the
  subcommand is installed. `Docs` asks whether a crate documents cleanly; this
  asks whether the registry will render what the author meant. Details:
  [What cargo docs-rs adds](https://casoon.github.io/opi/docs/guides/rust/#what-cargo-docs-rs-adds).

### Changed

- **Docs:** the manifest's `documentation` field points at
  [casoon.github.io/opi](https://casoon.github.io/opi/) instead of docs.rs,
  whose build fails for every binary-only crate. The link on crates.io led to an
  error page; it now leads to the documentation. Details:
  [Crate](https://casoon.github.io/opi/docs/reference/cli/#crate).

### Fixed

- **Security:** a failed audit is no longer reported as clean. An audit stopped by
  a network error or a broken lockfile prints no report; `opi` named the reason
  but still ended with "Nothing to act on." and exited 0. Details:
  [Exit codes](https://casoon.github.io/opi/docs/reference/cli/#exit-codes).
- **Contributors:** process tests in `tests/process.rs` run the built binary
  against stand-in package managers and check the argv, working directory and
  exit code at the boundary rather than the logic behind it. They found the
  audit exit code above.

## [0.9.0] - 2026-09-23

### Added

- **Updates:** **Decide per package** offers every finding as a ticked line,
  majors among them — the case the two bulk lines cannot reach, since both skip
  majors on purpose. The safe ones start ticked and the majors do not. pnpm
  only, like raising ranges. Details:
  [Taking the updates](https://casoon.github.io/opi/docs/guides/updates/#taking-the-updates).
- **Rust:** a fourth Cargo check, `Docs`: `cargo rustdoc -- -D warnings`. rustdoc
  renders a doc comment as HTML, so a bare `<iframe>` in one ends the page.
  docs.rs served a truncated crate for months before anyone noticed. Details:
  [Why rustdoc is a health check](https://casoon.github.io/opi/docs/guides/rust/#why-rustdoc-is-a-health-check).

## [0.8.0] - 2026-09-20

### Added

- **Updates:** `opi --updates` offers to take what it found, and runs the commit
  checks on what comes back. The choice is staying inside the declared ranges
  against raising them, never majors, pnpm only. This reverses "updates are
  never applied", whose reason did not survive being measured. Details:
  [Taking the updates](https://casoon.github.io/opi/docs/guides/updates/#taking-the-updates).
- **Updates:** Rust is answered as well, through `cargo outdated`. Two sections
  rather than one merged list, because "safe to take" orders within an ecosystem
  and not across two. Rust's section carries no next step, since `cargo update`
  writes the lockfile. Details: [Updates](https://casoon.github.io/opi/docs/guides/updates/).
- **Security:** bun's audit is read instead of saying it cannot be — a second
  shape behind the same call, and one that names only the vulnerable range, so
  its findings carry no remedy. Details:
  [Three shapes behind one question](https://casoon.github.io/opi/docs/guides/security/#three-shapes-behind-one-question).
- **Install:** `install.sh`, and release binaries for it to install — macOS and
  Linux, `aarch64` and `x86_64`, picked by platform and verified against their
  published checksum. Details:
  [Install script](https://casoon.github.io/opi/docs/getting-started/installation/#install-script).

### Fixed

- **Updates:** a pnpm workspace's packages are seen. Asked at the root alone,
  pnpm answers `{}` where that root declares no dependencies of its own, so
  `--updates` reported **"everything is current" over stale packages** — a false
  acquittal, which is worse than no answer. Details:
  [Limits](https://casoon.github.io/opi/docs/guides/updates/#limits).
- **Updates:** a warning printed ahead of the JSON no longer breaks the read.
  pnpm writes its own warnings to **stdout**, so a slow registry put `‼ WARN‼
  Request took 11084ms: …` in front of the document and the area answered
  "could not read pnpm's output". It only showed up when the network was slow
  enough, which is why the first round of testing missed it.
- **Updates:** a project without a `package.json` no longer gets `opi checks
  updates only for npm`, which stopped being true.
- **Security:** the dependency question is answered for Rust as well, through
  `cargo audit`, in its own `Dependencies (rust)` section. No severity is shown,
  because cargo-audit's JSON carries none. Details:
  [Rust](https://casoon.github.io/opi/docs/guides/security/#rust).
- **Security:** npm is no longer run in a project that has no `package.json`, in
  `--security` or in `--updates`. The npm fallback holds a value even where
  nothing signalled npm, so a Rust-only project got `npm audit --json` run in a
  directory with no manifest. Both areas now say there is nothing here they can
  audit, which is the one thing an empty section cannot say.
- **Project:** in a Cargo workspace, `opi` started inside a crate works on the
  workspace rather than on that crate. `opi --clean` had been looking for
  `target/` inside the crate, where it is not. Details:
  [Workspaces](https://casoon.github.io/opi/docs/guides/rust/#workspaces).
- **Project:** a Bun project is recognised by `bun.lock`, not only by the
  binary `bun.lockb` it stopped writing in 1.2. Only the old name was in the
  lockfile table, so a Bun project without a `packageManager` field fell
  through to the npm fallback — and `opi build` ran `npm run build` in a
  repository that has no npm in it.
- **Updates:** bun's and yarn's output is reported as unreadable instead of
  failing on it. Both were parsed as npm's shape, so the screen ended in a parse
  error that read like a broken project rather than a missing feature.
  `--security` had this guard from the start.

## [0.7.0] - 2026-09-20

### Changed

- **List:** workspace packages share one `Packages` tab instead of taking one
  each, with a divider in front of it. One tab each does not scale, and it put
  two different questions side by side as peers. Every tab is an action now,
  except the one the divider marks as something else. Details:
  [Tabs for long lists](https://casoon.github.io/opi/docs/guides/scripts/#tabs-for-long-lists).

## [0.6.1] - 2026-09-20

### Fixed

- **Crate:** `0.6.0` did not compile on Rust `1.85`, the version this crate
  declares. The cause was in runemark `0.7.0`, which used a `let` chain —
  stable only from `1.88` — for the tab digits. Requires runemark `0.7.1`.

## [0.6.0] - 2026-09-20

### Added

- **List:** past 15 entries or 5 groups the start screen shows the groups as a
  row of tabs and lists only the active one. `←` `→`, `Tab` and the digits
  `1`–`9` switch groups, `/` still searches every group, and `opi ↵` still runs
  `dev`. Details:
  [Tabs for long lists](https://casoon.github.io/opi/docs/guides/scripts/#tabs-for-long-lists).
- **List:** a line under the heading says what was found: `43 entries · 10
  groups · 3 packages`. Two of those three are no longer on screen at once once
  the groups are tabs, and packages are named only where there are any.
- **Crate:** both need runemark `0.7` (`Layout::Tabs`, `Menu::with_summary`),
  where the interactive path lives.

## [0.5.3] - 2026-09-19

### Fixed

- **Security:** the audit's severity counts are legible without colour. They
  carried a tone, which is nothing in a pipe or in CI, so `high: 1` read
  exactly like a clean count; they now render as `[FAIL] high: 1` (runemark
  0.6).

## [0.5.2] - 2026-09-19

### Changed

- **Security:** the dependency audit and the update list render as runemark
  `Report`s. The split between safe and breaking updates is now two groups
  rather than a sentence under a flat list, severity counts are metrics, and an
  advisory's fixed version range is a remedy.
- **Health:** health, the workflows and the secret scan keep printing their
  tool's own output, which was settled by trying the alternative: a relayed
  blob in one `Finding` collapses the tool's structure, and one `Finding` per
  line flattens its indentation and counts lines as if they were findings.

## [0.5.1] - 2026-09-19

### Fixed

- **Docs:** `opi --help` said entries come from `package.json`, which stopped
  being the whole truth when `Cargo.toml` was added in `0.5.0`. It now names
  both, and mentions the `<member>/<script>` form, which was undocumented.
- **Docs:** a full pass over the README and `docs/` against the code turned up
  eleven statements the implementation no longer supported — among them a
  `taze` adapter that was never built, a diff preview before writing that does
  not exist, an `opi.health` key that is not read, and a claim that raw mode is
  restored on a signal, three lines below the sentence explaining that it is
  not.

## [0.5.0] - 2026-09-19

### Added

- **Rust:** Rust projects. `Cargo.toml` is discovered the same way
  `package.json` is, and its commands join the same list, groups and search.
  Health gains `cargo fmt --check`, `cargo clippy` and `cargo test`; clean gains
  `target/`. Details: [Cargo projects](https://casoon.github.io/opi/docs/guides/rust/).
- **Project:** a repository can be both at once, and neither wins. Measured
  across 231 directories: 133 carry a `package.json`, 51 a `Cargo.toml`, and
  twelve both.
- **Project:** a repository with only a `Cargo.toml` is now a project. 39 of
  them were previously turned away with "No package.json found".
- **Project:** .NET is deliberately not built: it appeared in none of those
  directories on its own, and building for a marker nobody has is what retired
  Knip and taze earlier.

## [0.4.0] - 2026-09-19

### Added

- **Scripts:** per-script metadata in the `opi` key of `package.json`:
  `description`, `group`, `favorite` and `confirm`. None of it is required.
  Details: [Per script](https://casoon.github.io/opi/docs/guides/configuration/#per-script).
- **Scripts:** `description` there wins over `scripts-info` without
  invalidating it. Both may name the same script; that is how a project says
  something to `opi` without changing what `nr` and `npm-scripts-info` read.
- **Scripts:** `group` replaces the group a script's name implies. A name `opi`
  already knows takes that group's fixed place in the order, so configuration
  refines the meaning-first arrangement rather than dropping out of it.
- **Scripts:** `favorite` lifts a script out of its group into a Favorites
  group at the top. Sorting it first *within* its group would barely show —
  measured on a real project, five of ten groups held two entries. A favourite
  in a workspace member stays with its member: lifting it into the root's
  Favorites would lose which package it belongs to.
- **Scripts:** `confirm` asks before running. Without a terminal it refuses
  rather than assuming yes, since skipping the question where it cannot be
  asked would remove the protection in exactly the case it exists for. `--yes`
  says it out loud instead.

## [0.3.0] - 2026-09-19

### Added

- **Health:** `H` / `--health` runs every check the project's tools can answer,
  concurrently. `opi` reimplements none of them: it detects the tool, runs it,
  and relays the result. Checks run per workspace member, in the member — a
  monorepo keeps TypeScript and its test runner in the packages, and pnpm does
  not hoist their binaries.
- **Security:** `S` / `--security` scans for secrets and parses the package
  manager's audit into package, severity and the version that fixes it.
- **Updates:** `U` / `--updates` lists what is outdated, split into safe and
  major. A pre-1.0 minor counts as breaking, as does a version that does not
  parse. Applying updates is deliberately left to the package manager:
  rewriting `package.json` and a lockfile — which with pnpm catalogs may not
  even hold the versions — is not a guess worth making.
- **Clean:** `C` / `--clean` shows removable artefacts with what each costs,
  and removes build output without ever bundling `node_modules` along with it.
- **Workflows:** `--check commit` and `--check release` run named subsets plus
  repository gates: a clean tree and an untagged version before a release.
- **List:** the five areas each take a hotkey in the list and a flag on the
  command line — never a bare word, since `health`, `clean` and `release` are
  script names in real projects.

### Changed

- **List:** `deploy`, `release`, `clean`, `setup` and similar standalone names
  now have Deploy and Maintenance groups instead of the catch-all, which halves
  it.
- **List:** npm's own lifecycle scripts are hidden — `prepare` appeared in 49
  of 133 measured projects — but remain runnable by name.
- **Health:** two adapters in the plan were replaced by what the measurements
  found: Knip was present in 1 of 133 projects against fallow's 11%, and `taze`
  in none at all, while every package manager ships `outdated`.

## [0.2.0] - 2026-09-19

### Added

- **List:** `/` filters the list as you type. Groups with nothing left disappear
  and the cursor sits on the best match (runemark 0.5.1). Details:
  [Searching](https://casoon.github.io/opi/docs/guides/scripts/#searching).
- **List:** the footer shows `/ search`, so the key is discoverable rather than
  folklore.

## [0.1.1] - 2026-09-19

### Changed

- **List:** a workspace member's script is no longer listed when the root defines
  one by the same name. The root wins that name on the command line anyway, so
  listing both offered a choice the interface could not honour. Details:
  [Workspaces](https://casoon.github.io/opi/docs/guides/scripts/#workspaces).

### Fixed

- **List:** entries are fitted to the terminal width instead of wrapping to
  column zero, which destroyed the two-column layout (runemark 0.4.2). Found by
  running `0.1.0` against a real project. Details:
  [Tabs for long lists](https://casoon.github.io/opi/docs/guides/scripts/#tabs-for-long-lists).

## [0.1.0] - 2026-09-19

### Added

- **List:** an interactive list of the project's scripts, grouped by the prefix
  before the first `:` and labelled from `scripts-info`. Group order is
  meaning-first, not alphabetical. A list taller than the terminal scrolls.
  This is the first release that does something: `opi` makes a project's
  `package.json` scripts immediately runnable, while the maintenance areas are
  designed but not built.
- **Scripts:** `opi <script>` runs one directly, forwarding any further
  arguments. Scripts take precedence over built-in names, so a project with a
  script called `help` keeps working. Details:
  [Unix only](https://casoon.github.io/opi/docs/getting-started/installation/#unix-only).
- **Project:** package manager detection from the `packageManager` field or a
  lockfile, searching upwards. Where neither exists the npm default is marked
  as a fallback rather than presented as a detection.
- **Project:** monorepo support: the nearest `package.json` is found by
  searching upwards, and a workspace root also offers each member's scripts
  under the member's name. `opi blog/dev` addresses a member, since root and
  member scripts share names in practice.
- **Scripts:** suggestions for a mistyped script name. Details:
  [Parsing rules](https://casoon.github.io/opi/docs/reference/cli/#parsing-rules).
- **Scripts:** lifecycle hooks npm runs on its own are hidden when their main
  script exists.

## [0.0.1] - 2026-09-18

### Added

- **Crate:** a placeholder release, reserving the crate name on crates.io and
  establishing the repository skeleton. The binary prints its version and a
  development notice; no functionality is implemented.

[Unreleased]: https://github.com/casoon/opi/compare/v0.11.0...HEAD
[0.11.0]: https://github.com/casoon/opi/releases/tag/v0.11.0
[0.10.0]: https://github.com/casoon/opi/releases/tag/v0.10.0
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
