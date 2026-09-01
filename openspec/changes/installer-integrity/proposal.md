# Proposal: Harden Installer Artifact Verification

**Status:** Proposed
**Date:** 2026-09-01

## Goals

- Bind binary and source installation to an explicit immutable version tag.
- Verify checksums and Sigstore issuer/workflow/tag identity before publication.
- Download into private temporary storage and atomically replace only a regular
  destination target.
- Remove mutable `latest` API parsing and `raw/main | bash` guidance.
- Add offline failure-path tests for HTTP, checksum, signature, and symlink
  failures.

## Non-goals

- Do not change release asset naming or the release inventory schema.
- Do not add an implicit auto-update or mutable `latest` channel.
- Do not claim complete OS-level protection against a same-user concurrent
  filesystem attacker.
- Do not require users to trust an unpinned installer script.

## Success Metrics

| Boundary | Success criterion |
| --- | --- |
| Version | Missing, malformed, or mutable-version input fails before download. |
| Integrity | Manifest and binary signatures plus SHA-256 are verified with exact workflow/tag identity. |
| Publication | Existing regular targets are atomically replaced only after verification; symlink paths are rejected. |
| Failure safety | HTTP, checksum, signature, target, and parent-path failures leave the prior target unchanged. |
| Documentation | Every recommended installer invocation references an immutable tag. |

## Risk Assessment

**Risk: Medium.** The script is a user-facing security boundary and must work
on Linux, macOS, and MSYS/Cygwin environments. Offline fixture tests cover the
critical decisions; a future real release verifies the exact published asset
set.

## Related Records

- [ADR-0036: Verify Installer Artifacts Before Atomic Installation](../../../docs/adr/0036-verify-installer-artifacts.md)
- [ADR-0035: Protect Release Eligibility and Publication](../../../docs/adr/0035-protect-release-eligibility.md)
- [Release governance operations](../../../docs/release-governance.md)
