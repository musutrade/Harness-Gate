# ADR-0031: Harden Gate Boundaries and Delivery Contracts

## Status

**Proposed** (2026-08-30)

## Context

The review of the v0.3.1 gate found four classes of operational risk: working-
tree scanners followed file symlinks outside the repository; non-Unix process
termination only addressed a direct child; audit and scope hot paths repeated
compilation and traversal; and the committed CLI contract and remediation
text had drifted from the shipped binary. These failures affect security,
developer interruption, CI reliability, and large-repository throughput.

## Decision

- Treat file symlinks as an explicit scanner boundary and never read a target
  outside the canonical project root.
- Preserve Unix process groups and use a Windows Job Object for descendant
  ownership. Built-in scans must observe the shared cancellation state.
- Compile reusable glob and regex matchers at configuration or rule setup, and
  attach a stable rule identity to every violation.
- Resolve Doctor repository-relative paths from `Project::root`, surface service
  cleanup failures, and keep parser read failures distinct from zero results.
- Keep `harness-gate` as the only supported command name in source, docs,
  installer, and snapshots.

This remains a private binary architecture. New gate kinds, VCS providers, or
service runtimes still require a reviewed Rust change rather than a plugin API.

## Consequences

- Scans may skip symlink files that were previously read, which is safer and
  makes the repository boundary explicit.
- Windows builds gain a platform-specific process ownership implementation and
  require platform CI coverage.
- Configuration setup does more work once and less work per changed path/file.
- Existing report fields remain compatible, but rule attribution becomes
  deterministic for names that share prefixes.

## Alternatives Considered

- Canonicalize and read every symlink target: rejected because it still allows
  a repository entry to read arbitrary external files.
- Kill only the direct child on Windows: rejected because descendants can keep
  ports, locks, and inherited pipes alive.
- Add a public plugin trait now: rejected by ADR-0030 and the current binary
  distribution contract; this change is a hardening patch, not an API release.

## Related

- [OpenSpec: Harden Gate Boundaries and Delivery Contracts](../../openspec/changes/harden-gate-review-findings/proposal.md)
- [ADR-0025: Phase 1 Quality Baseline Gates](0025-phase-1-quality-baseline-gates.md)
- [ADR-0028: Dependency-Aware Parallel Scheduling](0028-parallel-scheduling.md)
