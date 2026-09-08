# ADR-0040: Language-agnostic evidence and policy with a fail-closed migration boundary

**Status:** Proposed for acceptance with GH-118; required Rust authority continues.

**Date:** 2026-09-08

**Scope:** OpenSpec `language-agnostic-evidence-policy-architecture`, tasks 10.1–10.4.

## Context

The Rust quality path already retains coverage, risk, source and critical-path
evidence. Future components and cross-component contracts need common identity,
provenance and policy interfaces without losing the meaning of those measurements.
The [OpenSpec design](../../openspec/changes/language-agnostic-evidence-policy-architecture/design.md)
and [ADR-0039](0039-required-risk-and-traceability-gates.md) establish the existing
required gate as the migration oracle.

## Decision

Rust is the **reference adapter**. Its retained candidate/raw artifacts feed the
versioned project/subject model, `harness-evidence/v1` envelope and generic policy
engine. Normalization retains native identities, raw counters, exact rational
CRAP, display values, source digests, capability limitations and debt.
Collectors measure and normalize facts; they do **not** decide final delivery
pass/fail. Harness-Gate policy owns thresholds, ratchets and aggregation.

Measurement series are **not globally interchangeable**. Language, tools, rules,
instrumentation, mapping, source identity, normalization and target are semantic
inputs to series identity. Equal metric names or similar numeric values do not
authorize baseline comparison. The accepted Phase 1 baseline retains its original
incompatible series; replaying later candidates does not accept a new baseline.

**Fail-closed behavior is the trust boundary.** Missing, modified, mixed, stale,
malformed or incompatible evidence cannot become success. Unsupported and absent
measurements remain explicit states without fabricated numeric values. Any
disagreement in outcome, debt, unsupported state or measurement-error semantics
blocks equivalence acceptance, even when both aggregates appear green.

The advisory `Generic Quality Shadow` job downloads the exact current run/attempt's
`ci_quality.py collect` artifact. It verifies the original base/head/run identity
and all retained artifact hashes, projects facts and evaluates without invoking
Rust collection. Its JSON outputs link back to that same retained evidence.
Failed or partial collection is still replayed where artifacts are available;
missing artifacts produce a compatibility failure. Shadow failures remain visible.

`Required Quality Aggregate` keeps its check name, event-specific dependencies,
`always()` execution and existing fail-closed evaluator. The current Rust path
remains the **sole release authority throughout GH-118 and after this PR**.
The [defined retrospective equivalence window](../quality/rust-equivalence-acceptance.md)
accepts representation/policy compatibility only. Replacing an authoritative child
gate requires a separate reviewed rollout; no branch-protection or release-policy
change is authorized here.

## Consequences

The shadow rollout adds projection/evaluation cost and JSON retention, without
another Rust test/coverage/risk/matrix collection. Operators can reproduce a
comparison offline using pinned archives and original identities. A failing
shadow run blocks migration acceptance but cannot alter the Rust release result.
Legacy coverage and critical-path stages are retained as authoritative stage
results, not independently reimplemented generic collectors.

The acceptance window covers two retained Linux Rust runs and explicit negative
fixtures. It does not certify future series, other platforms, hosted shadow
execution, or a second ecosystem. The separately scoped task 10.5 and real
TypeScript/Angular reference adapter remain pending; the whole OpenSpec change
is not declared accepted or stable.
