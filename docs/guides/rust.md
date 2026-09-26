---
title: Cargo projects
description: Cargo.toml and package.json share one list, and every area answers for both.
order: 8
---

A repository can be several things at once, and `opi` does not make it choose.
`package.json` and `Cargo.toml` are both read, and their commands share one list:

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
  committed. `rustdoc` is there because a doc comment is
  rendered as HTML: a bare `<iframe>` in one becomes an element, and the page can end
  there. Where `cargo docs-rs` is installed, one more check builds the documentation the
  way docs.rs will — with the features, targets and rustdoc arguments out of
  `[package.metadata.docs.rs]`, which is what decides how the published pages look.
- [Security](../security/) and [Updates](../updates/) answer for the crates through
  `cargo audit` and `cargo outdated`, where they are installed.
- [Clean](../clean/) lists `target/` with the build artefacts.

`cargo docs-rs` is external too, but as a check it is simply absent where it is not
installed: a health run lists what answered, not what could have.

Both `cargo audit` and `cargo outdated` are external subcommands, not part of the toolchain.
Where one is missing, `opi` says so along with the `cargo install` that adds it, instead of
leaving a section that reads like "nothing found".

## Workspaces

In a Cargo workspace `opi` works on the workspace even when started inside one of its crates
— that is where `target/` lives and where the checks reach every member. `cargo run` still
follows you: it is offered in a binary crate, because that is where cargo would start it.
