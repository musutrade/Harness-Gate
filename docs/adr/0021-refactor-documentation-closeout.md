# ADR 0021: Close Out Refactor Documentation After Merge

## Status

**Proposed** - 2026-08-27

## Context

The module-decomposition work is delivered through separate documentation and
implementation pull requests. After implementation is merged and the `main`
CI workflow passes, several OpenSpec change records can still describe the
implementation PR as pending. Those stale records obscure which work is
complete and which work remains subject to post-merge observation.

## Decision

Treat OpenSpec status, task checklists, and ADR status as part of the delivery
closeout. In a documentation-only closeout change, update records for merged
refactors to state that they are merged and verified by `main` CI. Mark an ADR
as Accepted when both its documentation and implementation pull requests have
merged. Keep time-bound post-merge observation tasks unchecked until their
stated observation window and acceptance criteria have actually completed.

## Consequences

- The documentation accurately distinguishes completed refactors from ongoing
  operational monitoring.
- Documentation-only closeout changes introduce no runtime behavior or
  production-source changes.
- Future refactors have an explicit, auditable final documentation step.

## Related

- [ADR-0007](0007-audit-module-decomposition.md)
- [ADR-0020](0020-audit-scanner-decomposition.md)
- [OpenSpec: close-refactor-documentation](../../openspec/changes/close-refactor-documentation/proposal.md)
