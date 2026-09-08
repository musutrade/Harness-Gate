# Implementation Tasks

All tasks belong to [proposal](proposal.md), [design](design.md), and the acceptance delta under `specs/`. This change is a compatibility migration, not a blanket Python rewrite.

## 1. Freeze the product boundary and compatibility corpus

- [x] 1.1 [P1][M] Inventory every production-like `tools/quality/*.py` module and classify A/B/C/D; acceptance: record purpose, callers, CI role, release impact, authoritative/non-authoritative status, and final disposition.
- [x] 1.2 [P1][M] Freeze shared positive/negative compatibility fixtures from retained Rust and TypeScript/Angular evidence; acceptance: exact inputs, expected outputs, schema versions and canonicalization rules are reviewable without recollecting expensive native evidence.

Validation: [GH-146 boundary and evidence](../../../docs/quality/gh-146/README.md); all 31 production modules inventoried and all 33 frozen oracle cases replayed successfully. Required hosted CI remains pending before merge; tasks 2–7 are not accepted.

## 2. Move evidence and project semantics into Rust

- [ ] 2.1 [P1][L] Implement Rust `harness-evidence/v1` model/validation and typed value/capability/series integrity semantics; acceptance: shared valid fixtures pass, malformed/stale/tampered/unknown/incompatible fixtures fail with equivalent reason classes.
- [ ] 2.2 [P1][L] Implement Rust project/component/subject/source-boundary/relationship model validation; acceptance: subject identity, path containment, duplicate/unknown references and relationship semantics match the Python reference corpus.

## 3. Move policy and ratchet semantics into Rust

- [ ] 3.1 [P1][L] Implement Rust policy schema validation, typed comparison, scope selection, `GateResult`, requiredness and aggregate semantics; acceptance: all policy fixtures produce zero unexplained semantic mismatches with Python.
- [ ] 3.2 [P1][L] Implement Rust baseline compatibility, lineage, debt/trend ratchet and exception-review semantics; acceptance: legacy/new/improved debt, rename/move lineage, missing base, incompatible series and invalid exceptions match Python behavior.

## 4. Move cross-component and project reporting semantics into Rust

- [ ] 4.1 [P1][M] Implement Rust cross-component contract/relationship validation on normalized evidence; acceptance: green local gates plus breaking provider/consumer contract still blocks the project with equivalent provenance.
- [ ] 4.2 [P1][L] Implement Rust project reporting and indexes; acceptance: component/local/cross-component aggregates, gate tables, indexes, evidence links and stable contract fields match canonical Python output.

## 5. Differential acceptance

- [ ] 5.1 [P1][L] Add a non-authoritative Rust differential replay entry point and comparator; acceptance: it consumes explicit project/policy/evidence/base/selection/source/artifact context and reports field-level mismatches without recollecting native evidence.
- [ ] 5.2 [P1][M] Run retained Rust-corpus acceptance; acceptance: zero unexplained semantic mismatches across positive and negative cases, with all current Rust required gates unchanged.
- [ ] 5.3 [P1][M] Run retained TypeScript/Angular-corpus acceptance; acceptance: exact accepted counters and generic outcomes reproduce with zero unexplained mismatches, including source identity, unsupported states and contract failures.
- [ ] 5.4 [P1][M] Run the consolidated negative matrix; acceptance: missing/stale/tampered evidence, ambiguous identity, unsupported required capability, malformed value, incompatible series, missing base, invalid exception and provenance/contract errors remain fail-closed.

## 6. Transfer generic semantic authority

- [ ] 6.1 [P1][M] Integrate the Rust generic core into an opt-in shadow CI path; acceptance: existing `Required Quality Aggregate` name/dependencies/authority are unchanged and Rust/Python differential artifacts are retained.
- [ ] 6.2 [P1][L] Perform explicit authority-transfer acceptance; acceptance: Rust and TypeScript/Angular corpora plus negative matrix are green, hosted CI is green, rollback is documented, and no unresolved mismatch remains.
- [ ] 6.3 [P1][M] Route authoritative generic evaluation/reporting through Rust while preserving collector protocol compatibility; acceptance: external collectors still measure only, and final generic decisions no longer require Python C-class modules.

## 7. Demote Python generic semantics safely

- [ ] 7.1 [P1][M] Apply final dispositions to C-class Python modules; acceptance: each is removed, converted to a thin Rust wrapper, or frozen as non-authoritative reference code with no release-approval path.
- [ ] 7.2 [P1][S] Document A/B/D retention policy; acceptance: CI/dev tooling and ecosystem adapters remain intentionally Python where appropriate, and migration/reference tooling has an explicit freeze/retirement policy.
- [ ] 7.3 [P1][M] Update architecture/ADR/quality documentation and run final strict OpenSpec validation; acceptance: product boundary is unambiguous, release binary responsibility is documented, full required CI passes, and no claim implies new ecosystem certification.
