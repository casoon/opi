# Contributing

## Before you start

`opi` is pre-release and its scope is deliberately narrow. Read
[docs/constraints.md](docs/constraints.md) and [docs/decisions.md](docs/decisions.md)
first — most feature ideas are already answered there, usually by a decision not
to build them.

Three rules account for the majority of rejected changes:

- **Zero configuration.** A feature that only works once the user configures
  something has the wrong default.
- **No task system.** `opi` does not define tasks with their own commands.
- **Orchestration, not reimplementation.** Checks call established tools and
  parse their output.

## Local validation

Run the full suite before opening a pull request:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo package --locked
```

CI runs the same commands, plus `cargo doc` with warnings denied and the test
suite on the minimum supported Rust version.

## Terminal output

All user-facing output goes through [runemark](https://github.com/casoon/runemark).
Do not emit raw ANSI escapes or hand-rolled colour — it breaks the colour policy,
`NO_COLOR` compliance and piped output in ways that are easy to miss locally.

## Things worth checking manually

Behaviour that is easy to break and not fully covered by tests:

- no `package.json` in the directory
- an empty `scripts` object
- output piped to another process (no TTY — nothing may block on input)
- `NO_COLOR` set
- aborting an interactive selection with `Esc`, `Ctrl-C` and on panic — the
  terminal must be restored in all three cases

## Commits and releases

Keep [CHANGELOG.md](CHANGELOG.md) current under `## [Unreleased]`.

Releases are published by the release workflow through crates.io trusted
publishing, so no registry token is stored anywhere. The publish job runs in
the `crates-io` environment and waits for a maintainer's approval, so releasing
stays a deliberate step rather than a side effect of pushing a tag:

1. Bump the version in `Cargo.toml` and date the release in `CHANGELOG.md`.
2. Run the gates above, and check the tarball with `cargo package --locked`.
3. Push the tag `v<version>`. The workflow verifies the tag matches the crate
   version and builds the binaries.
4. Approve the `Publish to crates.io` job. It publishes, and the GitHub
   release is created after it.
