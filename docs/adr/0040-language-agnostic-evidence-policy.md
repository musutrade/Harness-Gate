# ADR-0040: Language-agnostic evidence and policy with a fail-closed migration boundary

**Status:** Accepted through GH-118 / merged PR #127 and successful required CI; required Rust authority continues. See the [closure ledger](../quality/architecture-closure.md).

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
execution, or a second ecosystem. GH-119 task 10.5 creates the separate
[TypeScript/Angular proposal](../../openspec/changes/typescript-angular-reference-adapter/proposal.md).
The adapter remains unimplemented and uncertified; final architecture closure
awaits GH-119 required CI and controller acceptance. Cross-ecosystem stability
requires real second-ecosystem evidence.

## TypeScript/Angular follow-up review (GH-136)

The [bounded acceptance review](../quality/gh-136/README.md) supersedes the
implementation status above for this independent adapter follow-up: tasks
1.1–4.2 have merged PR and required CI evidence, and task 4.3 review is recorded
with final CI/controller closure pending. Only the retained fixture, exact
measurement series and supported capabilities in the
[certification matrix](../quality/typescript-certification.md) are covered.
This adds no generic contract amendment and does not broaden this ADR’s Rust
equivalence window. Unmeasured frontend capabilities remain unavailable, and
existing Rust required release authority remains unchanged. Any future generic
amendment needs a separately reviewed spec delta and Rust/frontend validation;
authority migration needs a future independent accepted change.

## Generic-core consolidation freeze (GH-146)

The [Python product-boundary inventory](../quality/gh-146/python-boundary.md)
and [shared compatibility corpus](../../tools/quality/fixtures/generic-core/README.md)
freeze the migration oracle for OpenSpec `consolidate-generic-quality-core-into-rust`
tasks 1.1–1.2. Generic semantics still run in Python shadow/reference paths.
The existing Rust required checks and their release authority remain unchanged;
this record does not accept the later Rust implementation or authority transfer.

## Candidate Rust policy and ratchet implementation (GH-148)

The independent Rust candidate now implements policy validation, exact typed
comparison, scope selection, policy-owned requiredness and aggregation, baseline
lineage, debt/trend ratchets and exception review. The
[GH-148 compatibility evidence](../quality/gh-148/README.md) covers OpenSpec tasks
3.1–3.2 against the frozen Python oracle. Exception review cannot waive quality
failures. Cross-component provenance validation remains a separate task 4.1
boundary that fails closed without a validator. The CLI has no dependency on the
candidate library; existing required CI and release authority remain unchanged.
This stage does not accept reporting, replay, authority transfer or the full proposal.
