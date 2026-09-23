# demo

The recordings in the README, and the fake projects they are recorded in.

```bash
./demo/record.sh              # every tape
./demo/record.sh hero         # one of them
```

> **Nothing has been recorded yet.** VHS 0.12 does not render on this machine —
> it drives a headless Chrome over ttyd, and with Chrome 154 the frame
> directory stays empty while VHS exits 0 without an error. The recordings will
> come from our own tool instead; the tapes below stay as the scripts for those
> scenes. See `plan/11-demo-aufnahmen.md`.

Needs [vhs](https://github.com/charmbracelet/vhs) (`brew install vhs`), which
brings `ttyd` and `ffmpeg` with it. The script builds `opi` in release mode,
copies the fixtures to `/tmp/opi-demo` (`OPI_DEMO_DIR` moves that), puts the
fresh binary first on `PATH` so the tapes can call a bare `opi`, and writes to
`assets/`.

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

The npm scripts in both fixtures point at small shell scripts under `bin/`
that print what the real tool would print. Nothing about `opi`'s own output is
faked — only the projects are.

## The tapes

| | |
| --- | --- |
| `hero.tape` | The start screen, the tab row, one search across every group, running a script |
| `ecosystems.tape` | npm and cargo in a single list |
| `health.tape` | Every check the project's tools can answer |
| `direct.tape` | The command line, without the interface |

`config.tape` holds the shared look. Each tape sources it **after** its own
`Output`, `Set Width` and `Set Height`: VHS silently ignores a `Set` that comes
after the first command, and the prompt setup at the bottom of `config.tape`
is one.
