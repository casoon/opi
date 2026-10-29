---
title: Hooks and CI
description: Where opi runs besides the terminal — Git hooks, CI pipelines, deploy steps and anything else that reads an exit code.
order: 9
---

`opi` is built to be used by hand: you open the list, pick a script, press `H` for the
checks. Every one of those areas is also a plain command with a meaningful exit code,
so the same checks run wherever a command can run: a Git hook, a CI job, a deploy step,
a `Makefile`, an editor task or a coding agent's hook.

| Where | Command | What it guards |
| --- | --- | --- |
| By hand | `opi`, `opi --health`, `opi --security` | Nothing: you read the result |
| Pre-commit hook | `opi --check commit` | Staged secrets, whitespace errors, conflict markers, large new files, fast project checks |
| Pre-push hook | `opi --hooks` installs `opi --check push` | Everything a CI would run, plus the build |
| CI job | `opi --health` or `opi --check push` | The same checks as locally, on a clean checkout |
| Before a deploy | `opi --check deploy && opi --yes deploy` | Checks and build first, then the project's deploy script |
| Before a release | `opi --check release` | Clean tree, untagged version, every check, the build |
| Scheduled audit | `opi --security` | Secrets and known vulnerabilities in the lockfiles |
| SBOM step | `opi --sbom dist/` | CycloneDX documents per ecosystem |

The checks come from the project's own tools either way. `opi` adds no rules of its own
that would make a hook disagree with the terminal. The one exception is the bundled
nosecrets engine, which steps in when a project has no secret scanner.

## Without a terminal

Hooks and CI give `opi` no terminal, and it changes behaviour in a few deliberate ways:

- **No menu.** A bare `opi` prints its list as plain text instead of opening the
  interactive list. Pipe it into `grep` or keep it in a log.
- **No colour.** Colour is used only when both output streams are terminals and
  `NO_COLOR` is unset. A failed step still reads as failed in plain text: results carry a
  mark such as `✗` or `[FAIL]`, not only a colour.
- **No silent yes.** A script marked `"confirm": true` refuses to run without a terminal
  to ask in. Say `opi --yes <script>` when that is what the pipeline means. The
  protection exists for exactly the case where nobody is watching.
- **No configuration.** `opi --checks` needs a terminal and fails elsewhere without
  writing anything. Pick the checks locally and commit `.opi/checks.json`.

The exit codes are listed in the [command line reference](../../reference/cli/#exit-codes).
In short, every check command exits non-zero when something needs acting on, including
an audit that could not run at all.

## Sharing the selection

A workflow runs its [defaults](../workflows/) until you choose otherwise with
`opi --checks <workflow>`. The choice is stored in `.opi/checks.json`, and that file is
meant to be committed. `opi` keeps `.opi/.gitignore` so that `checks.json` is shared
while generated SBOMs stay ignored.

One file then decides what runs on every machine and every pipeline:

```json
{
  "workflows": {
    "commit": { "checks": ["Secrets", "Staged files", "Lint & format"], "build": false },
    "push": { "checks": ["Secrets", "Lint & format", "TypeScript", "Tests"], "build": true }
  }
}
```

A selected check the project cannot run fails the workflow instead of being skipped. A CI
image without the test runner therefore cannot pass on a selection that names `Tests`.
[Configuration](../configuration/#workflow-checks) has the details.

## Pre-commit

The commit workflow is built for this hook. It reads Git's index rather than the working
tree for the parts where that matters:

- **Secrets** come from the staged contents. A key that was staged and then deleted
  in the editor still blocks the commit, because the deleted copy is the one that would
  be committed.
- **Staged files** reports whitespace errors and conflict markers in added lines, and
  new files over 500 KiB, measured by their size in the index.
- **Lockfile, lint, format, types and docs** run as usual, on the working tree. Tests
  and dead-code checks are left out because they are too slow for every commit.

The hook changes nothing in the repository. It stages nothing and fixes nothing.

### Plain Git

```sh
printf '#!/bin/sh\nexec opi --check commit\n' > .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

That hook lives only in your clone. To share it, use one of the hook managers below.

![A plain pre-commit hook running opi --check commit: git commit is refused because a staged 800 KB export exceeds the 500 KiB limit for new files and rustfmt found a diff](https://raw.githubusercontent.com/casoon/opi/main/assets/commit.svg)

### Husky

Projects with a `.husky` directory commit their hooks:

```sh
echo "opi --check commit" > .husky/pre-commit
```

### Lefthook

```yaml
# lefthook.yml
pre-commit:
  commands:
    opi:
      run: opi --check commit
```

### pre-commit

The [pre-commit](https://pre-commit.com) framework runs local hooks with
`language: system`. `opi` decides which files matter, so tell the framework to call it
once instead of once per file:

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: opi
        name: opi --check commit
        entry: opi --check commit
        language: system
        pass_filenames: false
        always_run: true
```

Git runs hooks at the repository root. When the project lives in a subdirectory, step
into it first, for example `cd app && opi --check commit`.

In every variant `git commit --no-verify` skips the hook. That escape hatch is Git's own,
and `opi` does not try to close it.

## Pre-push

```sh
opi --hooks
```

installs `opi --check push` as the pre-push hook. It writes into `.husky/pre-push` when
the repository has a `.husky` directory, so every clone gets it, and into Git's hook
directory otherwise. Existing lines in the hook are kept. The
[Workflows](../workflows/#the-pre-push-hook) guide describes the gates.

![opi --hooks installing the pre-push hook, then git push running every check and the build and refusing the push because rustfmt found a diff](https://raw.githubusercontent.com/casoon/opi/main/assets/push.svg)

Push is the workflow for a repository without CI. It runs every check and the build, so
a red state cannot leave the machine by accident. With CI it shortens the loop:
the push that CI would reject is stopped before it leaves.

## CI

On a CI runner `opi` is one more binary. Install the project's dependencies first:
`opi` runs the project's own tools from `node_modules/.bin` and Cargo, and does not
install anything itself. Then install `opi` at a pinned version and call it.

```yaml
# .github/workflows/check.yml
name: check
on: [pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - uses: actions/setup-node@v5
        with:
          node-version: 22
      - run: npm ci
      - name: Install opi
        run: curl -fsSL https://raw.githubusercontent.com/casoon/opi/main/install.sh | sh
        env:
          OPI_VERSION: 0.13.0
      - run: ~/.local/bin/opi --health
```

The install script checks the download against the release's published SHA-256. The
Linux binaries are static musl builds, so they run on any distribution image. `cargo
install opi --locked` works as well but compiles for a while. For Rust projects, install
the toolchain with the `rustfmt` and `clippy` components so that the format and lint
checks can run.

Which command to call depends on what the job is for:

- **`opi --health`** runs every detected check and asks no questions about Git. That
  is the usual choice for a pull request.
- **`opi --check push`** runs the push selection from `.opi/checks.json`, so CI and the
  pre-push hook run exactly the same list. The "not behind upstream" gate is skipped on
  a CI checkout, which has no upstream branch. The working tree has to be clean, which a
  fresh checkout followed by a frozen install normally is.
- **`opi --security`** queries audit services over the network and exits non-zero when
  one of them cannot be reached. That makes it a good scheduled job and a poor required
  check on every pull request.

Do not run `opi --check release` in a pipeline triggered by the release tag. Its version
gate fails once `v<version>` exists, and in that pipeline it always does. Run the release
workflow before tagging, and the push or deploy workflow in the tag's pipeline.

## Before a deploy

```sh
opi --check deploy && opi --yes deploy
```

`opi --check deploy` runs the deploy selection and the build on a clean working tree, and
never runs a deploy script itself. The second command runs the project's own `deploy`
script. `--yes` answers its confirmation when the script asks for one, which a pipeline
has to say explicitly.

## Anything else

Everything above is a command with an exit code, so the same calls fit wherever one is
expected:

- a `Makefile` target or a `justfile` recipe, such as `check: ; opi --health`;
- an editor task that runs `opi --check commit` before you stage;
- a coding agent's hook that runs `opi --check commit` after it edits files, so the agent
  sees the same findings you would;
- a release script that writes `opi --sbom` next to its artefacts.

When a program rather than a person reads the result, rely on the exit code. The output
is written for people: findings name their file and line, but there is no JSON report.
