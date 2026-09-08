# Generic policy engine v1

GH-114 implements OpenSpec tasks 5.1–5.5 as a standalone **shadow** evaluator.
The existing Rust required gates and `Required Quality Aggregate` remain release
truth under [ADR 0039](../adr/0039-required-risk-and-traceability-gates.md).
[Collectors](collector-protocol.md) return facts; policy owns comparisons,
requiredness, violation severity and aggregation. No required CI wiring changes.

The [closed JSON policy schema](../../tools/quality/schema/policy.schema.json)
requires explicit rule IDs, scope, generic metric name, typed limit, operator,
requiredness, violation state and remediation classes. Unknown fields and metric
names, duplicate rule IDs, invalid selectors, mismatched types and executable
expressions are rejected. Operators are `lt/le/eq/ne/ge/gt`; boolean values allow
only `eq/ne`. Ratios retain numerator/denominator and decimals use canonical
strings. Comparisons use exact rational arithmetic, including large counts,
durations and sizes. Limits use the metric's normalized type and unit.

## Scope and evidence ownership

Every rule evaluates each declared subject within the requested target:

| Selector | Selected subjects |
| --- | --- |
| `project` | All project subjects in the target |
| `component` + `component` | Subjects owned by that component |
| `boundary` + `component` + `boundary` | Subjects in that component's source boundary |
| `changed_subject` | Caller-supplied changed subject IDs |
| `critical_subject` | Caller-supplied critical subject IDs |

These are selection scopes; they do not average metrics across subjects or
measurement series. A component or boundary summary can be represented as a
subject by the project model. Changed/critical selections are supplied separately
from collector evidence, validated against the project and target, and required
when that selector is used. Missing selected-subject evidence, ambiguous series,
invalid provenance and corrupt artifacts produce `measurement_error`. An empty
selection produces `not_applicable` and cannot satisfy a required rule. A project
with an unmeasured contract subject consequently cannot pass a project coverage
rule merely because its measured functions pass.

The evaluator validates the entire evidence batch through the existing
[evidence contract](harness-evidence.md) before using values. It never trusts a
collector's numerical pass/fail judgment. Unavailable metrics have no fabricated
numeric value. Capability mapping is explicit:

| Capability | Gate result |
| --- | --- |
| `supported` | Comparison gives `pass` or configured `fail/warning/informational` |
| `unsupported` | `unsupported` |
| `not_applicable` | `not_applicable` |
| `not_configured` | `blocked` |
| `not_collected` | `skipped` |
| `measurement_error` | `measurement_error` |

## Results and aggregation

`GateResult` and `GateState` retain `pass`, `fail`, `warning`, `informational`,
`skipped`, `unsupported`, `not_applicable`, `measurement_error`, `blocked`, and
`cancelled`. The latter represents orchestration cancellation without conflating
it with a threshold failure. The aggregate reads requiredness from policy, never
from child records. Required states other than `pass` and `informational` block;
`required: false` explicitly makes a rule non-blocking. A required warning still
blocks. Missing required rules block. Duplicate children and unknown policy IDs
are rejected. Informational results retain their records without blocking.

The aggregate preserves every blocking child and original state. Its summary is
`measurement_error` if any such blocker exists, otherwise `fail` if any threshold
failure exists, otherwise `blocked`, or `pass` with no blockers. Thus mixed
failure/error/cancellation results cannot erase one another's causes.

`harness-policy-results/v1` JSON contains `mode: shadow`, `aggregate`, all
`results`, and non-pass `violations`. Each child has `policy` (rule ID), `subject`
(ID), `state`, `reason`, and a `record` containing:

- component, complete subject identity and generic metric;
- typed base/head values (null when unavailable) and commit/base/target/run context;
- complete governing policy, including the typed limit and operator;
- base/head measurement-series IDs and evidence IDs with artifact paths/digests;
- remediation classes from policy for comparisons, or `repair_measurement` for
  unavailable/invalid measurements.

The Python API accepts optional separately validated base evidence for remediation
context. Base commit must match the head's declared base; matching subject IDs
must have compatible series. Missing base subjects remain null. This does not
classify changes, map renamed subjects, accept baselines, compute regressions or
implement debt/ratchets (tasks 6.x). Invalid batches report a policy-level error
with null untrusted subject/value/link fields and caller-owned commit context.

## Standalone example and validation

Run from the repository root:

```bash
python3 tools/quality/policy_engine.py \
  --policy tools/quality/fixtures/policy/policy.json \
  --evidence tools/quality/fixtures/harness-evidence/polyglot.json \
  --project tools/quality/fixtures/project-model/base.json \
  --source-root tools/quality/fixtures/project-model/sources \
  --artifact-root tools/quality/fixtures/harness-evidence \
  --expected tools/quality/fixtures/harness-evidence/expected.json \
  --output target/quality/policy-example.json
```

Expected exit code: **1**. Frontend coverage passes (4/5); CRAP 3.072 exceeds
3 and produces a structured failure. This synthetic example certifies no real
ecosystem adapter. `--selection` accepts an object with `changed_subject` and/or
`critical_subject` ID arrays. The CLI exits zero only for aggregate pass; invalid
policy/input emits a machine-readable measurement error and exits one.

[Focused tests](../../tools/quality/tests/test_policy_engine.py) cover scope
ownership, exact comparison boundaries, coverage/CRAP/mutation/breaking counts
and booleans across four synthetic ecosystems, capability states, corrupt and
missing evidence, every aggregate state, base-series compatibility, remediation
serialization and CLI exit behavior. Actual command evidence is recorded in the
[GH-114 validation summary](gh-114/validation-summary.json).
