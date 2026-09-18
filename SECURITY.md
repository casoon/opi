# Security Policy

## Supported versions

`opi` is pre-release. Only the latest published version receives fixes.

## Reporting a vulnerability

Report security issues privately through
[GitHub Security Advisories](https://github.com/casoon/opi/security/advisories/new),
or by email to security@casoon.de. Please do not open a public issue for a
vulnerability.

Include the affected version, reproduction steps and the impact you observed.

## Scope notes

`opi` executes commands defined in a project's `package.json` and invokes
external tools. Two areas deserve particular attention in reports:

- **Command execution.** Anything that causes `opi` to run a command other than
  the one a project defines, or to run one without the user selecting it.
- **Destructive operations.** The clean feature removes files. Any path escaping
  the project directory — via `..`, an absolute path in the `opi` key, or a
  followed symlink — is a security issue, not a bug.
- **Secret disclosure.** Secret scan findings are reported by location, never by
  value. Any output that prints a discovered secret is a security issue.
