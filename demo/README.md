# demo

The recordings in the README, and the fake projects they are recorded in.

```bash
./demo/record.sh              # every scene
./demo/record.sh health       # one of them
```

Two kinds of scene, because no single tool covers both yet:

- **`casts/*.terminal.yaml`** — rendered with
  [castwright](https://github.com/casoon/castwright) to an animated SVG. Its
  `exec:` step runs the real command in a pseudo-terminal and records what it
  printed, so the output is `opi`'s own. Needs `castwright` on `PATH`
  (`npm i -g @casoon/castwright node-pty`; if `exec` fails with
  "posix_spawnp failed", `chmod +x` the `spawn-helper` the error names). Under
  Volta, global packages are isolated from each other and castwright cannot
  find `node-pty`; install both into one directory instead, with
  `"allowScripts": {"node-pty": true}` in its `package.json` since npm 11
  runs no install scripts otherwise, and put its `node_modules/.bin` on `PATH`).
- **`tapes/*.tape`** — the scenes that press keys *inside* the running
  interface (tab row, search, `Enter` on a row), which castwright cannot do
  yet. They need [vhs](https://github.com/charmbracelet/vhs) 0.12.1 or newer
  (`brew install vhs`); 0.12.0 wrote no frames with Chrome 154 while still
  exiting 0. `record.sh` skips them when `vhs` is not installed and fails when
  the GIF was not written.

The script builds `opi` in release mode, copies the fixtures to `/tmp/opi-demo`,
puts the fresh binary first on `PATH` so the scenes can call a bare `opi`, and
writes to `assets/`. The casts name `/tmp/opi-demo` in their `cwd:`, so
`OPI_DEMO_DIR` only moves the tapes.

## Why the fixtures are copied out

`opi` walks up from the working directory to find a project. A fixture sitting
inside this repository therefore finds *this* repository's `Cargo.toml` and
lists cargo tasks that are not its own. Recording from a copy outside the tree
is the fix.

## The fixtures

| | |
| --- | --- |
| `casoon-web` | A pnpm workspace — 23 entries in 10 groups across 3 packages, which is what puts the groups into a tab row |
| `pulse` | `package.json` and `Cargo.toml` in one repository, and a real crate, so `--health` runs real checks |

`pulse/src/report.rs` is left unformatted on purpose: the health recording
needs one check to fail, and a failure the tool actually found is worth more
than a staged one.

`pulse` also pins `lodash` 4.17.20 for the same reason: it has a published
advisory and a newer release, so `--security` and `--updates` each have a real
finding. `--updates` compares against what is installed, so `record.sh` runs
`npm ci` in the copy — the only step that needs the network besides the audit.

For the push scene `record.sh` turns the `pulse` copy into a git repository
with a bare upstream beside it and one commit not yet pushed, so the hook has
something to guard. The same unformatted `report.rs` is what stops the push.

The npm scripts in both fixtures point at small shell scripts under `bin/`
that print what the real tool would print. Nothing about `opi`'s own output is
faked — only the projects are.

## The scenes

| | |
| --- | --- |
| `casts/health.terminal.yaml` | Every check the project's tools can answer |
| `casts/security.terminal.yaml` | The dependency audit, with the fixture's old lodash as the finding, and the supply-chain lines |
| `casts/updates.terminal.yaml` | The same lodash as an update that is safe to take |
| `casts/direct.terminal.yaml` | The command line, without the interface |
| `casts/push.terminal.yaml` | `opi --hooks`, then a `git push` the hook refuses |
| `tapes/hero.tape` | The start screen, the tab row, one search across every group, running a script |
| `tapes/ecosystems.tape` | npm and cargo in a single list |

`config.tape` holds the shared look. Each tape sources it **after** its own
`Output`, `Set Width` and `Set Height`: VHS silently ignores a `Set` that comes
after the first command, and the prompt setup at the bottom of `config.tape`
is one.
