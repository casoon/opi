# opi

**Operations Interface** — a project control center for your terminal.

Go into any repository, type `opi`, and get a usable interface for that
project — without configuring anything first.

> **Status: `0.0.1` is a placeholder that reserves the crate name.**
> The tool is not implemented yet. The first working release (`0.1.0`) will do
> one thing: make a project's `package.json` scripts immediately runnable.

## What it will do

Its primary job is to be a better `npm run`. The start screen shows the
project's scripts — grouped by prefix, labelled from `scripts-info`, with the
package manager detected rather than typed:

```
casoon.dev                                      pnpm
  Scripts
  ─────────────────────────────────────────────────────────
  Development
  › dev              Startet casoon.dev
    dev:landings     Startet die Produkt-Landingpages
  Build
    build            Baut Website + Landingpages
    build:landings   Baut nur die Landingpages
  Check
    check            Lint + Formatierung prüfen
    check:fix        Fehler automatisch beheben
  ─────────────────────────────────────────────────────────
  U  Updates       C  Clean       S  Secrets       H  Health
  /  Search        ?  Help
```

`Enter` runs the highlighted script. That is the shortest path, and nothing is
placed in front of it — maintenance areas sit behind hotkeys.

Three speeds, all backed by the same task model:

```bash
opi              # navigate:  ↓ ↓ Enter
opi              # search:    /dev land ↵
opi dev          # direct, no interface
```

In a monorepo each workspace member's scripts appear under the member's name.
Root and member scripts share names in practice, so a bare name runs the
root's and `blog/dev` runs the member's:

```bash
opi dev          # the root's dev
opi blog/dev     # the blog package's dev
```

`opi` also works from anywhere inside a project, not only from the directory
holding `package.json`.

Later releases add project health with parallel checks, dependency updates,
clean with real sizes, and secret scanning — each orchestrating an established
tool (`tsc`, ESLint, Biome, Vitest, Knip, No Secrets, `pnpm audit`, Taze) rather
than reimplementing it.

## Design rules

- **Zero configuration.** `opi` must be useful in an unmodified repository. A
  tool you have to configure first never gets started in someone else's project.
- **`package.json` is the only interface.** No `opi.toml`, no second source of
  truth. `scripts`, optionally `scripts-info`, optionally an `opi` key for
  refinement.
- **No task system of its own.** `opi` does not define tasks with their own
  commands — that would put it in competition with npm scripts, `just`, `make`
  and Taskfile for no gain.
- **Orchestration, not reimplementation.** `opi` is a UX layer over proven
  tooling.

## Install

```bash
cargo install opi
```

**Unix only.** `opi` runs a script by replacing its own process with `exec`, so
`Ctrl-C` reaches the dev server rather than killing a wrapper, and its
interactive list drives termios directly. Windows offers neither, and a second
execution model that nothing exercises would be worse than an honest boundary.
Building on Windows fails with that message rather than producing a degraded
binary.

## Documentation

- [docs/project-state.md](docs/project-state.md) — what this is and where it stands
- [docs/constraints.md](docs/constraints.md) — the boundaries the implementation respects
- [docs/decisions.md](docs/decisions.md) — the decisions in force, and why

Terminal presentation comes from [runemark](https://github.com/casoon/runemark),
the shared presentation layer for these CLI tools.

## Name

`opi` is unrelated to [OdradekAI/opi](https://github.com/OdradekAI/opi), which
owns the `opi-*` crate namespace on crates.io. Installing both puts two `opi`
binaries in `~/.cargo/bin`.

## License

MIT ([LICENSE](LICENSE)).
