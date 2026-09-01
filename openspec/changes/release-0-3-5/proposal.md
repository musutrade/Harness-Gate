# Proposal: Release Harness-Gate 0.3.5

**Status:** Proposed

## Why

The immutable `v0.3.4` tag reached the protected publication job, but the
workflow's broad artifact glob also downloaded the retained release-policy
evidence. Exact inventory verification correctly failed closed before any
GitHub Release or crates.io publication. Because the tag cannot be moved or
retried with different source, a new patch release is required after narrowing
the artifact boundary.

## Goals

- Fix the release artifact download selection and add a regression contract for
  the policy-evidence boundary.
- Publish `harness-gate 0.3.5` to crates.io and GitHub Release `v0.3.5`.
- Preserve protected-main eligibility, immutable version tags, the `release`
  environment, checksums, SBOM, Sigstore signatures, and provenance.
- Verify the exact published inventory and the documented clean-environment
  installer flow.
- Keep Cargo metadata, CLI output, changelog, project status, and release
  documentation synchronized.

## Non-goals

- No mutation, replacement, deletion, or reuse of the existing `v0.3.4` tag.
- No runtime CLI behavior, configuration schema, or public API changes.
- No weakening of exact inventory verification or publication protections.

## Success Metrics

- The release PR and the subsequent protected `main` CI run pass, including
  `Required Quality Aggregate`.
- The immutable `v0.3.5` tag passes release-policy verification against the
  exact successful `main` commit.
- The release workflow completes policy, build, quality, signing, attestation,
  GitHub Release, and crates.io jobs successfully.
- The GitHub Release contains exactly the inventory upload set; all subjects
  have valid checksums, signatures, certificates, and provenance.
- The published crate and binaries report version `0.3.5`.

## Risk Assessment

**Risk: Medium.** The code change is narrow, but this is a second operational
exercise of the protected signing and publication path after the failed
`v0.3.4` attempt. The new workflow contract and fail-closed inventory checks
limit the chance of publishing an incomplete asset set.

## Related Records

- [Failed v0.3.4 release record](../release-0-3-4/proposal.md)
- [ADR-0035: Protect Release Eligibility and Publication](../../../docs/adr/0035-protect-release-eligibility.md)
- [ADR-0036: Verify Installer Artifacts Before Atomic Installation](../../../docs/adr/0036-verify-installer-artifacts.md)
- [Release governance operations](../../../docs/release-governance.md)
