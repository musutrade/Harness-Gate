# Proposal: Release Harness-Gate 0.3.6

**Status:** Proposed
**Date:** 2026-09-02

## Why

The previous published version is 0.3.5. The current source contains the
reviewed post-remediation implementation for selected R-13 through R-18
tracks, but the public documentation still exposed the configuration
reference only in Chinese. This release records the implementation and
publishes a complete English configuration and JSON Schema reference.

## Goals

- Publish package metadata, CLI output, generated contracts, snapshots,
  changelog, and user-facing installation examples as version 0.3.6.
- Make docs/configuration.md the English schema v2 reference and retain the
  existing Chinese content in docs/configuration.zh-CN.md.
- Add schema/README.md as an English catalog for every shipped JSON Schema.
- Keep the release workflow protected by exact main CI, immutable tags,
  signed inventory, provenance, and the release environment.
- Preserve explicit boundaries for work that is not part of this release.

## Non-goals and open boundaries

- No DevRail business-code or policy change.
- No claim of DevRail staging G-03/G-04 acceptance, shadow/canary approval,
  or rollback authority.
- No OS-level network, filesystem, resource, or complete descendant sandbox
  for adapters; R-07 remains a separate future decision.
- No claim that the post-remediation umbrella ADR/OpenSpec is complete while
  cross-platform and other unchecked acceptance evidence remains open.

## Release contents

- R-13 webhook host/address policy and redacted destination diagnostics.
- R-14 through R-17 lifecycle, typed failure, scheduler, runner, and
  configuration validation hardening present in the source tree.
- R-18 repository and quality governance files and checks.
- English and Chinese configuration references plus the JSON Schema catalog.

## Acceptance

Before publication, the release PR must pass locked tests, formatting,
strict Clippy, dependency audit, schema generation, documentation
consistency, quality-script tests, release tests, and package validation.
After merge, tag v0.3.6 only on the exact protected main commit and approve
the protected release environment only after the release policy and build
matrix pass. Record the final PR, commit, workflow, release URL, asset
inventory, and crate URL here before marking this record Implemented.

## Related records

- ADR-0038: Plan Post-Remediation Security, Reliability, and Maintenance Follow-ups
- ADR-0035: Protect Release Eligibility and Publication
- Release governance: docs/release-governance.md
- Post-remediation hardening: ../post-remediation-hardening/proposal.md
