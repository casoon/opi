---
title: Clean
description: Removable build artefacts, with what each one costs, removed only when you choose to.
order: 7
---

`C` in the list, or:

```sh
opi --clean
```

`opi` lists the directories a build leaves behind, with their sizes, and offers two choices:
**Build artefacts**, or **Everything, including node_modules**. Nothing is removed without
someone choosing it, so without a terminal it only lists. Run on `opi`'s own repository, piped:

```
opi  clean

  target  552.4 MB

Run opi --clean in a terminal to remove any of it.
```

## What is offered

`dist`, `build`, `out`, `.astro`, `.next`, `.nuxt`, `.svelte-kit`, `.output`, `.turbo`,
`.wrangler`, `.vercel`, `.parcel-cache`, `coverage`, `test-results`, `playwright-report`,
`storybook-static`, `node_modules/.cache`, `node_modules/.vite` — in the project and in each
workspace member — plus Rust's `target/` at the workspace root, and anything listed under
`opi.clean` (see [Configuration](../configuration/)).

`node_modules` is listed, but never rides along with the build artefacts: it is the largest
item and the most expensive to rebuild, so removing it is a choice of its own.

## What is never touched

Clean is the only code in `opi` that removes data, and it is narrow by construction:
directories inside the project only, never through a symlink. A candidate inside another
candidate is dropped, so no size is counted twice.
