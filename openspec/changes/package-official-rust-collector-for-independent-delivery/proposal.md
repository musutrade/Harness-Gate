# Independently delivered official Rust collector (GH-225)

## Why
GH-219/220/221 provide native inventory, production mapping and scoped file
classification, but the tools still require repository-relative scripts and a
pinned compiler overlay. GH-215 needs a reproducible consumer installation.

## What Changes
Introduce a separately versioned Linux x86_64 collector bundle, checked installation,
compatibility manifest and clean-environment acceptance. Reuse collector protocol
v1 and harness-evidence/v1; add only the delivery capability described in this
change. No existing protocol or measurement requirement is replaced.

## Goals
- Install and run without a Harness-Gate source checkout or ambient Python.
- Keep compiler, LLVM, runtime, dependencies and artifact provenance explicit.
- Deliver measurement facts to the released Rust Core without duplicate collection.
- Hand an immutable RC version to GH-215 for separately authorized integration.

## Non-goals
No registry, marketplace, new repository, multi-repository orchestration, mandatory
Python rewrite, additional platform certification, baseline acceptance, threshold
change, or Arc-Admin gate transfer. No implementation or publication in this PR.

## Impact and policy
All relevant Engineering Policy semantics remain unchanged; no normative policy
delta is proposed. Core owns final policy, requiredness, debt and ratchets.
Accepted Rust coverage/CRAP rules and historical evidence remain unchanged.
A Linux-only new collector does not remove existing required Core platform jobs.
Package authenticity does not establish runtime capture authenticity.

## Success Metrics
A clean supported host installs an exact signed version, captures a real fixture,
retains complete re-export inputs, and passes the delivery rejection matrix.
Low coverage remains valid measurement followed by a Core policy failure.
No missing prerequisite is counted as a passed acceptance test.
Record package bytes, cold/warm setup, capture/re-export wall time, disk peak and
producer launches; establish costs from measurements, not projected savings.

## Risks
High: compiler-private ABI and relocation can invalidate capture/series identity.
High: incomplete binary retention prevents reproducible native re-export.
Medium: bundled Python/toolchain dependency and license closure increases size.
Medium: independent tags need protected release eligibility and identity rules.
Low: keeping the source in this repository avoids a second maintenance workflow.

## Related contracts
- [Engineering Policy](../../../docs/engineering-policy.md)
- [Collector protocol](../../../docs/quality/collector-protocol.md)
- [Python retention](../../../docs/quality/python-retention.md)
- [Existing protocol requirements](../language-agnostic-evidence-policy-architecture/specs/collector-adapter-protocol/spec.md)
- [Native mapping](../rust-native-production-mapping/design.md)
- [Native classification](../rust-native-missing-file-classification/design.md)
- [ADR-0040](../../../docs/adr/0040-language-agnostic-evidence-policy.md)
- [ADR-0044](../../../docs/adr/0044-trusted-quality-baselines.md)
- [ADR-0049](../../../docs/adr/0049-project-owned-validation-extension-boundary.md)

Status: proposed; GH-225 remains open. See design.md for decisions and tasks.md
for unchecked implementation and review prerequisites.
