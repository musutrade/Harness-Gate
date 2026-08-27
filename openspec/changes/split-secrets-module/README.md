# Split Secrets Module

This change decomposes the secret scanner by responsibility while preserving
the existing `crate::secrets` boundary.

- ADR: [0009-secrets-module-decomposition](../../../docs/adr/0009-secrets-module-decomposition.md)
- Status: implementation and local verification complete; PR CI pending
