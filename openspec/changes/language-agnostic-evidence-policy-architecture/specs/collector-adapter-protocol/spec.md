# Capability: Collector Adapter Protocol

## ADDED Requirements

### Requirement: Separate collection from policy evaluation
Collectors and adapters SHALL measure and normalize facts but SHALL NOT be authoritative for final delivery pass/fail decisions. Final policy evaluation belongs to Harness-Gate.

#### Scenario: Collector reports a low coverage value
- **GIVEN** a collector that measures line coverage as 0.72
- **WHEN** it emits evidence
- **THEN** the collector reports the measurement and provenance
- **AND** does not decide whether 0.72 is acceptable for release.

### Requirement: Declare collector capabilities and measurement contracts
Each collector SHALL declare its supported metric capabilities, collector/tool identity and version, rule or parser version where applicable, required inputs, platform/toolchain constraints, and output series identity.

#### Scenario: Collector lacks branch instrumentation
- **GIVEN** a coverage collector that supports line/function/region coverage but not branch coverage
- **WHEN** its capabilities are inspected
- **THEN** branch coverage is explicitly declared unsupported
- **AND** consumers do not infer support from the presence of other coverage metrics.

### Requirement: Produce normalized evidence or explicit measurement failure
A collector invocation SHALL either produce schema-valid normalized evidence linked to raw artifacts or produce a structured collection/measurement error. Silent omission of requested required metrics is not allowed.

#### Scenario: Required analyzer cannot parse source
- **GIVEN** production source outside the collector's validated syntax subset
- **WHEN** collection is requested for that source
- **THEN** the collector returns a structured measurement error
- **AND** the required policy path fails closed instead of treating the source as clean.

### Requirement: Reuse retained evidence when projection is sufficient
Adapters SHALL be able to normalize already-retained evidence without rerunning expensive tests or instrumentation when the existing evidence contains the required facts and compatible provenance.

#### Scenario: Shadow generic evaluation of current Rust CI
- **GIVEN** `ci_quality.py collect` has already retained compatible Rust coverage/risk/traceability artifacts for a run
- **WHEN** the generic shadow evaluator needs normalized evidence
- **THEN** an adapter projects those retained artifacts
- **AND** does not rerun the expensive Rust collection solely for normalization.

### Requirement: Isolate collector implementation from release runtime where practical
Development/CI-only collectors MAY be implemented outside the release binary, but their identity, version, invocation contract, artifacts, and failures SHALL remain reproducible and reviewable.

#### Scenario: Python quality collector remains development-only
- **GIVEN** a Python-based collector used only in CI
- **WHEN** Harness-Gate records its evidence
- **THEN** its version and invocation are captured
- **AND** the collector does not need to be linked into the shipped Harness-Gate binary.
