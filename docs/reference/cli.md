---
title: Command line
description: Every flag, hotkey and exit code, as opi --help prints them.
order: 1
---

`opi` has one shape: `opi [flags] [script] [script args…]`. A project's scripts win over
anything built in, so every built-in area is a flag or a hotkey, never a bare word.

```
opi 0.9.0 — Operations Interface

Usage:
  opi                     List what this project can do
  opi <script> [args…]    Run a script, passing args on to it
  opi <member>/<script>   Run a workspace member's script

In the list, ↑↓ move, Enter runs, / filters, H checks, C cleans,
S scans for secrets and vulnerable dependencies, U shows updates.

Options:
  -y, --yes               Answer a script's confirmation in advance
      --health            Run the project's checks
      --clean             Show and remove build artefacts
      --security          Scan for secrets and vulnerable dependencies
      --updates           Show dependencies with newer versions
      --check <workflow>  Run a workflow: commit or release
  -h, --help              Show this help
  -V, --version           Show the version

Entries come from package.json and Cargo.toml. A project's own scripts
always take precedence over built-in names, so a project with a script
called "clean" keeps working — every built-in area is a flag or a
hotkey instead.
```

## Parsing rules

- The first bare word is the script. Everything after it belongs to the script, including
  anything that looks like a flag: `opi build --verbose` passes `--verbose` to `build`.
- A literal `--` directly after the script name is dropped; a second one is passed on.
- `--yes` (`-y`) is the only flag that may precede a script. After the script name it
  belongs to the script.
- `--check` needs a workflow name, `commit` or `release`.
- An unknown flag before the script is an error, and so is an unknown script name — with
  a suggestion where one is close.

## Keys in the list

| Key | Action |
| --- | --- |
| `↑` `↓` | Move |
| `Enter` | Run the highlighted script |
| `/` | Filter across all groups |
| `←` `→`, `Tab`, digits | Switch group, when the groups are tabs |
| `H` | [Health](../../guides/health/) |
| `S` | [Security](../../guides/security/) |
| `U` | [Updates](../../guides/updates/) |
| `C` | [Clean](../../guides/clean/) |

## Exit codes

| Command | Exit code |
| --- | --- |
| `opi <script>` | The script's own — the script replaces the `opi` process |
| `opi --health` | Non-zero when a check fails |
| `opi --check <workflow>` | Non-zero when not ready |
| `opi --security` | Non-zero when something needs acting on |
| Unknown flag or script | `1` |

## Crate

`opi` is a binary crate without a library API, so there is no API reference on docs.rs — its
build there fails with `no library targets found`, which is what every binary-only crate gets and
no metadata can change. The manifest's `documentation` field therefore points at this site. The
package is on [crates.io/crates/opi](https://crates.io/crates/opi); the internals are described in
[Architecture](../../architecture/).
