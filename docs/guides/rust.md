---
title: Cargo projects
description: Cargo.toml and package.json share one list, and every area answers for both.
order: 8
---

A repository can be several things at once, and `opi` does not make it choose.
`package.json` and `Cargo.toml` are both read, and their commands share one list. Measured
across 231 directories here, 133 carry a `package.json`, 51 a `Cargo.toml` and twelve both —
and 39 of those Cargo-only ones were turned away before this existed, with "No package.json
found":

```
llmux-dashboard  npm · cargo

Development
  cargo run     Build and run the binary
  dev
Build
  build
  cargo build   Build in release mode
  cargo check   Type-check without building
```

A Rust project defines no scripts, so its commands are a fixed set — the ones these
repositories run in CI. On `opi` itself:

```
opi  cargo
  7 entries · 3 groups

Development
  cargo run     Build and run the binary
Build
  cargo build   Build in release mode
  cargo check   Type-check without building
Quality
  cargo clippy  Lint with warnings denied
  cargo doc     Build and open the documentation
  cargo fmt     Format the source
  cargo test    Run the test suite

H Health   C Clean   S Security   U Updates
```

`cargo run` is offered only where something is runnable.

## Every area follows

- [Health](../health/) runs `cargo fmt --check`, `cargo clippy`, `cargo rustdoc` and
  `cargo test` alongside the npm checks, and `cargo metadata --locked` where `Cargo.lock` is
  committed. Where `cargo docs-rs` is installed, one more check builds the documentation the
  way docs.rs will.
- [Security](../security/) and [Updates](../updates/) answer for the crates through
  `cargo audit` and `cargo outdated`, where they are installed.
- [Clean](../clean/) lists `target/` with the build artefacts.

All three of `cargo docs-rs`, `cargo audit` and `cargo outdated` are external subcommands
rather than part of the toolchain. Where one is missing, `opi` says so along with the
`cargo install` that adds it, instead of leaving a section that reads like "nothing found" —
except `docs.rs`, which as a check is simply absent where it is not installed: a health run
lists what answered, not what could have.

### Why rustdoc is a health check

rustdoc renders a doc comment as HTML, so a bare `<iframe>` in one becomes a real element and
the page can end right there. docs.rs served a truncated page for one of these crates for
months before a docs.rs maintainer reported it, while `rustdoc::invalid_html_tags` had been
warning the whole time — simply not wired to anything that could fail. It runs with
`-D warnings` rather than that one lint, because a broken intra-doc link and a bare URL are
the same class of defect: documentation that is wrong where nobody looks.

`cargo rustdoc` rather than `cargo doc`, because the lint level has to reach the crate being
documented and nothing else. `cargo doc` would take it through `RUSTDOCFLAGS`, an environment
variable the check list cannot carry, while `cargo rustdoc` passes it as an argument — the
shape every other check already has. It documents the library target where there is one, the
same target a registry publishes, and the binary otherwise.

### Two feature sets, two runs

`rustdoc` runs a second time with `--all-features` where the manifest has a feature to turn
on. A link from an always compiled item to one behind a feature resolves in the one feature
set and not in the other, so checking a single set answers half the question. Where there is
no feature to turn on, both runs would compile the same crate and only one is offered.

### What `cargo docs-rs` adds

`Docs` asks whether a crate documents cleanly. `docs.rs` asks a different question: whether
the *registry* will render what the author meant. `cargo docs-rs` builds with the features,
targets and rustdoc arguments out of `[package.metadata.docs.rs]` — the metadata that decides
how the published pages look, and which nothing else in the check list reads.

### Packages no workspace flag reaches

A package below the root that belongs to no workspace, and that a path dependency ties to the
root, gets a test run of its own under its package name. Every workspace flag above misses it.

## Workspaces

In a Cargo workspace `opi` works on the workspace even when started inside one of its crates
— that is where `target/` lives and where the checks reach every member. Taking the nearest
`Cargo.toml` instead would miss the largest directory in the repository: 509 MB in the one
this was found on, which is the only reason anyone starts Clean. `cargo run` still
follows you: it is offered in a binary crate, because that is where cargo would start it.
