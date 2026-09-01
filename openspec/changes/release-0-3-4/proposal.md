# Proposal: Release Harness-Gate 0.3.4

**Status:** Proposed

## Why

Since the `0.3.3` release, Harness-Gate has added protected release eligibility,
explicit release inventories, checksums, CycloneDX SBOM generation, Sigstore
signatures, GitHub build provenance, and an installer that verifies immutable
artifacts before atomic replacement. These changes are already merged on
`main`, but they need a versioned package and a real tag-triggered release so
the publication and clean-environment consumer paths can be verified together.

## Goals

- Publish `harness-gate 0.3.4` to crates.io.
- Build and publish GitHub Release `v0.3.4` for all supported platform targets.
- Preserve the protected-main, immutable-tag, and protected-environment release
  controls while producing the complete inventory, checksum, SBOM, signature,
  and provenance set.
- Verify the published assets with the inventory verifier and in a clean
  environment, including the checked installer and its source-install path.
- Keep Cargo metadata, CLI output, changelog, project status, and documented
  release commands synchronized.

## Non-goals

- No new DevRail integration or real `.arc-flow` consumer behavior; those are
  tracked by G-03 and G-04.
- No implementation of R-06, R-07, R-09, R-11, or the P2/P3 quality debts.
- No mutation, replacement, or deletion of an existing release tag or release
  asset.

## Success Metrics

- The release PR and the subsequent protected `main` CI run pass all required
  checks, including `Required Quality Aggregate`.
- The immutable tag `v0.3.4` passes release-policy verification against the
  exact successful `main` commit and the release workflow completes build,
  quality, signing, attestation, and publication.
- Every inventory subject has the declared checksum, Sigstore signature and
  certificate, and GitHub provenance; the release page contains exactly the
  inventory upload set.
- `release_inventory.py verify` and the documented offline consumer checks
  detect no modification, omission, or extra asset in a clean environment.
- The published crate and binaries report version `0.3.4`.

## Risk Assessment

**Risk: Medium.** The code changes are already covered by CI and offline
fixtures, but this is the first real exercise of the protected publication,
keyless signing, attestation, and clean-environment verification path. Any
partial publication remains immutable and must be corrected with a new version.

## Related Records

- [ADR-0032: Harness-Gate capability contracts and the DevRail boundary](../../../docs/adr/0032-harness-gate-devrail-capability-contracts.md)
- [ADR-0035: Protect release eligibility and publication](../../../docs/adr/0035-protect-release-eligibility.md)
- [ADR-0036: Verify installer artifacts before atomic installation](../../../docs/adr/0036-verify-installer-artifacts.md)
- [Release governance procedure](../../../docs/release-governance.md)
