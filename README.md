# opi

**Operations Interface** — a project control center for your terminal.

Go into any repository, type `opi`, and get a usable interface for that
project — without configuring anything first.

> **Status: `0.11.0`.** Scripts are the main thing; health, security, updates,
> clean and the commit/push/release workflows are built on top of them. npm, pnpm,
> bun and Cargo projects are covered; yarn's scripts, health and security
> audit are too — its update listing is not read yet.

![opi listing a monorepo's scripts, switching groups, searching across all of them and running one](https://raw.githubusercontent.com/casoon/opi/main/assets/opi.gif)

## What it does

Its primary job is to be a better `npm run`. The start screen shows the
project's scripts — grouped by prefix, labelled from `scripts-info`, with the
package manager detected rather than typed:

```
astro-v7-workspace  pnpm
  12 entries · 4 groups

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

Past **15 entries or 5 groups** the groups move into a row of tabs and only the
active one's entries are listed, because a heading per group over thirty-odd
scripts is taller than the terminal:

```
casoon-web  pnpm
  43 entries · 10 groups · 3 packages

  1 Development   2 Build   3 Preview   4 Quality   …   │   8 Packages
  ─────────────
› dev              Startet die Homepage
  dev:homepage     Startet die Homepage
  dev:insights     Startet Insights
  dev:webcheck     Startet Webcheck

←→ group   / search   H Health   C Clean   S Security   U Updates
```

A workspace's packages share the last tab rather than taking one each — 52 of
them against 6 action groups is a tab row nobody can use — and a divider marks
it, because the other tabs name what you are doing and that one names where.

The first group is preselected and the cursor is on its first entry, so `opi ↵`
still starts `dev` — the tabs hide what is not needed rather than asking for a
choice first. `←` `→`, `Tab` and the digits switch groups; `/` searches **all**
of them, which is the case the tabs are worst at. `opi | cat` lists every group
as before: nothing on the other end of a pipe can press a key.

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

![opi dev starting the dev server directly, then opi api/db:migrate running a workspace member's script by path](https://raw.githubusercontent.com/casoon/opi/main/assets/direct.svg)

`opi` also works from anywhere inside a project, not only from the directory
holding `package.json`.

The script replaces the `opi` process, so `Ctrl-C` reaches your dev server
rather than killing a wrapper, and the script's exit code is what your shell
sees — `opi build && …` works.

## More than one kind of project

![opi listing a repository that carries both package.json and Cargo.toml](https://raw.githubusercontent.com/casoon/opi/main/assets/ecosystems.gif)

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

Every area follows: `cargo fmt --check`, `cargo clippy`, `cargo rustdoc` and
`cargo test` run alongside the npm checks, `cargo audit` and `cargo outdated`
answer for the crates where they are installed, and `target/` is listed with
the build artefacts. A repository carrying both manifests gets a section each
rather than one merged list — `Dependencies` and `Dependencies (rust)` —
because "safe to take" orders within an ecosystem and not across two.

Both are external cargo subcommands rather than part of the toolchain, so
where one is missing `opi` says so along with the `cargo install` that adds
it, instead of leaving a section that reads like "nothing found".

`opi --security` also runs `osv-scanner` over the lockfiles (npm, pnpm, yarn,
bun, Cargo) as a section of its own, `Dependencies (osv)`. It is an additional
source, not a replacement: the native audits stay, and OSV lists only the
packages they did not already report. Where one of them is missing or failed,
all OSV findings are shown. Without `osv-scanner` the section says so.

In a Cargo workspace `opi` works on the workspace even when started inside one
of its crates — that is where `target/` lives and where the checks reach every
member. `cargo run` still follows you: it is offered in a binary crate, because
that is where cargo would start it.

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

![opi --health running five checks in a project with package.json and Cargo.toml, one failing with rustfmt's diff shown under it](https://raw.githubusercontent.com/casoon/opi/main/assets/health.svg)

Each is a hotkey in the list and a flag on the command line. Never a bare word:
`health`, `clean` and `release` are script names in real projects, and the bare
word stays theirs.

| | | |
| --- | --- | --- |
| `H` | `opi --health` | Runs every check the project's tools can answer, concurrently |
| `S` | `opi --security` | Secret scan, a parsed dependency audit per ecosystem, signatures, release age and licenses (pnpm, npm, bun and Cargo; production dependencies only) |
| `U` | `opi --updates` | What is outdated, split into safe and major — and the offer to take it |
| `C` | `opi --clean` | Removable artefacts, with what each one costs |
| | `opi --check commit` | The fast checks, before you commit |
| | `opi --check push` | Everything plus the build, on a clean tree that is not behind upstream |
| | `opi --check release` | Everything plus the build, a clean tree and an untagged version |
| | `opi --hooks` | Installs `opi --check push` as the pre-push hook |
| | `opi --sbom [dir]` | CycloneDX SBOMs, one file per ecosystem, written to `dir` or the project root |

`--sbom` writes `sbom-npm.cdx.json` (from `npm sbom`, or `pnpm sbom` in a pnpm
project; Yarn and Bun projects use `npm sbom` too, which reads their
lockfile and `node_modules`) and `sbom-cargo.cdx.json` (from `cargo cyclonedx`).
The documents are not merged, so each stays attributable to its producer. A
missing producer is named with the command that installs it, e.g.
`cargo install cargo-cyclonedx`.

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
Output is parsed only where the format is documented (`audit --json`,
`osv-scanner --format json`, `outdated --json`, `audit signatures --json`, `licenses list --json`, `sbom --sbom-format cyclonedx`, `cargo metadata`). Everything else is relayed whole and capped at twenty lines,
