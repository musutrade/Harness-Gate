## ADDED Requirements

### Requirement: Production inventory SHALL retain compiler provenance
The analyzer SHALL retain source hashes, macro invocation and expansion origins,
cfg/features, toolchain and compiler arguments for all production executable code.
Zero source AST functions SHALL NOT prove absence of generated executable code.

#### Scenario: Expression-shaped or generated macro
- **WHEN** metrics, anyhow, try_join, task_local, macro_rules or a derive is present
- **THEN** explicit grammar and compiler expansion provenance are required
- **AND** unknown or incomplete ownership remains a measurement error

### Requirement: LLVM production ownership SHALL be complete and unique
Every observed LLVM function instance and region SHALL have verifiable owners.
Closures and async bodies SHALL require their own observations. One-to-many origins
and excluded test mappings SHALL be retained; duplicates and ambiguity SHALL fail.

#### Scenario: Missing closure or duplicate region
- **WHEN** a closure lacks compiler evidence or a region is counted twice
- **THEN** certification fails without borrowing its parent function count

### Requirement: Production metrics SHALL use an independent native series
Raw native counts SHALL determine production line, region and function coverage
and exact rational CRAP. Toolchain, selection, mapping and rules SHALL identify the
series; filtered production SHALL NOT reuse llvm-file-summary-unfiltered/1.

#### Scenario: Incompatible historical measurement
- **WHEN** a baseline uses another toolchain, build selection or mapping rule
- **THEN** comparison fails and the baseline remains unchanged

### Requirement: Certification SHALL retain real evidence and limits
Positive certification SHALL use real compiled native output with retained raw
artifacts. Negative validation SHALL detect omissions, ambiguity, duplication and
tampering. Minimal fixtures SHALL NOT imply complete Arc-Admin backend certification.

#### Scenario: Backend cannot be fully measured
- **WHEN** real backend mapping or host capability is missing
- **THEN** the exact failure and original evidence are retained
- **AND** no completion declaration or GH-221 readiness is asserted

## Implementation plan and timeline
Within this issue, execute N0/N1 first, then N2/N3, N4/N5 and N6. Each evidence or
implementation chunk in tasks.md is bounded to less than four hours; unresolved
compiler boundaries require further design and cannot be marked accepted on time
spent. Acceptance requires all four requirements above, not only the prototype.

Example of a bounded experiment (paths remain inside the current workspace):
```sh
python3 tools/quality/rust_native.py collect \
  --source tools/quality/fixtures/rust-native/complete.rs \
  --analyzer target/gh-220/analyzer/debug/harness-gate-rust-measure \
  --output target/gh-220/example-native
```
The output is an evidence-manifest SHA, not a gate pass. `certify --evidence ...
--expected-sha256 ... --output ...` recomputes the bounded report and returns failure
for unchanged threshold violations. It cannot certify a complete Cargo backend.

Alternatives rejected: source AST absence as proof, arbitrary expression fallback,
parent counters for closures, unfiltered file summaries relabeled as production,
and invented JSON positives. A compiler-integrated DefId/expansion collector remains
a possible next implementation; this prototype does not pretend to provide it.

Rollback: remove the opt-in experimental adapter and native syntax version without
changing the old reference series or any accepted baseline. Keep raw evidence and
failure records for review even if the experiment is rolled back.
