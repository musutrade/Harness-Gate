# Design: Project-Owned Validation Boundary and Arc-Admin Dogfood

## 1. Durable architecture boundary

Harness-Gate is a gate orchestration, evidence ingestion and quality-decision platform. It is not an application test framework.

The product SHALL expose three generic extension layers:

### 1.1 Command hook / execution gate

Use for project-owned validations whose primary contract is execution success/failure plus optional structured output. Examples include:

- unit/integration/API/E2E/smoke/load/migration tests;
- code generation consistency;
- schema/deployment/observability/supply-chain validation;
- project-specific scripts and organization-specific checks.

Harness-Gate owns only generic execution semantics: selection/scope, profiles, dependencies, services, environment, timeout, retry, requiredness, logs/artifacts and report linkage. The project owns the executable/test code and its domain-specific meaning.

### 1.2 Structured result adapter

A command gate MAY attach a declared structured-result parser/adapter. Generic reusable formats such as JUnit, SARIF or versioned JSON contracts may be supported. Parsed details enrich diagnostics and machine reporting but SHALL NOT silently turn tool-native severity or test metadata into Harness-Gate policy authority unless an explicit generic policy contract governs that field.

### 1.3 Quality collector plugin

Use when measurements need generic cross-run/project policy such as coverage, complexity, CRAP inputs, bundle size or performance series. Collectors produce normalized evidence under existing trust/identity rules. The released Rust core owns requiredness, thresholds, baseline/ratchet/debt and final generic quality decisions.

## 2. Anti-specialization rule

Adding a new project test runner or application validation framework SHALL NOT require a Harness-Gate Core code change if it can already be represented as a command hook.

Examples:

```text
Playwright E2E     -> command hook
Cypress E2E        -> command hook
Postman/Newman API -> command hook
Go integration     -> command hook
pytest API         -> command hook
k6 load smoke      -> command hook
custom shell test  -> command hook
```

Harness-Gate-specific support is appropriate only for reusable protocol surfaces: structured result formats, collector protocols, ecosystem/capability packs, service orchestration or policy semantics.

## 3. Arc-Admin baseline/oracle

Arc-Admin currently owns a mature `.arc-flow/flow.toml` and `cargo flow` binary. Before migration, freeze:

- all required step IDs;
- component/scope mappings;
- hook/full behavior;
- PostgreSQL service requirements;
- environment removals/injections;
- parsers/timeouts;
- CI job routing and self-hosted execution;
- pre-commit and delivery expectations;
- existing project-specific gate implementations.

The frozen inventory is the assurance oracle. A Harness-Gate migration may be stricter, but it must not omit an existing blocker without explicit reviewed rationale.

## 4. Migration/import strategy

Arc-Admin's execution-plane config is structurally close to Harness-Gate flow schema. The dogfood SHALL prefer a deterministic import/migration mechanism over hand-copying steps.

The migration result must preserve semantic identity where possible:

```text
.arc-flow/flow.toml
       |
       +-- aliases/services/parsers/scope/steps/profiles
       v
.harness-gate/flow.toml
```

Any unsupported field or behavior must be reported explicitly. The importer SHALL NOT silently drop required steps or coerce unsupported semantics into defaults.

Manual edits after import should be limited to documented incompatibilities or intentional Harness-Gate additions. Migration effort is a product metric and must be recorded.

## 5. Quality configuration

Add `.harness-gate/quality.toml` separately from migrated execution config.

Arc-Admin should model at least:

- `frontend` Angular/TypeScript component;
- `backend` Rust component;
- relevant frontend/backend API relationship/contract;
- certified Rust quality collector/series;
- Angular certified capabilities with CRAP explicitly unsupported;
- trusted baseline provider;
- full/ci quality participation and honest hook omission where expensive.

Project-owned E2E/API/smoke steps remain in `flow.toml`; they do not become fake collector capabilities merely to appear in the generic quality report.

## 6. Dogfood phases

### Phase A — observation

No Arc-Admin required authority changes. Generate/import Harness-Gate configuration and run config/schema checks. Record setup friction and unsupported semantics.

### Phase B — shadow parity

Run existing `cargo flow` and Harness-Gate against equivalent source states. Harness-Gate remains non-authoritative for Arc-Admin merge policy. Compare:

- selected components;
- required command-gate results;
- service behavior;
- failure classification;
- diagnostics/artifacts;
- elapsed/runner cost;
- duplicate command/measurement execution.

### Phase C — generic quality addition

Enable certified Rust coverage/CRAP/baseline/ratchet through Harness-Gate. Keep Angular CRAP unsupported. Verify generic quality adds value without altering project-owned command semantics.

### Phase D — controlled negatives

Use temporary/dedicated branches or fixtures, never permanent broken main code, to prove:

- E2E/API/smoke command failure blocks when required;
- collector failure/missing evidence fails closed;
- incompatible/stale baseline cannot reset lineage;
- Rust CRAP regression blocks;
- unsupported Angular CRAP is not invented;
- scope/profile omission is represented honestly.

### Phase E — authority-transfer decision

Only after accepted parity and cost evidence may a follow-up change transfer Arc-Admin required authority to Harness-Gate and remove duplicated old infrastructure. This OpenSpec may recommend that follow-up but SHALL NOT silently delete `cargo flow` as part of initial dogfood.

## 7. Gap classification

Every dogfood discrepancy must be classified before modification:

- **Arc-Admin project issue** — invalid project test/config/code under existing intended policy;
- **Harness-Gate capability gap** — generic execution/result/evidence model cannot express an existing valid gate;
- **Harness-Gate UX gap** — capability exists but migration/configuration/diagnostics are unnecessarily difficult;
- **expected semantic difference** — Harness-Gate intentionally adds stricter generic quality under accepted policy.

A capability/UX gap is fixed or tracked in Harness-Gate. Arc-Admin SHALL NOT weaken or delete an existing valid gate merely to achieve parity.

## 8. CI cost and duplication

During shadowing, duplication may temporarily exist for measurement, but it must be visible and bounded. No permanent final topology should run equivalent authoritative measurements twice.

The dogfood record must distinguish:

- project command work;
- measurement collection work;
- result parsing/report work;
- setup/tool acquisition;
- self-hosted runner wall time.

Authority-transfer recommendations should identify which old jobs/steps can be removed only after Harness-Gate ownership is proven.

## 9. Long-term ecosystem implication

This boundary is intentionally ecosystem-neutral. A future Go + Vue project should be able to keep its Go tests, Vue/Playwright E2E, API tests and custom scripts project-owned while Harness-Gate provides generic command hooks and separately binds certified Go/Vue collectors/packs. No application test framework belongs in Generic Core solely to make its command executable.
