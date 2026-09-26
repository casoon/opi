---
title: Constraints
description: "Hard boundaries the implementation respects: toolchain and platforms, zero configuration, what may be deleted."
order: 3
---

Hard boundaries the implementation must respect. Where something is an
assumption rather than a settled requirement, it is marked as such.

## Toolchain and platforms

Rust edition 2024, minimum supported Rust version `1.85` — matching runemark,
which `opi` depends on. CI runs the test suite against `1.85` and stable on
Linux and macOS.

**Unix only, deliberately.** Two things `opi` relies on have no Windows
equivalent: running a script by replacing its own process with `exec`, and
driving termios for the interactive list. Supporting Windows would mean a
second execution model — spawn and supervise — with different `Ctrl-C`
semantics and no test coverage. Building on Windows fails with an explicit
message instead.

## Zero configuration

`opi` must be useful in an unmodified repository, with no setup step. This is
not a convenience goal — it is the condition for the tool being used at all. A
tool that must be configured first will never be started in someone else's
project.

Consequence: every screen needs sensible defaults without configuration. If a
screen is useless without the optional `opi` key in `package.json`, the default
is wrong, not the configuration missing.

## The project's own files are the only interface

`opi` reads `package.json` and `Cargo.toml` — the files a project already has.
There is no `opi.toml`, no `.opirc`, no second source of truth of `opi`'s own
making.

Three levels, each optional:

1. `scripts` — sufficient on its own
2. `scripts-info` — descriptions, deliberately the field `nr` already uses, so
   existing projects benefit without change
3. `opi` — refinement `opi` defines itself: `opi.clean` for extra removable
   paths, and `opi.scripts.<name>` for a description, a group, a favourite or a
   confirmation. Never a precondition for a screen to work.

A Rust project declares none of this. Its commands are the same everywhere, so
`Cargo.toml` is read for two facts — the package name and whether a binary
exists — and nothing else.

## No task system of its own

`opi` does not define tasks with their own commands. The moment it does, it
competes with npm scripts, `just`, `make`, `mise` and Taskfile — a contest with
no upside — while also becoming a tool that must be configured first.

It does name them. A `Makefile`, `justfile`, `Taskfile.yml` or a `mise.toml` with tasks beside
the manifest shows up in the line under the heading — `also Makefile` — so the list does not
pass for everything the project can do. Named only: their targets are never read and never run.

Workflows (`commit`, `push`, `release`) are named lists of existing task names, not a new
task type.

## Orchestration, not reimplementation

No check is implemented inside `opi`. TypeScript errors come from `tsc` or
`astro check`, lint findings from Biome or ESLint, dead code from fallow or
Knip, vulnerabilities from the package manager's own audit. `opi` detects which
tool a project uses, invokes it, and relays the result.

Output is parsed only where the format is documented — `audit --json`,
`outdated --json`. Everything else is relayed whole and capped, because a
parser that guesses at a tool's output breaks on that tool's next release.

Consequence: adapters must be swappable. The "Lint" check exists independently
of whether ESLint or Biome satisfies it. Where several tools are detected for the
same check, `opi` shows which one actually ran.

## runemark is the only presentation dependency

All terminal output goes through [runemark](https://github.com/casoon/runemark),
including the interactive list, its filter and the confirmation dialogue. No
second TUI or prompt library.

Consequence: every user-facing string is produced by a runemark `Console`,
`Report` or block type before it is written out. No raw ANSI escapes, no
hand-rolled colour. Bypassing this breaks the colour policy, `NO_COLOR`
compliance and piped output in ways that are hard to notice and hard to undo.

## Terminal behaviour

- `NO_COLOR` is honoured, colour is TTY-aware by default (inherited from
  runemark's existing policy).
- Without a TTY, nothing may block waiting for input. `opi | cat` and `opi` in
  CI must terminate.
- Raw mode is restored on every path out of the list: selection, cancellation,
  error and an unwinding panic. **Not** on a signal that kills the process
  outright — `SIGTERM` and `SIGHUP` run no destructor, and runemark installs no
  signal handlers. `Ctrl-C` is unaffected: in raw mode it arrives as a key and
  closes the list normally.
- Exit codes of executed scripts are passed through, so `opi build && …` works
  in shell chains.

## Destructive operations

Clean is the only area that irreversibly removes data. It is restricted to paths
inside the project directory, does not follow symlinks, and rejects paths from
the `opi` key that escape the project (`..`, absolute paths). `node_modules/` is
never preselected.

***opi* itself writes nothing else.** `--updates` can now apply what it found,
but only by calling the package manager with a list of names. No
`package.json`, no lockfile, no `pnpm-workspace.yaml` is written by `opi`.
Catalogs, overrides and `workspace:` protocols stay the problem of the tool
that understands them.

The earlier rule was "updates are never applied", and its reason was that pnpm
catalogs put the versions outside `package.json`. Measured, pnpm rewrites the
catalog correctly — and 45 of 79 workspaces here use one, so treating catalogs
as the exception was the wrong way round. What survives is the narrower rule
above.

Two things are still refused rather than done badly:

- **Majors never ride along.** The safe packages are passed by name, because
  `pnpm update --latest` with no names ignores the range and the risk alike.
- **Raising ranges is not offered for npm.** It has no command that keeps the
  operator: `npm install x@1.1.1` turns an exact `2.1.2` into `^2.1.3`. An
  exact pin is a statement, and rewriting it in passing is the silent edit this
  section exists to prevent. Only the range-faithful `npm update` appears
  there.

Where two lockfiles disagree and no `packageManager` field settles it, the
offer is withheld and the finding named instead. Writing is where a wrong
detection stops being an annoyance: `pnpm update` in a repository that actually
runs npm leaves a `pnpm-lock.yaml`, and the next `npm ci` resolves against
something nobody saw.

No check is run in a fixing mode.

A script the project marked `confirm` is asked about before it runs, and
refuses without a terminal rather than assuming yes — `--yes` says it out loud
instead.

## Assumptions, not yet settled

- **Nested workspaces** — a member that declares workspaces of its own. None of
  the 81 workspace repositories measured had one, so the behaviour is untested
  rather than decided. For Cargo the rule is at least written down — the
  outermost `[workspace]` wins — but it is a rule without a measurement behind
  it.
- **A `Cargo.toml` far above that does not list this crate.** The outermost
  `[workspace]` is taken as the root without checking that the crate is among
  its `members`, which would need the TOML parser deliberately not taken. In a
  normal repository the question does not arise; a crate checked out underneath
  an unrelated workspace would be misread.
- **Terminal behaviour beyond macOS and Linux.** CI covers both; other Unixes
  are assumed to behave the same and have not been tried.
