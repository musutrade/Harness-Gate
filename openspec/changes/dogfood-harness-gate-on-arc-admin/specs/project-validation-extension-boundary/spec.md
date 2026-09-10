# Project Validation Extension Boundary

## ADDED Requirements

### Requirement: Project-owned validation SHALL remain project-owned

Harness-Gate SHALL treat application- and repository-specific test or validation implementations as project-owned behavior. The project SHALL remain responsible for test code, domain assertions, fixtures, request flows, browser automation, load scenarios, migration checks, generation checks and other application-specific validation logic.

Harness-Gate SHALL provide generic orchestration and result/evidence integration rather than reimplementing project test frameworks.

#### Scenario: Project uses Playwright for E2E
- **GIVEN** a project owns Playwright E2E tests
- **WHEN** the tests are configured as a required Harness-Gate execution step
- **THEN** Harness-Gate executes and gates the project command through generic execution semantics
- **AND** Harness-Gate does not require a Playwright-specific Generic Core implementation
- **AND** the Playwright test logic remains in the project repository

#### Scenario: Project changes E2E framework
- **GIVEN** a project replaces Playwright with another executable E2E runner
- **WHEN** the replacement can be expressed using the existing command-hook contract
- **THEN** the project can update configuration without modifying Harness-Gate Generic Core

### Requirement: Harness-Gate SHALL expose a generic command-hook execution layer

Harness-Gate SHALL support arbitrary project-owned validation commands as execution gates. The generic execution layer SHALL own configurable scope/profile participation, dependencies, services, environment handling, timeout, retry, requiredness, logs/artifacts and final blocking composition.

The execution layer SHALL NOT infer application-domain policy from a tool name.

#### Scenario: Project API tests are a shell or package command
- **GIVEN** a required API-test command defined by the project
- **WHEN** the selected profile/component includes that step
- **THEN** Harness-Gate executes the command with its configured services/environment/timeouts
- **AND** a required failed execution blocks the final workflow
- **AND** no API-test-framework-specific Core branch is required

#### Scenario: Project validation requires PostgreSQL
- **GIVEN** a project-owned full-stack smoke step declaring a PostgreSQL service
- **WHEN** Harness-Gate executes the step
- **THEN** it supplies the configured isolated service contract
- **AND** preserves project-defined validation logic outside Harness-Gate

### Requirement: Structured result adapters SHALL enrich diagnostics without acquiring policy authority

Harness-Gate MAY support reusable structured-result formats or adapters, including JUnit, SARIF or versioned declared JSON contracts. Structured result ingestion SHALL preserve provenance and improve machine/human diagnostics, but SHALL NOT independently define Harness-Gate requiredness, thresholds, ratchets, release approval or final generic PASS/FAIL unless those semantics are governed by an explicit accepted Harness-Gate policy contract.

#### Scenario: E2E command emits JUnit
- **GIVEN** a project-owned E2E command emits a valid JUnit report
- **WHEN** a structured result adapter is configured
- **THEN** Harness-Gate may expose test counts, failures and locations in the unified report
- **AND** the execution gate remains governed by configured Harness-Gate requiredness
- **AND** the JUnit producer does not gain release authority

#### Scenario: Structured result is malformed
- **GIVEN** policy/configuration requires a structured result for a required gate
- **WHEN** the result cannot be parsed or validated
- **THEN** Harness-Gate fails closed according to the declared result contract
- **AND** does not silently fall back to a favorable interpretation

### Requirement: Quality collector plugins SHALL remain measurement-only extensions

Measurements intended for generic quality policy SHALL enter through the accepted collector/evidence boundary. Collectors SHALL produce normalized facts with compatible source/config/tool/series identity; the released Rust core SHALL retain requiredness, thresholds, baseline/ratchet/debt semantics, cross-component aggregation and final generic decisions.

#### Scenario: Coverage and complexity feed CRAP policy
- **GIVEN** a certified ecosystem collector produces accepted coverage and complexity evidence
- **WHEN** the corresponding quality policy requires CRAP
- **THEN** the evidence enters the generic Rust evaluator
- **AND** the collector does not decide whether delivery passes

#### Scenario: Ecosystem has no certified CRAP series
- **GIVEN** a project ecosystem whose CRAP measurement series is not certified
- **WHEN** quality verification runs
- **THEN** CRAP remains explicitly unsupported
- **AND** Harness-Gate does not combine unrelated tool outputs into an invented CRAP value

### Requirement: Tool/framework participation SHALL be configuration-driven unless a reusable protocol boundary requires product support

A new application validation tool or framework SHALL NOT require Harness-Gate Generic Core code changes merely to execute its project-owned command. Product-specific support MAY be added for reusable protocol/format/certification boundaries, but SHALL be justified independently from the popularity of a particular tool.

#### Scenario: Unknown future test runner is introduced
- **GIVEN** an executable future test runner unknown when Harness-Gate was released
- **WHEN** its invocation, services, environment and result contract can be represented by existing generic configuration
- **THEN** it participates in required verification without schema or Generic Core redesign

#### Scenario: Reusable result protocol needs native support
- **GIVEN** multiple ecosystems produce the same stable structured result protocol
- **WHEN** native parsing would materially improve diagnostics or trust validation
- **THEN** Harness-Gate may add a protocol adapter
- **AND** that adapter remains generic to the protocol rather than embedding one application framework's test implementation

### Requirement: Dogfood migration SHALL preserve existing project assurance before authority transfer

When Harness-Gate is dogfooded on an existing project with a mature validation workflow, the existing required assurance SHALL be treated as a migration baseline/oracle until parity and fail-closed replacement behavior are evidenced. Harness-Gate SHALL NOT obtain a successful migration result by deleting, weakening, skipping or silently dropping existing valid project gates.

#### Scenario: Existing project has a full-stack smoke gate
- **GIVEN** Arc-Admin currently requires a project-owned full-stack smoke test
- **WHEN** Harness-Gate shadow configuration is introduced
- **THEN** the equivalent validation remains represented and required in the applicable full/CI flow
- **AND** any inability to express it is classified as a Harness-Gate capability gap
- **AND** the Arc-Admin gate is not removed to make parity pass

#### Scenario: Imported execution config contains unsupported semantics
- **GIVEN** an existing execution-plane rule cannot be represented losslessly
- **WHEN** import/migration is attempted
- **THEN** Harness-Gate reports the unsupported mapping explicitly
- **AND** does not silently discard the rule
- **AND** authority transfer remains blocked until the gap is resolved or explicitly reviewed
