# ADR-0041: Version the quality configuration plane independently of flow

**Status:** Proposed for GH-179; implementation and local validation recorded in the PR, hosted CI/controller acceptance pending.

**Date:** 2026-09-09

**Scope:** OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 1.1–1.4.

## Context

Flow v2 models execution. Generic quality needs stable component/subject identity,
measurement expectations and policy participation without embedding language
commands or giving collectors delivery authority. Existing flow-only repositories
must remain compatible. The [Engineering Policy](../engineering-policy.md) and
[ADR-0040](0040-language-agnostic-evidence-policy.md) constrain this addition.

## Decision

Define an independently versioned, closed `.harness-gate/quality.toml` v1 Rust model
and generated schema. Explicit file presence activates cross-plane config checks.
Named components map to flow components; profiles map to flow step profiles.
Collectors declare capabilities and exact measurement series. Policy bindings
reference existing `harness-policy/v1` rule documents, which remain the single
source of requiredness, limits and ratchet intent. Config validation reuses the
frozen strict parser and policy shape validator without evaluating thresholds.

Reject unresolved references, missing required producers, conflicting ownership,
incompatible series, escaping paths, inactive discovery dependencies and
baseline/ratchet contradictions. Export schemas without requiring project config;
keep existing flow export and print defaults. The
[configuration reference](../quality-configuration.md) specifies fields and limits.

## Consequences

The quality plane is language-neutral and policy authority stays outside
collectors. Existing presets and flow-only verification remain unchanged. Schema
sync and the executable quality example extend existing validation jobs, preserving
single-owner measurement and the lightweight Required Quality Aggregate.

This decision covers configuration only. Compilation, signed request orchestration,
loaded baseline trust, evidence evaluation and project verify/report integration
remain later [OpenSpec tasks](../../openspec/changes/integrate-generic-quality-into-project-workflow/tasks.md).
Configuration validation cannot certify a collector or replace the existing Rust
release gate. It does not amend CRAP, coverage, debt, ratchet, fail-closed or
measurement-series semantics.
