# Architecture

What exists today. `opi` lists a project's `package.json` scripts and runs the
one you pick. Health, updates, clean and security are not built.

## Modules

```
src/
├── main.rs      entry point, wiring, all user-facing output
├── cli.rs       argument parsing → Invocation
├── manifest.rs  package.json → Manifest
├── project.rs   Manifest + directory → Project (name, package manager)
├── task.rs      Manifest → Vec<Task>, grouped and ordered
└── run.rs       Task → the running script
```

## Flow

```mermaid
flowchart TD
  Args["std::env::args"] --> Cli["cli::parse"]
  Cli -->|"Invocation"| Main["main"]
  Manifest["manifest::Manifest::load\npackage.json"] --> Project["project::Project::detect"]
  Manifest --> Task["task::Task::from_manifest"]
  Main --> Manifest
  Project -->|"PackageManager"| Run["run::execute"]
  Task -->|"Vec&lt;Task&gt;"| Menu["runemark::Menu"]
  Task -->|"exact name match"| Run
  Menu -->|"Outcome::Selected"| Run
  Menu -->|"Outcome::Unavailable"| Render["Menu::render → stdout"]
  Run -->|"exec, replaces this process"| Script["pnpm run dev"]
```

`package.json` is parsed once, by `manifest.rs`, and the result is shared by
project detection and the task model. Neither re-reads the file.

## The task model is the single abstraction

`Task` is what the interactive list, the command line and — later — the search
all operate on. `opi dev` and selecting `dev` from the list reach
`run::execute` by different routes but with the same value.

This is deliberate: if the two ever need separate handling, the boundary has
been broken. Built-in actions and tool checks are meant to join `Task` rather
than arrive as a parallel type.

## Grouping

`task::Group` derives `Ord`, so group order is the order its variants are
declared — `Development`, `Build`, `Preview`, `Quality`, then unrecognised
prefixes alphabetically, then the catch-all. There is no separate sort
function to keep in step.

Two rules are less obvious than they look:

- A script with no prefix that heads a family joins that family. `deploy`
  belongs with `deploy:blog` and `deploy:starter`, not in the catch-all.
- Lifecycle hooks npm runs on its own (`prebuild` where `build` exists) are
  hidden. The check is guarded against recognised groups, because `preview`
  strips to `view`.

## Execution

`run::execute` replaces the process through `exec`. The script inherits the
terminal, the signals and the exit status, so `Ctrl-C` reaches a dev server
instead of killing a wrapper, and `opi build && …` works in a shell chain.
`opi` does not return to its list afterwards, because it no longer exists.

This is why `opi` is Unix-only — see [constraints.md](constraints.md).

npm is the only package manager given a `--` before forwarded arguments; it
needs the separator to tell its own flags from the script's and strips it,
while the others would pass a literal `--` through.

## Presentation

Every user-facing string comes from [runemark](https://github.com/casoon/runemark).

- The list is a `runemark::Menu`, built in `main::build_menu`. Item ids are
  script names, so a selection is ready to run.
- Interactive selection needs runemark's `select` feature and both stdout and
  stderr to be terminals — stdout decides whether output is being captured,
  and the menu draws its frames on stderr.
- Where that does not hold, `Menu::render` prints the same layout as plain
  text. One menu definition serves both paths.
- Errors are `runemark::ErrorBlock` on stderr.
