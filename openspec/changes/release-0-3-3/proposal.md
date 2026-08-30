# Proposal: Release Harness-Gate 0.3.3

**Status:** In progress

## Why

The published README is also rendered by crates.io, where Cargo rewrites
relative links against the package root. Links to the language README, local
documentation, the license, and contribution guide therefore pointed at paths
that do not exist in the packaged crate. Several navigation anchors also did
not match their headings on GitHub. A patch release is required because
crates.io versions are immutable.

## Goals

- Make README navigation anchors resolve to existing headings.
- Use stable GitHub URLs for repository documentation from GitHub, crates.io,
  and docs.rs.
- Expand the documentation-only library target shown on docs.rs.
- Publish `harness-gate 0.3.3` and GitHub Release `v0.3.3`.

## Non-goals

- No public runtime library API.
- No changes to the CLI behavior or configuration schema.
- No removal of imports that are already proven used by strict Clippy and
  `cargo +nightly udeps`.

## Success Metrics

- `cargo package --locked --allow-dirty --no-verify` contains the corrected
  README and `src/lib.rs`.
- Format, strict Clippy, tests, CLI contracts, docs consistency, and strict
  OpenSpec validation pass.
- crates.io and docs.rs expose version `0.3.3` with the updated links.
- GitHub Release `v0.3.3` contains all platform binaries.

## Risk Assessment

**Risk: Low.** The release only changes documentation, package metadata, and
the version; the binary implementation remains unchanged.

## Related Records

- [ADR-0030: docs.rs library target](../../../docs/adr/0030-docs-rs-library-target.md)
- [ADR-0031: Harden gate boundaries](../../../docs/adr/0031-harden-gate-boundaries.md)
