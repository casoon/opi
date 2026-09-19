# opi

**Operations Interface** — a project control center for your terminal.

Go into any repository, type `opi`, and get a usable interface for that
project — without configuring anything first.

> **Status: `0.5.0`.** Scripts are the main thing; health, security, updates,
> clean and the commit/release workflows are built on top of them.

## What it does

Its primary job is to be a better `npm run`. The start screen shows the
project's scripts — grouped by prefix, labelled from `scripts-info`, with the
package manager detected rather than typed:

```
astro-v7-workspace  pnpm

Development
› dev              Startet die Starter-App im Dev-Modus
  dev:blog         Startet die Blog-App im Dev-Modus
Build
  build            Baut Starter- und Blog-App
  build:blog       Baut die Blog-App
Preview
  preview          Vorschau des Starter-Builds
Quality
  check            Prüft das Repo mit Biome
  check:fix        Behebt Lint- und Formatfehler
```

`Enter` runs the highlighted script. That is the shortest path, and nothing is
placed in front of it. A list taller than the terminal scrolls, and `/` filters
it as you type — in a monorepo that is the difference between scrolling past
thirty entries and typing three letters.

Three speeds, all backed by the same task model:

```bash
opi                   # navigate: ↓ ↓ Enter
opi                   # filter:   /dep ↵
opi dev               # direct, no interface
```

Arguments after the name go to the script: `opi build --verbose`.

In a monorepo each workspace member's scripts appear under the member's name —
only those the root does not already define, since the root's script usually
wraps them and wins the name anyway. Where both exist, a bare name runs the
root's and `blog/dev` runs the member's:

```bash
opi dev          # the root's dev
opi blog/dev     # the blog package's dev
```

`opi` also works from anywhere inside a project, not only from the directory
holding `package.json`.

The script replaces the `opi` process, so `Ctrl-C` reaches your dev server
rather than killing a wrapper, and the script's exit code is what your shell
sees — `opi build && …` works.

## More than one kind of project

A repository can be several things at once, and `opi` does not make it choose.
`package.json` and `Cargo.toml` are both read, and their commands share one
list:

```
llmux-dashboard  npm · cargo

Development
  cargo run     Build and run the binary
  dev
Build
  build
  cargo build   Build in release mode
  cargo check   Type-check without building
```

Health and clean follow: `cargo fmt --check`, `cargo clippy` and `cargo test`
run alongside the npm checks, and `target/` is listed with the build artefacts.

## Saying more about a script

Optional, and only ever refinement — a project with neither `opi` nor
`scripts-info` still gets a usable list, which is the whole point.

```json
{
  "scripts": { "dev": "astro dev", "deploy": "wrangler deploy" },
  "scripts-info": { "dev": "Start development server" },
  "opi": {
    "scripts": {
      "dev":    { "favorite": true },
      "deploy": { "group": "Deployment", "confirm": true }
    }
  }
}
```

| | |
| --- | --- |
| `description` | Wins over `scripts-info`, which stays valid for `nr` |
| `group` | Replaces the group the script's name implies |
| `favorite` | Lifts it out of its group, to the top |
| `confirm` | Asks before running it; `opi --yes <script>` answers in advance |

A `confirm` script refuses to run without a terminal rather than assuming yes —
otherwise the protection would vanish in exactly the case it exists for.

## The other areas

Each is a hotkey in the list and a flag on the command line. Never a bare word:
`health`, `clean` and `release` are script names in real projects, and the bare
word stays theirs.

| | | |
| --- | --- | --- |
| `H` | `opi --health` | Runs every check the project's tools can answer, concurrently |
| `S` | `opi --security` | Secret scan and a parsed dependency audit |
| `U` | `opi --updates` | What is outdated, split into safe and major |
| `C` | `opi --clean` | Removable artefacts, with what each one costs |
| | `opi --check commit` | The fast checks, before you commit |
| | `opi --check release` | Everything, plus a clean tree and an untagged version |

```
astro-v7-workspace  health

✓ Lint & format                     0.2s  biome
✗ Secrets                           1.0s  nosecrets
✓ TypeScript (@astro-v7/starter)    5.2s  astro
✓ TypeScript (@astro-v7/blog)       5.4s  astro
```

`opi` reimplements none of this. It detects which tool a project depends on —
Biome, ESLint, Prettier, `tsc`, `astro check`, Vitest, Jest, fallow, Knip, a
secret scanner, `cargo` — runs it, and relays what came back. In a workspace the
checks run per member, in the member, because that is where the tools and their
config live.

Output is parsed only where the format is documented (`audit --json`,
`outdated --json`). Everything else is relayed whole and capped at twenty lines,
with the command to see the rest: a parser that guesses at a tool's output
breaks on that tool's next release.

There is no health score. A composite number stops meaning anything within
weeks; what a failing tool actually said does not.

## Design rules

- **Zero configuration.** `opi` must be useful in an unmodified repository. A
  tool you have to configure first never gets started in someone else's project.
- **The project's own files are the only interface.** `package.json` and
  `Cargo.toml` — no `opi.toml`, no second source of truth of `opi`'s own
  making. `scripts`, optionally `scripts-info`, optionally an `opi` key for
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
- [docs/architecture.md](docs/architecture.md) — the modules and how they fit
- [docs/constraints.md](docs/constraints.md) — the boundaries the implementation respects
- [docs/conventions.md](docs/conventions.md) — the rules future changes follow
- [docs/decisions.md](docs/decisions.md) — the decisions in force, and why

Terminal presentation comes from [runemark](https://github.com/casoon/runemark),
the shared presentation layer for these CLI tools.

## Name

`opi` is unrelated to [OdradekAI/opi](https://github.com/OdradekAI/opi), which
owns the `opi-*` crate namespace on crates.io. Installing both puts two `opi`
binaries in `~/.cargo/bin`.

## License

MIT ([LICENSE](LICENSE)).
