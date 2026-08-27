# Split Verify Module

This change decomposes verification orchestration, task execution, parsing,
and tests while preserving the existing `crate::verify` boundary.

- ADR: [0010-verify-module-decomposition](../../../docs/adr/0010-verify-module-decomposition.md)
- Status: implementation and local verification complete; PR CI pending
