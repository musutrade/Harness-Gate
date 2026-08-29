# Tasks: Fix the docs.rs Build Target

**Parent:** [proposal.md](proposal.md) and
[ADR-0030](../../../docs/adr/0030-docs-rs-library-target.md)

- [x] **1.1 (P0, S)** Add a documentation-only `src/lib.rs` target.
  **Acceptance:** `cargo doc --lib --no-deps` succeeds and no binary modules
  are re-exported.
- [x] **1.2 (P1, S)** Record the docs.rs packaging decision in ADR-0030 and
  the OpenSpec change.
  **Acceptance:** The ADR index and proposal link the decision and explain the
  required patch release.
- [x] **1.3 (P0, S)** Run package, format, lint, test, and strict OpenSpec
  validation checks.
  **Acceptance:** Package/docs, formatter, Clippy, and strict OpenSpec checks
  pass; the complete test suite passes in a clean worktree.
- [ ] **1.4 (P0, S)** Create a PR and verify its required CI checks.
  **Acceptance:** The PR is green; publish a patch release after merge so docs.rs
  can build the fixed package.
