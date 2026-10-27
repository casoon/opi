---
title: Security
description: A secret scan, a parsed dependency audit per ecosystem, and three supply-chain questions for npm projects.
order: 5
---

`S` in the list, or:

```sh
opi --security
```

Two questions, answered by tools the project already uses:

- **Secrets** — the secret scanner the project depends on (nosecrets or secretlint). Where
  there is none, `opi` says so rather than reporting a clean result.
- **Dependencies** — the package manager's audit, parsed from its JSON: `npm audit` and
  `pnpm audit` share npm's format, bun and yarn each have their own.

Run on `opi`'s own repository, which has no secret scanner and no `package.json`:

```
opi  security

No secret scanner: this project depends on none that opi knows.

✓ Dependencies (rust) — no known vulnerabilities

Nothing to act on.
```

## Supply chain

A `package.json` project gets up to three more lines, each answered by the package manager
itself — pnpm all three, npm and bun two (licenses from `npm sbom`), yarn only the signatures:

- **Signatures** — `npm audit signatures`: whether every installed package carries a valid
  registry signature. It reads `node_modules`, so it works under pnpm and bun too; under
  Yarn Plug'n'Play there is nothing for it to read. A signature that does not verify is the
  only one of the three that fails the run.
- **Release age** — whether freshly published versions are held back:
  `minimumReleaseAge` for pnpm, `min-release-age` for npm. Asked with `config get`, so a
  global setting counts as well as the workspace file. yarn and bun are not asked.
- **Licenses** — the most common licenses with their counts, and by name every package under
  a copyleft, source-available, missing or `UNLICENSED` license. Production dependencies only:
  from `pnpm licenses list --prod`, from `npm sbom --omit dev` for npm and bun, from
  `cargo metadata` for Cargo.

```
✓ Signatures — every installed package verified
! Release age — not set: a version installs the moment it is published
  minimumReleaseAge: <minutes> in pnpm-workspace.yaml
! Licenses — 480 MIT · 22 ISC · 11 Apache-2.0 · 9 BSD-2-Clause · 20 other
  @img/sharp-libvips-darwin-arm64  LGPL-3.0-or-later
  lightningcss  MPL-2.0
```

Four more lines appear where there is something to say:

- **Install scripts** — dependencies that declare `preinstall`, `install` or `postinstall`,
  and whether the project allows them (`onlyBuiltDependencies` / `ignoredBuiltDependencies`
  for pnpm, `trustedDependencies` for bun, `ignore-scripts` for npm). Scripts nothing has
  decided on are named.
- **Sources** — dependencies resolved from git, a tarball URL or a path outside the
  workspace, read from `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock` and
  `Cargo.lock`.
- **GitHub Actions** — `uses:` references in `.github/workflows` that name a tag or branch
  instead of a 40-character commit SHA, with file and line.
- **Dependencies (osv)** — `osv-scanner` over all lockfiles, shown beside the native audits
  and only for what they did not report. Without `osv-scanner` the section says so.

An unset release age and a copyleft dependency are reported, not judged: both can be a
deliberate choice, so neither changes the exit code.

## Findings

Advisories are condensed to one line per package and severity, which is what decides what to
do about them — the severity counts are therefore counts of vulnerable dependencies, not of
advisories. Where the advisory names a patched range, it becomes the remedy. bun and yarn name
only the vulnerable range, so their findings carry no remedy: deriving "update to 4.17.21"
from `<4.17.21` would be inventing the one number that has to be right.

### Three shapes behind one question

Every manager is asked for its audit as JSON, and three of them answer in a shape of their
own. Which one to read is decided once, by which manager was asked, never by guessing from
the document:

- **npm and pnpm** return an object carrying one advisory per package.
- **bun** returns a map from package name to a **list** of advisories — which npm's shape
  cannot express. Measured against bun 1.3.3: clean JSON on stdout, its banner on stderr.
- **yarn** returns newline-separated JSON, one object per advisory, with capitalised keys.
  Measured against yarn 4.18.0 with `yarn npm audit --all --recursive --json`.

yarn is also the one manager that prints **nothing at all** when it finds nothing, rather than
an empty report. An empty result is therefore read as clean only when stderr is empty too —
the same distinction `--updates` makes for npm and pnpm.

## Rust

A project with a `Cargo.toml` gets its own section, `Dependencies (rust)`, answered by
`cargo audit`. It is an external cargo subcommand; where it is missing, `opi` names the
`cargo install` that adds it.

The Rust advisory carries no severity in its JSON, and `opi` invents none. The key is simply
absent; only a CVSS vector string is there. `cargo audit`'s own console output does print
`Severity: 7.5 (high)`, because it scores that vector itself — so every Rust advisory counts
here, and the next step points at `cargo audit`. Scoring the vector inside `opi` would mean
reimplementing CVSS, and reading the number out of console text would mean a parser on an
undocumented format.

`opi --security` exits non-zero when something needs acting on — including an audit that
could not run, such as one stopped by a network error. That audit has said nothing about the
dependencies, so it is not reported as clean. A missing `cargo audit` is named with its
`cargo install` instead and does not fail the run.
