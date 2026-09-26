---
title: Scripts
description: How opi groups a project's scripts, lays them out, and addresses workspace members.
order: 1
---

Scripts are the product, not one feature among several. The start screen shows them
immediately — grouped, labelled, with the package manager detected — and `Enter` runs the
highlighted one.

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

The descriptions come from the project's `scripts-info`; see
[Configuration](../configuration/).

The line under the heading counts entries, groups and, in a workspace, packages. Where a
`Makefile`, `justfile`, `Taskfile.yml` or a `mise.toml` with tasks sits beside the manifest, it
says so too — `7 entries · 3 groups · also Makefile` — so the list does not pass for everything
the project can do. `opi` names those files but never reads or runs them.

## Grouping

A script is grouped by the segment before its first `:` — `build:landings:watch` belongs to
Build. Groups appear in a fixed order of meaning:

| Group | Prefixes |
| --- | --- |
| Development | `dev`, `start`, `serve`, `watch` |
| Build | `build`, `bundle`, `compile` |
| Preview | `preview` |
| Quality | `check`, `lint`, `format`, `fmt`, `test`, `audit`, `typecheck`, `type-check`, `types` |
| Deploy | `deploy`, `release`, `publish`, `ship` |
| Maintenance | `clean`, `setup`, `bootstrap`, `upgrade`, `update-deps` |

Any other prefix is a deliberate grouping by the author and becomes its own group,
alphabetically after these. A standalone script that heads a family joins it: `deploy`
belongs with `deploy:blog`. Everything else lands in **Other**.

Lifecycle hooks npm runs on its own — `prebuild` where `build` exists — are hidden.

## Tabs for long lists

Past **15 entries or 5 groups** the groups move into a row of tabs and only the active one's
entries are listed:

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

`←` `→`, `Tab` and the digits switch groups. `/` searches **all** of them. A list taller than
the terminal scrolls, and entries are shortened rather than wrapped.

## Workspaces

`opi` reads member patterns from `pnpm-workspace.yaml` or the `workspaces` field and lists
each member's scripts under the member's name — only those the root does not already define,
since the root's script usually wraps them. A workspace's packages share the last tab, marked
off by a divider.

Where root and member both define a script, a bare name runs the root's and
`<member>/<script>` runs the member's. A scoped package answers to its short name:

```sh
opi dev          # the root's dev
opi blog/dev     # the dev script of @casoon/blog
```

Package manager detection searches upwards too, so a member package in a pnpm monorepo runs
with pnpm.

## Without a terminal

`opi | cat` prints every group as plain text: nothing on the other end of a pipe can press a
key. The interactive list needs both stdout and stderr to be terminals.
