# Security Policy

## Supported versions

Security fixes apply to the latest minor release of codeint.

| Version | Supported |
| ------- | --------- |
| Latest (`0.4.x`) | Yes |
| Older releases | No |

## Reporting a vulnerability

Do not open a public GitHub issue for a security vulnerability.
Instead, report it privately through [GitHub Security Advisories](https://github.com/avidianity/codeint/security).

Include:

- A clear description of the vulnerability and its impact.
- The affected codeint version and platform.
- Minimal steps or a sample input to reproduce the issue.

## Response timeline

Maintainers aim to acknowledge private reports within 3 business days and provide an initial assessment or fix plan within 7 business days.

## Scope

Security reports are especially welcome for:

- Sandbox escapes, resource exhaustion, or SHA-256 verification bypasses in the WebAssembly Plugin host (`crates/plugin-host`).
- Path traversal or unintended writes when scanning Git repositories or running `codeint init` and `codeint update`.
- Checksum verification or supply-chain issues in release installers and npm packages.
