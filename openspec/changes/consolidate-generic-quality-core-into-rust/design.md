# Design: Rust-authoritative Generic Quality Core

## Context

Harness-Gate has completed the language-agnostic architecture migration and a bounded real TypeScript/Angular second-ecosystem acceptance. The external protocol boundary is now credible: collectors can produce normalized evidence without owning delivery decisions.

However, several generic semantics currently live in Python under `tools/quality/`: `harness_evidence.py`, `project_model.py`, `policy_engine.py`, `policy_ratchet.py`, `cross_component.py`, and `project_report.py`. These modules define behavior that is product semantics rather than merely repository-local CI glue.

The consolidation must preserve behavior before improving implementation. The Python implementation is therefore treated as a compatibility oracle during migration, not as technical debt to delete first.

## Product-boundary decision

Every production-like Python quality module is assigned one category:

- **A — CI/dev tooling:** orchestration, hosted-CI glue, documentation checks, benchmarks, local reports. Remains Python unless there is a separate operational reason to change it.
- **B — Ecosystem adapter:** tool parsing, source-map handling, native-output normalization, fixture collection. May remain Python or any other language behind the collector protocol.
- **C — Generic product semantics:** evidence/project/policy/ratchet/relationship/report rules that decide or define generic outcomes. Migrates into Rust.
- **D — Migration/reference tooling:** equivalence replays, certification/advisory helpers and one-time migration utilities. Retained while useful, then frozen or retired.

The architectural rule is:

> Collectors may be written in any language. Harness-Gate's authoritative generic decision semantics live in the released Rust core.

## Rust module boundaries

The Rust implementation should mirror domain boundaries, not Python filenames. A likely decomposition is:

```text
harness-gate core
├── quality::evidence
│   ├── schema/model validation
│   ├── capability/value types
│   ├── artifact/source integrity
│   └── series compatibility
├── quality::project
│   ├── project/component/subject model
│   ├── source boundaries
│   └── relationships
├── quality::policy
│   ├── typed comparisons
│   ├── rule validation
│   ├── subject selection
│   ├── gate results
│   └── aggregate requiredness
├── quality::ratchet
│   ├── baseline compatibility
│   ├── lineage
│   ├── debt classification
│   └── exception review
├── quality::contracts
│   └── cross-component relationship validation
└── quality::report
    ├── project/component indexes
    ├── local/cross-component aggregates
    └── machine report serialization
```

Exact Rust paths may differ after implementation review, but generic semantics must not be split by ecosystem language.

## Contract strategy

The migration is contract-first. Existing accepted JSON schemas and payloads are the external compatibility surface. Rust must deserialize, validate and serialize those contracts without narrowing accepted behavior or silently widening invalid input.

The initial migration should consume the existing schema fixtures and golden records. Schema changes are not bundled into implementation convenience refactors. If Rust exposes a real ambiguity or an unrepresentable invariant, stop that subtask and propose a separate reviewed spec delta with Python/Rust/TypeScript evidence.

## Differential migration

Each C-class capability moves in stages:

1. Freeze representative positive and negative fixtures from both real ecosystems.
2. Implement a Rust candidate behind a non-authoritative CLI/library entry point.
3. Run Python reference and Rust candidate over byte-identical inputs.
4. Compare canonicalized machine outputs field-by-field for the defined compatibility surface.
5. Investigate every mismatch; no allowlist may convert an unexplained semantic mismatch into success.
6. Only after the slice reaches zero unexplained mismatches may downstream generic semantics depend on the Rust implementation.

Do not recollect expensive native coverage/build evidence merely to test the Rust core. Reuse retained normalized/raw artifacts wherever semantics permit.

## Equivalence surface

Equivalence includes more than pass/fail. At minimum compare:

- accepted/rejected input and measurement-error reason class;
- capability state mapping;
- typed numeric/ratio/boolean comparison outcomes;
- selected subjects and relationships;
- gate state and requiredness;
- blockers and aggregate state;
- base/head series compatibility;
- lineage/move/rename treatment;
- debt state and trend;
- exception-review state;
- evidence/artifact links;
- cross-component contract outcome;
- project/component/local/cross-component aggregates;
- report indexes and stable identifiers where contractually defined.

Human wording may be normalized where it is explicitly non-contractual; machine semantics may not.

## Authority model during migration

The existing required Rust CI remains authoritative. Python generic semantics act as the reference for the new generic core until acceptance. Rust candidate output is shadow evidence.

Authority transfer occurs only after:

- Rust corpus differential acceptance;
- TypeScript/Angular corpus differential acceptance;
- complete negative matrix acceptance;
- hosted CI evidence;
- explicit documentation of rollback;
- proof that current required check identity and semantics are unchanged.

After transfer, the Rust core is authoritative for generic semantics. Python C-class modules are either removed, converted into thin compatibility wrappers invoking Rust, or frozen as test/reference implementations that cannot approve release decisions.

## Adapter boundary

No ecosystem parser is pulled into the Rust core solely because the core is Rust. TypeScript/Angular collection remains external. Future Python/Java/other adapters can do the same.

The collector protocol remains the process boundary:

```text
native tools -> adapter/collector -> normalized evidence -> Rust core -> gate/project result
```

This preserves language independence without creating a multi-language decision core.

## CLI and library exposure

Prefer Rust library APIs for evidence/project/policy evaluation, with a narrow CLI surface for differential replay and future external integration. The CLI must consume explicit project/policy/evidence/base/selection/artifact/source context and must not infer trusted context from collector-controlled data.

Do not expose a second loosely specified JSON API during migration. Reuse the existing versioned contracts.

## Failure and rollback

The migration is fail-closed:

- parser/validation disagreements block migration;
- missing base context cannot become pass;
- unsupported/not-collected/measurement-error states remain distinct;
- series incompatibility blocks incremental comparison;
- relationship/provenance failures remain blockers;
- invalid exception metadata remains a measurement/configuration error.

Rollback before authority transfer is trivial: disable the Rust shadow path. After authority transfer, rollback may restore the previous release while retaining all differential evidence and Python reference fixtures. Do not delete accepted evidence or baselines as part of rollback.

## Testing strategy

Testing is layered:

- Rust unit tests for typed values, validators, selection, aggregation and ratchet rules;
- schema/golden fixtures shared with Python;
- differential tests against retained Rust evidence;
- differential tests against retained TypeScript/Angular evidence;
- negative mutation/tamper/series/identity/exception/contract cases;
- hosted CI shadow job before transfer;
- full existing Rust regression suite and Python quality suite throughout migration.

## Python dispositions after migration

Expected disposition, subject to inventory evidence:

- `ci_quality.py`, `coverage.py`, `critical_paths*.py`, `contracts.py`, `benchmarks.py`, `docs_consistency.py`: remain A-class Python tooling.
- TypeScript/Angular collectors and native contract collectors: remain B-class adapters/tooling.
- `harness_evidence.py`, `project_model.py`, `policy_engine.py`, `policy_ratchet.py`, `cross_component.py`, `project_report.py`: C-class semantics migrate to Rust; afterward remove/freeze/wrap according to dependency inventory.
- `rust_equivalence.py`, `typescript_advisory.py` and migration-specific replayers: D-class; retain through acceptance, then document freeze/retirement policy.

The final inventory, not this provisional list, is authoritative.
