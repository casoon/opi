# Fixture provenance

Output the stub tools in `tests/stubs.rs` answer with. Recorded fixtures come
from real runs in throwaway projects (pinned old dependencies such as
`lodash@4.17.20`, `ms@2.0.0`, `minimist@1.2.0`, `typescript@5.4.5`,
`quick-xml =0.38.4`, `clap =3.2.25`, `serde =1.0.200`), recorded 2026-11-08 on
macOS. Only the absolute project path was replaced by `/project`; nothing else
is edited.

| Fixture | Source | Tool version | Command |
|---|---|---|---|
| `npm-audit.json` | recorded | npm 11.19.1 | `npm audit --json` |
| `npm-audit-clean.json` | recorded | npm 11.19.1 | `npm audit --json` (only `ms@2.1.3`) |
| `npm-outdated.json` | recorded | npm 11.19.1 | `npm outdated --json` |
| `pnpm-audit.json` | recorded | pnpm 10.26.0 | `pnpm audit --json` |
| `pnpm-outdated.json` | recorded | pnpm 10.26.0 | `pnpm outdated -r --json` |
| `pnpm-outdated-warn.json` | hand-written | derived from `pnpm-outdated.json` | a `WARN` line is put in front of the recorded JSON; pnpm 10.26.0 printed none in the recording |
| `cargo-audit.json` | recorded | cargo-audit 0.22.1 (cargo 1.99.1) | `cargo audit --json` |
| `cargo-outdated.jsonl` | recorded | cargo-outdated 0.19.0 (cargo 1.99.1) | `cargo outdated --workspace --root-deps-only --format json` (two-member workspace) |
| `osv-scanner.json` | recorded | osv-scanner 2.7.0 (osv-scalibr 0.5.3) | `osv-scanner scan --format json -r .` |

Not covered by any fixture: yarn and bun.
