---
title: Command line
description: Every flag, hotkey and exit code, as opi --help prints them.
order: 1
---

`opi` has one shape: `opi [flags] [script] [script args…]`. A project's scripts win over
anything built in, so every built-in area is a flag or a hotkey, never a bare word.

```
opi 0.13.4 — Operations Interface

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
      --check <workflow>  Run a workflow: commit, push, release or deploy
      --json              With --health, --security, --updates or --check: print JSON
      --sbom [dir]        Write CycloneDX SBOMs, one file per ecosystem
      --hooks             Install the push workflow as the pre-push hook
      --checks [workflow] Select checks for a workflow
      --completions <sh>  Print a completion script: bash, zsh or fish
  -h, --help              Show this help
  -V, --version           Show the version

Entries come from package.json and Cargo.toml. A project's own scripts
always take precedence over built-in names, so a project with a script
called "clean" keeps working — every built-in area is a flag or a
hotkey instead.
```

`--checks [workflow]` opens the check selector and saves `.opi/checks.json` at the
project root. It needs a terminal. Without a workflow name it first asks which
workflow to configure. [Configuration](../../guides/configuration/#workflow-checks)
describes the selections and defaults.

`--completions <shell>` prints a completion script for `bash`, `zsh` or `fish`; setup is in
[Installation](../../getting-started/installation/#shell-completions). The scripts complete flags
and workflow names statically and ask `opi --list-names` for the script names of the current
project. That hidden flag prints one name per line, without the interface, and skips scripts
`opi` keeps out of the list.

## Check scope

`--health` and `--security` use the bundled nosecrets scanner when the root has
no detected project scanner. No separate scanner installation is required;
`.nosecrets.toml` and `.nosecretsignore` at the repository root are respected.

In Git repositories, `--check commit` switches bundled or detected nosecrets to
staged contents and checks whitespace errors, conflict markers and new files
larger than 500 KiB. Other project tools retain their usual scope. It does not modify or stage files.
`--hooks` installs only the pre-push hook. Details: [Workflows](../../guides/workflows/).

## Parsing rules

- The first bare word is the script. Everything after it belongs to the script, including
  anything that looks like a flag: `opi build --verbose` passes `--verbose` to `build`.
- A literal `--` directly after the script name is dropped; a second one is passed on.
- `--yes` (`-y`) is the only flag that may precede a script. After the script name it
  belongs to the script.
- `--check` needs a workflow name: `commit`, `push`, `release` or `deploy`.
- An unknown flag before the script is an error, and so is an unknown script name — with
  a suggestion where one is close, matched by optimal string alignment, so a transposition
  counts as one edit rather than two.

## JSON output

`--json` prints one JSON document to stdout instead of the text report, for `--health`,
`--security`, `--updates` and `--check <workflow>`. It can stand anywhere on the line
(`opi --health --json`); after a script name it still belongs to the script, and with any
other flag it changes nothing. Colour and progress are not printed, `--updates` lists and
never offers to apply, and the exit codes are those in the table below. Failures before
anything runs (unknown workflow, unreadable `.opi/checks.json`, no project) stay an error
on stderr with a non-zero exit and print no JSON.

Every document carries `schema` (currently `1`), `command`, `project` and a verdict.
New keys may be added within a schema version; a key is removed or changes meaning only
with a new `schema`.

**`--health`** — `{schema, command: "health", project, ok, checks}`. `ok` is true when no
check failed; a warning does not fail.

**`--check <workflow>`** — `{schema, command: "check", workflow, project, ready, gates, checks}`.
`gates` are the repository questions, `{name, status, detail}` with `status` one of
`passed`, `skipped`, `failed`. `ready` is false when a gate or a check failed.

A check, in `--health`, `--check` and as a `Secrets` section of `--security`:

```json
{ "name": "Lockfile", "tool": "pnpm", "scope": null, "status": "failed",
  "duration_ms": 412, "output": "…the tool's full output…" }
```

`scope` is the workspace member or `null` for the root. `status` is `passed`, `warning`,
`failed`, or `unavailable` when the tool could not be started. `output` is not truncated.

**`--security`** — `{schema, command: "security", project, ok, sections}`. `ok` is false
for a serious (critical or high) advisory, a signature that does not verify, a secret
finding or an audit that broke. Each section has `name` and `status` (`passed`, `warning`,
`failed`, `unavailable`, `skipped`; `message` says why for the last three) plus the
fields of its kind. Sections appear in the order of the text report, and `GitHub Actions`
and `Sources` only when they have something to report.

| Section | Fields |
| --- | --- |
| `Secrets` | a check, as above |
| `Dependencies`, `Dependencies (rust)`, `Dependencies (osv)` | `advisories`: `{package, severity, patched}`; Rust adds `version` and `id` and has no `severity`; osv adds `ecosystem` |
| `Signatures` | `invalid`, `missing`: `name@version` lists |
| `Release age` | `age`, `null` when not set |
| `Install scripts` | `packages`: `{package, policy}`, policy `allowed`, `ignored` or `unreviewed` |
| `Licenses`, `Licenses (rust)` | `counts`: `{license, count}`; `notable`: `{license, package}` |
| `GitHub Actions` | `unpinned`: `{file, line, reference}` |
| `Sources` | `outside_registry`: `{package, source}` |

`severity` is `critical`, `high`, `moderate`, `low` or `info`.

**`--updates`** — `{schema, command: "updates", project, ok, ecosystems}`. `ok` is false
only when a listing failed. Each ecosystem (`npm`, `rust`) has `status` (`current`,
`outdated`, `unavailable`, `failed`), `updates`, and `message` for the last two. An update is
`{package, current, latest, jump, in_range}`, with `jump` one of `patch`, `minor`, `major`.

```sh
opi --check push --json | jq -r '.checks[] | select(.status == "failed") | .name'
```

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
| `opi --hooks` | Non-zero outside a git repository or when the hook cannot be written |
| `opi --checks [workflow]` | Non-zero without a terminal or when configuration cannot be read or saved |
| `opi --security` | Non-zero when something needs acting on |
| Unknown flag or script | `1` |

## Crate

`opi` is a binary crate without a library API, so there is no API reference on docs.rs — its
build there fails with `no library targets found`, which is what every binary-only crate gets and
no metadata can change. The manifest's `documentation` field therefore points at this site. The
package is on [crates.io/crates/opi](https://crates.io/crates/opi); the internals are described in
[Architecture](../../architecture/).
