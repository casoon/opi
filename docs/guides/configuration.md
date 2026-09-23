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

A `group` naming one `opi` already knows — `Development`, `Build`, `Preview`, `Quality`,
`Deploy` (or `Deployment`), `Maintenance` — takes that group's fixed place in the order.
Any other name becomes a group of its own.

A favourite in a workspace member stays with its member rather than joining the root's
Favorites, so it still says which package it belongs to.

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
