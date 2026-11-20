---
status: accepted
---

# Every direct dependency is pinned to an exact version

The Owner decided this on 2026-11-21 during Bootstrap, to settle a conflict between the Factory Kit and Rust convention.

## Context

In `Cargo.toml`, a bare version such as `"0.159.0"` is a caret range.
Rust convention for binaries is caret ranges plus a committed `Cargo.lock`.
The Factory Kit check dependency-pinning requires an exact version in every manifest.
Checked on 2026-11-21 on crates.io: `oxc_parser` 0.159.0 and `gix` 0.90.0 are pre-1.0 and change their APIs on minor releases.

## Decision

**Every direct dependency in every manifest uses an exact version, such as `=0.159.0` in `Cargo.toml`, `Cargo.lock` is committed, and CI builds with `--locked`.**
The npm packages that wrap the binaries pin their platform packages to the exact release version.

## Considered options

- **Caret ranges plus the lockfile.**
  Rejected: a lockfile refresh can pull a breaking minor release of a pre-1.0 dependency.

## Consequences

- Every upgrade is a deliberate change in a pull request.
- Upgrades take more manual work.
