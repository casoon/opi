---
title: Installation
description: Install the release binary with the install script, or build it from crates.io.
order: 1
---

## Install script

```sh
curl -fsSL https://raw.githubusercontent.com/casoon/opi/main/install.sh | sh
```

The script downloads the release binary for your platform, checks it against the published
SHA-256 and puts it in `~/.local/bin`. Nothing else is touched.

This exists because `cargo install opi` builds the binary on your machine, and needing a Rust
toolchain is a poor requirement for a tool whose point is that you can use it in any
repository. The binaries come out of CI; publishing the crate stays local.

| Variable | Default | Purpose |
| --- | --- | --- |
| `OPI_INSTALL_DIR` | `$HOME/.local/bin` | Where the binary goes |
| `OPI_VERSION` | latest | Version to install, without the leading `v` |

Release binaries exist for macOS on Apple silicon and Intel, and for Linux on x86-64 and
arm64. The Linux binaries are static musl builds, so the distribution does not matter.

## Verify a download

Official binaries come only from the
[GitHub releases of `casoon/opi`](https://github.com/casoon/opi/releases), built there by the
release workflow. Check the hash, the signed build provenance and the attested SBOM:

```sh
shasum -a 256 -c opi-<version>-<target>.tar.gz.sha256
gh attestation verify opi-<version>-<target>.tar.gz --repo casoon/opi
gh attestation verify opi-<version>-<target>.tar.gz --repo casoon/opi \
  --predicate-type https://cyclonedx.org/bom
```

Each release also carries the SBOM itself, `opi-v<version>.cdx.json`, in CycloneDX. The
binaries are built with `cargo auditable`, so the dependency list sits inside them:

```sh
cargo audit bin opi
```

## With cargo-binstall

```sh
cargo binstall opi
```

[cargo-binstall](https://github.com/cargo-bins/cargo-binstall) finds the same release binary
on GitHub and puts it in `~/.cargo/bin`, without building anything. It does not check the
published SHA-256 — the install script does, or verify the download by hand as above.

## From crates.io

```sh
cargo install opi
```

This builds `opi` locally. It needs Rust `1.85` or newer.

## Unix only

`opi` runs a script by replacing its own process with `exec`, so `Ctrl-C` reaches the dev
server rather than killing a wrapper, and its interactive list drives termios directly.
Windows offers neither. Building on Windows fails with an explicit message rather than
producing a degraded binary — see [Constraints](../../constraints/).

## Another `opi`

`opi` is unrelated to [OdradekAI/opi](https://github.com/OdradekAI/opi), which owns the
`opi-*` crate namespace on crates.io. Installing both puts two `opi` binaries in
`~/.cargo/bin`.

## Check it

```sh
opi --version
```
