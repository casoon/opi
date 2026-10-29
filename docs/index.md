---
title: Overview
description: What opi is, what it covers, and how this documentation is organised.
order: 0
---

`opi` — **Operations Interface** — is a project control center for your terminal. Go into any
repository, type `opi`, and get a usable interface for that project without configuring
anything first.

Its primary job is to be a better `npm run`: the start screen shows the project's scripts,
grouped and labelled, with the package manager detected rather than typed. Health, security,
updates and clean sit one hotkey away, and four workflows run the right checks before a
commit, a push, a release or a deployment — the push one as a pre-push hook if you want it binding.

**Status:** `0.13.1`. npm, pnpm, bun and Cargo projects are covered; yarn's scripts, health and
security audit are too — its update listing is not read yet. `opi` runs on macOS and Linux
only.

## Where to start

- [Installation](getting-started/installation/) — the install script or `cargo install opi`.
- [Quickstart](getting-started/quickstart/) — the three ways to run a script.
- [Scripts](guides/scripts/) — how the list is grouped, and how it behaves in a monorepo.
- [Hooks and CI](guides/integrations/) — the same checks in pre-commit and pre-push hooks, CI jobs and deploy steps.
- [Command line](reference/cli/) — every flag and hotkey.

## How it works

`opi` reimplements none of the tools it surfaces. It detects which one a project depends on —
Biome, ESLint, Prettier, `tsc`, `astro check`, Vitest, Jest, fallow, Knip, a secret scanner,
`cargo` — runs it, and relays what came back. Without a detected root secret scanner,
opi uses its bundled nosecrets engine, including in Rust-only projects. Commit workflows
also inspect staged whitespace, conflict markers and the size of newly added files.
Output from external tools is parsed only where the format is
documented (`audit --json`, `osv-scanner --format json`, `outdated --json`, `audit signatures --json`, `licenses list --json`).

The project's commands come from `package.json` and `Cargo.toml`; an optional `opi` key
refines script metadata. `opi --checks` saves workflow check selections separately in
`.opi/checks.json`. Commands stay in the project's own manifest.

## Project documentation

These pages also describe the codebase as it stands — for contributors, and for anyone who
wants to know why `opi` behaves the way it does:

- [Project state](project-state/) — what this is and where it stands
- [Architecture](architecture/) — the modules and how they fit
- [Constraints](constraints/) — the boundaries the implementation respects
- [Conventions](conventions/) — the rules future changes follow
- [Decisions](decisions/) — the decisions in force, and why
