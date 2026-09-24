---
title: Health
description: Run every check the project's tools can answer, concurrently, and see what each one said.
order: 3
---

`H` in the list, or:

```sh
opi --health
```

`opi` detects which tools the project depends on, runs them concurrently, and prints each
result as it finishes, so a slow test run does not look like a hang. Run on `opi`'s own
repository:

```
opi  health

✓ Format (rust)       0.1s  cargo
✓ Lint (rust)         0.1s  cargo
✓ Docs (rust)         0.1s  cargo
✓ Lockfile (rust)     0.0s  cargo
✓ Tests (rust)        0.3s  cargo

5 checks passed.
```

## What is detected

| Check | Tools, first match wins |
| --- | --- |
| TypeScript | `astro check` (with `@astrojs/check`), `tsc --noEmit` |
| Lint & format | Biome |
| Lint | ESLint |
| Format | Prettier |
| Tests | Vitest, Jest |
| Dead code | fallow, Knip |
| Secrets | nosecrets, secretlint |
| Lockfile | the package manager's own frozen install, changing nothing — pnpm, npm and bun |
| Rust | `cargo fmt --check`, `cargo clippy` and `cargo rustdoc` with warnings denied, `cargo test --all-features`, and `cargo metadata --locked` where `Cargo.lock` is committed |

A tool applies when the package declares it as a dependency; its binary is taken from a
`node_modules/.bin` at or above the package. A yarn project using Plug'n'Play has no
`node_modules`, so there the tool is started through `yarn run` instead. Where several tools
answer for the same concern, only the first detected one runs, and its name is shown next to
the result.

**Lockfile** asks whether the lockfile still matches `package.json` — the state in which every
CI that installs with a frozen lockfile stops. Each manager is asked with a command measured to
change nothing: `pnpm install --frozen-lockfile --lockfile-only --offline`, `npm ci --dry-run`,
`bun install --frozen-lockfile --dry-run`. yarn gets no lockfile check, because its only strict
form is a full install. It is the one project prerequisite `opi` checks: a declared
`packageManager` version, `engines.node` and `rust-toolchain.toml` were measured never to
disagree with what ran, since pnpm and rustup switch versions themselves.

In a workspace the checks run **per member**, in the member's directory, because that is
where the tools and their config live.

## Reading the result

A failing check's output is relayed whole and capped at twenty lines, with the command to see
the rest. A tool that will not start is reported apart from one that found something, since a
broken install needs a different remedy.

There is no health score. A composite number stops meaning anything within weeks; what a
failing tool actually said does not.

`opi --health` exits non-zero when a check fails.
