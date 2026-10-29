---
title: Configuration
description: Optional script refinements in package.json and workflow check selections in .opi/checks.json.
order: 2
---

None of this is required. A project with neither `scripts-info` nor an `opi` key still gets a
usable list — that is the point. Script metadata refines what `package.json` already says.
Workflow check selections are optional and live in `.opi/checks.json`.

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

## Per script

| Key | Effect |
| --- | --- |
| `description` | Wins over `scripts-info`, which stays valid for `nr` |
| `group` | Replaces the group the script's name implies |
| `favorite` | Lifts it out of its group, to the top |
| `confirm` | Asks before running it; `opi --yes <script>` answers in advance |

None of it is required: a project with neither an `opi` block nor `scripts-info` still gets a
usable list, which is the point. The block is held as unparsed JSON and read field by field,
so a `favorite` written as a string rather than a boolean costs that one field — not the
entry, and certainly not a `package.json` whose `scripts` are perfectly fine.

A `group` naming one `opi` already knows — `Development`, `Build`, `Preview`, `Quality`,
`Deploy` (or `Deployment`), `Maintenance` — takes that group's fixed place in the order.
Any other name becomes a group of its own.

`favorite` lifts a script into a Favorites group at the top rather than sorting it first
within its own group, which would barely show — measured on a real project, five of ten groups
held two entries. A favourite in a workspace member stays with its member rather than joining
the root's Favorites, so it still says which package it belongs to.

A `confirm` script refuses to run without a terminal rather than assuming yes — otherwise the
protection would vanish in exactly the case it exists for. In a script or CI, say it out loud:

```sh
opi --yes deploy
```

## Clean paths

`opi.clean` adds directories to what [Clean](../clean/) offers:

```json
{
  "opi": { "clean": ["dist", ".astro"] }
}
```

A path that escapes the project is refused rather than corrected.

## Workflow checks

```sh
opi --checks          # choose a workflow first
opi --checks push
opi --checks deploy
```

Select checks with Space, confirm with Enter, or leave with Escape without saving.
The menu starts with the current selection, or the workflow defaults. A check name
selects that concern across all detected tools and workspace members. Build is a
separate choice and runs after the checks. Configuration requires a terminal and
never runs the checks itself.

The project root's `.opi/checks.json` stores one selection per workflow:

```json
{
  "workflows": {
    "push": {
      "checks": ["Secrets"],
      "build": false
    },
    "deploy": {
      "checks": ["Secrets", "Tests"],
      "build": true
    }
  }
}
```

Names must match the menu exactly: for example Biome supplies `Lint & format`,
whereas Rust supplies `Lint` and `Format`. The example deploy selection requires
an available test runner and build. Unknown names, malformed configuration and
selected checks or builds that are unavailable fail the workflow. They are never
silently skipped. Unavailable saved checks remain visible in the menu so they can
be deselected.

An absent workflow entry uses its defaults. Remove an entry to reset it; removing
`.opi/checks.json` resets every workflow. An explicit empty `checks` array with
`build: false` runs only the workflow's Git gates. Selecting checks does not change
Health or Security. The Git gates themselves retain their workflow defaults.

The configuration is shareable in Git. Opi preserves `.opi/.gitignore` and adds
exceptions for `checks.json` and `.gitignore`; generated SBOMs remain ignored.
No manifest, lockfile, script or command is rewritten.

Run the selection with `opi --check push` or `opi --check deploy`. The deploy
workflow requires a clean tree, defaults to all detected checks plus the build,
and does not execute a deploy script. To check and then deploy:

```sh
opi --check deploy && opi deploy
```
