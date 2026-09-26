---
title: Quickstart
description: Open the list, filter it, or run a script by name.
order: 2
---

Change into a project — any directory inside it works, `opi` searches upwards for
`package.json` and `Cargo.toml` — and run it:

```sh
opi
```

The start screen lists the project's scripts. `Enter` runs the highlighted one; the cursor
starts on the first entry of the first group, so `opi ↵` usually starts `dev`.

## Three speeds

All three are backed by the same task model:

```sh
opi                   # navigate: ↓ ↓ Enter
opi                   # filter:   /dep ↵
opi dev               # direct, no interface
```

Arguments after the name go to the script:

```sh
opi build --verbose
```

A literal `--` before the script's arguments is accepted and dropped, since that is the habit
`npm run` teaches.

## The script replaces opi

The script replaces the `opi` process. `Ctrl-C` reaches your dev server rather than a
wrapper, and the script's exit code is what your shell sees, so `opi build && …` works.

## The other areas

From the list, a hotkey opens each area; on the command line it is a flag:

| Key | Flag | Area |
| --- | --- | --- |
| `H` | `opi --health` | [Health](../../guides/health/) |
| `S` | `opi --security` | [Security](../../guides/security/) |
| `U` | `opi --updates` | [Updates](../../guides/updates/) |
| `C` | `opi --clean` | [Clean](../../guides/clean/) |
| | `opi --check commit` | [Workflows](../../guides/workflows/) |
| | `opi --check push` | [Workflows](../../guides/workflows/) |
| | `opi --check release` | [Workflows](../../guides/workflows/) |
| | `opi --hooks` | [Workflows](../../guides/workflows/#the-pre-push-hook) |

Never a bare word: `health`, `clean` and `release` are script names in real projects, and
the bare word stays theirs.
