---
title: Configuration
description: Optional refinements in package.json — descriptions, groups, favourites, confirmation and clean paths.
order: 2
---

None of this is required. A project with neither `scripts-info` nor an `opi` key still gets a
usable list — that is the point. Everything here refines what `package.json` already says;
there is no `opi.toml`.

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
