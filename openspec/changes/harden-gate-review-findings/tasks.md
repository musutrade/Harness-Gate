# Tasks: Harden Gate Boundaries and Delivery Contracts

**Parent:** [proposal.md](proposal.md), [design.md], and
[ADR-0031](../../../docs/adr/0031-harden-gate-boundaries.md)
**Status:** Implemented; acceptance evidence reviewed against PR #46 and the
green CI run `33293667323`.

- [x] **1.1 (P0, S)** Add repository-boundary checks for working-tree secret and audit scans.
  **Acceptance:** outside symlink targets are never read; regression tests pass.
- [x] **1.2 (P0, M)** Implement cross-platform process-tree cancellation and built-in scan cancellation points.
  **Acceptance:** descendants terminate on timeout/interrupt; platform tests pass.
- [x] **1.3 (P1, S)** Fix audit rule identity, scope/allowlist compilation, and parser error handling.
  **Acceptance:** collision and performance regressions have focused tests.
- [x] **1.4 (P1, S)** Fix Doctor path resolution and service cleanup diagnostics.
  **Acceptance:** repository-relative checks work from subdirectories and cleanup failures are observable.
- [x] **1.5 (P1, S)** Synchronize CLI snapshots and user-facing installation/remediation text.
  **Acceptance:** contract, docs, and version checks pass.
- [x] **1.6 (P0, S)** Run full validation, update this task list, and verify required CI checks.
  **Acceptance:** all required PR checks are green.
