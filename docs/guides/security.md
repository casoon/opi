---
title: Security
description: A secret scan and a parsed dependency audit, one section per ecosystem.
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

## Findings

Advisories are condensed to one line per package and severity, which is what decides what to
do about them — the severity counts are therefore counts of vulnerable dependencies, not of
advisories. Where the advisory names a patched range, it becomes the remedy. bun and yarn name
only the vulnerable range, so their findings carry no remedy: deriving one would be inventing
the one number that has to be right.

## Rust

A project with a `Cargo.toml` gets its own section, `Dependencies (rust)`, answered by
`cargo audit`. It is an external cargo subcommand; where it is missing, `opi` names the
`cargo install` that adds it.

The Rust advisory carries no severity in its JSON, and `opi` invents none. Every Rust
advisory therefore counts, and the next step points to `cargo audit`, where the severity is
shown.

`opi --security` exits non-zero when something needs acting on — including an audit that
could not run, such as one stopped by a network error. That audit has said nothing about the
dependencies, so it is not reported as clean. A missing `cargo audit` is named with its
`cargo install` instead and does not fail the run.
