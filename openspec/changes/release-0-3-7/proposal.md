# Proposal: Release Harness-Gate 0.3.7

**Status:** Proposed
**Date:** 2026-09-03

## Why

The previous published version is 0.3.6. Since that tag, `main` has accepted
the post-remediation hardening umbrella (ADR-0038 and the companion OpenSpec),
recorded the R-07 adapter-isolation wording inventory and OS-sandbox decision
matrix, added reproducible R-16/R-17 benchmark and allocation evidence, and
stabilized the Windows lease heartbeat lifecycle test. This patch publishes
those synchronized records and the stabilized quality baseline.

## Goals

- Publish package metadata, CLI output, generated contracts, snapshots,
  changelog, project status, and user-facing installation examples as version
  0.3.7.
- Keep the release workflow protected by exact main CI, immutable tags,
  signed inventory, provenance, and the `release` environment.
- Preserve explicit boundaries for work that is not part of this release.

## Non-goals

- No runtime CLI behavior, configuration schema, public API, or trust-boundary
  changes beyond the accepted v0.3.6 implementation.
- No OS-level adapter sandbox, DevRail staging G-03/G-04 acceptance,
  shadow/canary approval, or rollback authority.
- No mutation or reuse of existing version tags.

## Release contents

- Post-remediation hardening OpenSpec and ADR-0038 accepted records.
- R-07 adapter isolation wording inventory and OS-sandbox decision matrix with
  documentation consistency fixtures.
- R-16 scheduler/wait scenario evidence and R-17 validation allocation
  evidence under `docs/benchmarks`.
- Windows lease heartbeat lifecycle test stabilization.
- Version, changelog, README, project status, and release documentation
  synchronization.

## Acceptance

Before publication, the release PR must pass locked tests, formatting,
strict Clippy, dependency audit, schema generation, documentation
consistency, quality-script tests, release tests, and package validation.
After merge, tag v0.3.7 only on the exact protected main commit and approve
the protected release environment only after the release policy and build
matrix pass. Record the final PR, commit, workflow, release URL, asset
inventory, and crate URL here before marking this record Implemented.

## Related records

- [ADR-0035: Protect Release Eligibility and Publication](../../../docs/adr/0035-protect-release-eligibility.md)
- [ADR-0038: Post-Remediation Hardening](../../../docs/adr/0038-post-remediation-hardening.md)
- [Post-remediation hardening OpenSpec](../post-remediation-hardening/proposal.md)
- [Release governance operations](../../../docs/release-governance.md)
