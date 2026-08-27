# Architecture Decision Records (ADR)

This directory contains Architecture Decision Records (ADRs) for the Harness-Gate project.

## What is an ADR?

An Architecture Decision Record (ADR) is a document that captures an important architectural decision made along with its context and consequences.

## Format

We use a simplified version of the Michael Nygard ADR template:

- **Title**: Short noun phrase
- **Status**: Proposed, Accepted, Deprecated, or Superseded
- **Context**: What is the issue that we're seeing that is motivating this decision?
- **Decision**: What is the change that we're proposing and/or doing?
- **Consequences**: What becomes easier or more difficult to do because of this change?

## Index

- [ADR-0001](0001-use-rust-for-cli.md) - Use Rust for CLI implementation
- [ADR-0002](0002-optimize-release-builds.md) - Optimize release builds with LTO and strip
- [ADR-0003](0003-enhance-ci-pipeline.md) - Enhance CI pipeline with security and coverage checks
- [ADR-0004](0004-add-integration-tests.md) - Add integration tests for end-to-end scenarios
- [ADR-0005](0005-phase-2-optimization-strategy.md) - Phase 2 optimization strategy (performance and code quality)
- [ADR-0006](0006-test-fixtures-and-internal-boundaries.md) - Test fixtures and internal workflow boundaries
- [ADR-0007](0007-audit-module-decomposition.md) - Decompose the audit module by responsibility
- [ADR-0008](0008-config-module-decomposition.md) - Decompose the workflow configuration module by responsibility
- [ADR-0009](0009-secrets-module-decomposition.md) - Decompose the secret scanner by responsibility
- [ADR-0010](0010-verify-module-decomposition.md) - Decompose the verification module by responsibility
- [ADR-0011](0011-service-module-decomposition.md) - Decompose the service module by responsibility
- [ADR-0012](0012-process-module-decomposition.md) - Decompose the process module by responsibility
- [ADR-0013](0013-doctor-module-decomposition.md) - Decompose the doctor module by responsibility
- [ADR-0014](0014-preset-module-decomposition.md) - Decompose the preset module by responsibility
- [ADR-0015](0015-main-module-decomposition.md) - Decompose the CLI entry module by responsibility
- [ADR-0016](0016-project-module-decomposition.md) - Decompose the project module by responsibility
- [ADR-0017](0017-scope-module-decomposition.md) - Decompose the scope module by responsibility
- [ADR-0018](0018-audit-boundary-decomposition.md) - Complete the audit boundary decomposition

## Creating New ADRs

When making a significant architectural decision:

1. Copy the template from an existing ADR
2. Number it sequentially (e.g., `0005-your-decision.md`)
3. Fill in the sections
4. Submit as part of your PR
5. Update this README index

## Superseding ADRs

When an ADR is superseded:
1. Update the status to "Superseded by ADR-XXXX"
2. Create the new ADR that supersedes it
3. Link between the two documents
