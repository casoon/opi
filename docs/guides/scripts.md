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

Lifecycle hooks npm runs on its own — `prebuild` where `build` exists — are hidden. So is
npm's own `prepare`, which appeared in 49 of 133 measured projects; all of them stay runnable
by name.

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

`←` `→`, `Tab` and the digits switch groups. `/` searches **all** of them.

**A threshold rather than the terminal's own height**, which would have fitted more exactly.
The height changes while the menu is open, and a list that rearranged itself mid-keystroke
would move entries under a cursor already on its way to one. The case it was built for:
`web-casoon` has 27 scripts in its root in 7 groups, which is 34 lines with a heading each —
taller than a full-screen terminal, so the top scrolled away. `opi | cat` lists every group
either way.

The 0.1.0 rule survives the tabs: the first group is preselected with the cursor on its first
entry, so `opi ↵` still runs `dev`. A submenu would demand a choice before anything runnable
is on screen; the tabs demand nothing and hide what is not needed.

**Every tab is an action**, except the one the divider marks as something else. Workspace
packages share a single `Packages` tab rather than taking one each — one tab per package was
measured and does not scale: the largest workspace here has 52 packages against 6 action
groups, a tab row of 58 that is almost entirely package names, permanently scrolling, with the
digits worthless past the ninth. Collapsed it is seven tabs, and `web-casoon` is eight instead
of ten. It also removes a mix-up: action tabs answer "what am I doing", package tabs "where am
I doing it", and standing side by side as peers meant "where is the thing that deploys
insights?" had two right answers.

**Entries are shortened rather than wrapped**, with an ellipsis marking the cut, and where the
name column leaves no useful room the description is dropped rather than cut to a stub.
Wrapping to column zero destroyed the two-column layout — found on a real project with 41
entries and lines up to 101 columns wide.

## Searching

`/` filters the list as you type. Groups with nothing left disappear and the cursor sits on
the best match. On one real project this narrows 29 entries to 3 for `dep`; `og` finds a
workspace member's `generate:og` that would otherwise be twenty rows down.

Matching ranks a name the user is typing towards above a description that happens to share
the same letters, and a tight run of characters above those letters scattered through a longer
name.

## Workspaces

`opi` reads member patterns from `pnpm-workspace.yaml` or the `workspaces` field and lists
each member's scripts under the member's name — only those the root does not already define,
since the root's script usually wraps them and the root wins that name on the command line
anyway. On one real project that removed 12 of 18 member entries, none of which carried a
description; a member left with nothing shows no group at all. A workspace's packages share the last tab, marked
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
