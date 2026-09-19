# Constraints

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
3. `opi` — project-specific refinement (health step list, clean paths), never a
   precondition

## No task system of its own

`opi` does not define tasks with their own commands. The moment it does, it
competes with npm scripts, `just`, `make`, `mise` and Taskfile — a contest with
no upside — while also becoming a tool that must be configured first.

Workflows (`commit`, `release`) are named lists of existing task names, not a new
task type.

## Orchestration, not reimplementation

No check is implemented inside `opi`. TypeScript errors come from `tsc`, unused
exports from Knip, vulnerabilities from `pnpm audit`. `opi` detects which tool a
project uses, invokes it, parses the result and presents it.

Consequence: adapters must be swappable. The "Lint" check exists independently
of whether ESLint or Biome satisfies it. Where several tools are detected for the
same check, `opi` shows which one actually ran.

## runemark is the only presentation dependency

All terminal output goes through [runemark](https://github.com/casoon/runemark) —
including the interactive selection layer, which runemark is being extended to
provide (see [decisions.md](decisions.md)). No second TUI or prompt library.

Consequence: every user-facing string is produced by a runemark `Console`,
`Report` or block type before it is written out. No raw ANSI escapes, no
hand-rolled colour. Bypassing this breaks the colour policy, `NO_COLOR`
compliance and piped output in ways that are hard to notice and hard to undo.

## Terminal behaviour

- `NO_COLOR` is honoured, colour is TTY-aware by default (inherited from
  runemark's existing policy).
- A signal that kills the process outright while the interactive list is open
  leaves the terminal in raw mode; no destructor runs. `Ctrl-C` is unaffected —
  in raw mode it arrives as a key and closes the list normally.
- Without a TTY, nothing may block waiting for input. `opi | cat` and `opi` in
  CI must terminate.
- Raw mode must be restored on abort, error, panic and signal.
- Exit codes of executed scripts are passed through, so `opi build && …` works
  in shell chains.

## Destructive operations

Clean is the only area that irreversibly removes data. It is restricted to paths
inside the project directory, does not follow symlinks, and rejects paths from
the `opi` key that escape the project (`..`, absolute paths). `node_modules/` is
never preselected.

Operations that modify files — dependency updates, auto-fixes — show a diff
preview and require confirmation before writing.

## Assumptions, not yet settled

