# Harness-Gate Engineering Policy

Status: **Normative repository policy**

This document defines engineering invariants that apply to every change in this repository, whether the work is performed by a human, Codex, Symphony, another agent, or automation. Accepted ADRs and OpenSpec changes explain and evolve individual decisions; this policy states the durable rules that all later work inherits.

An OpenSpec or issue does not need to repeat these rules. Silence does not disable them. A change may alter a rule only through an explicit normative policy delta with rationale, compatibility/measurement evidence, rollback consequences, and review. Change-specific acceptance criteria may be stricter than this policy but may not silently weaken it.

## 1. Authority and measurement boundary

- Collectors measure and normalize facts. They do not decide final delivery PASS/FAIL.
- Requiredness, thresholds, baseline/lineage rules, debt and ratchet semantics, cross-component aggregation, and final generic decisions belong to the released Rust Harness-Gate core.
- External collectors/adapters may use any implementation language. Their language does not transfer decision authority out of the Rust core.
- A reference, shadow, compatibility, migration, or advisory implementation cannot approve a release or silently become an authoritative fallback.

## 2. Fail-closed required gates

- A required gate may pass only from valid evidence and a successful authoritative decision.
- Missing, stale, malformed, mixed-identity, incompatible, modified, or unverifiable evidence must not become success.
- `unsupported`, `not_configured`, `not_collected`, `not_applicable`, and `measurement_error` are distinct capability states. None may be fabricated as a favorable numeric value.
- If policy requires a capability that is expected to be supported, missing collection or measurement failure is a blocking failure. A genuinely uncertified ecosystem capability must remain explicitly unsupported rather than being guessed.
- Tool failure, timeout, runner loss, missing artifact, parser failure, or collector crash must never be converted to PASS merely to keep delivery moving.

## 3. CRAP, coverage, complexity, and risk

CRAP is a normal quality gate for an ecosystem and measurement series only after that capability has been explicitly validated. For the currently accepted Rust risk series:

- changed production functions require exact CRAP `<= 30`;
- selected functions and changed functions with cyclomatic complexity greater than 10 additionally require line coverage `>= 80%` and region coverage `>= 80%`;
- historical unmodified debt may remain visible under the accepted debt policy, but new debt is forbidden and existing debt must not regress;
- improvements must not silently rebound through a baseline or lineage reset;
- changes outside a certified measurement boundary must fail for measurement review rather than receive invented CRAP or coverage results.

For ecosystems whose CRAP capability has not been certified, CRAP must remain `unsupported`; similar-looking complexity or coverage numbers must not be combined into an invented cross-tool CRAP series.

Thresholds, measurement-series identity, and supported source boundaries are policy. They must not be weakened, widened, or replaced merely to make CI green. Any intentional change requires an explicit policy delta and compatible evidence.

## 4. Debt and no-regression ratchet

- Historical debt may be retained only when the governing policy explicitly permits it and the debt remains visible.
- New or newly selected code must not create new quality debt.
- Existing debt must not worsen.
- An improvement cannot be discarded by silently rebasing to the worse state.
- Exceptions document reviewed context and compensating controls; they do not turn a failing quality measurement into a passing one unless a separately approved policy explicitly defines that behavior.

## 5. CI repair integrity

A failing required gate must be repaired at its root cause. The following are not acceptable CI fixes unless they are the subject of an explicit reviewed policy change:

- lowering CRAP, coverage, complexity, critical-path, performance, or other quality thresholds;
- changing a required gate to advisory or optional;
- deleting or skipping a failing test, mandatory matrix row, supported source, or required platform solely to obtain green status;
- swallowing errors, treating missing evidence as success, or adding an automatic fallback to a non-authoritative implementation;
- broadening waivers/exceptions or resetting baselines to hide a regression;
- changing measurement series so incompatible values can be compared as if they were continuous.

Rollback must preserve evidence and trust semantics. A reviewed revert may remove a broken extension, but it must not rewrite history or silently accept a weaker baseline.

## 6. Verification profiles and cost

Quality is continuous, but not every measurement belongs in every latency tier.

- `hook` is the fast deterministic developer path. Expensive full coverage/CRAP collection and broad cross-platform certification need not run here.
- `full` is the normal complete local verification path and must run the required quality measurements supported by the selected project/policy, including CRAP where certified and required.
- `ci` is the authoritative hosted verification path. It must preserve required evidence, baseline/ratchet semantics, and fail-closed aggregation.
- Expensive cross-platform or ecosystem-certification work should be risk-driven by affected capabilities/paths when that can be done without weakening required assurance.
- CI performance regressions are engineering regressions. New required work must state its expected profile/critical-path cost and avoid unnecessary recollection of equivalent evidence.

## 7. Project-owned validation and extension boundaries

Application- and repository-specific validation logic belongs to the project being verified. Harness-Gate orchestrates, ingests and decides at generic boundaries; it does not become the implementation home for every API, E2E, integration, smoke, migration, load, generation or framework-specific test system.

Harness-Gate extensions are divided into three layers:

- **Command hooks / execution gates** run project-owned validation commands. Harness-Gate owns generic orchestration semantics such as scope/profile selection, dependencies, services, environment handling, timeout, retry, requiredness, logs/artifacts and blocking composition. The project owns the test code, fixtures, domain assertions and tool invocation semantics.
- **Structured result adapters** may ingest reusable machine formats such as JUnit, SARIF or versioned JSON contracts to improve diagnostics and reporting. Parsing a tool result does not transfer requiredness, threshold, ratchet or release authority to that tool or parser.
- **Quality collector plugins** measure normalized facts intended for generic policy evaluation. Collectors remain measurement-only; the released Rust core retains requiredness, thresholds, baseline/ratchet/debt, cross-component aggregation and final generic quality decisions.

A new application validation tool or framework must not require a Harness-Gate Generic Core code change merely to participate if its validation can be expressed through the generic command-hook contract. Native product support is justified only for a reusable protocol, structured-result format, service/execution primitive, ecosystem/capability pack, certification boundary or generic policy semantic—not merely because a particular test runner or framework is popular.

When migrating an existing project with mature validation, current required assurance is the migration baseline until replacement parity and fail-closed behavior are evidenced. A migration must not obtain success by silently dropping, weakening or reclassifying valid project-owned gates. Unsupported migration semantics are product/capability gaps to surface and resolve, not reasons to weaken the project.

## 8. OpenSpec and change governance

Every OpenSpec, issue, PR, and agent task inherits this document.

Changes touching quality, coverage, complexity, CRAP, collectors, evidence, policy, baseline, ratchet/debt, exceptions, CI profiles, presets, project aggregation, release authority, project-validation ownership, structured-result ingestion, or extension boundaries must explicitly state one of:

- the relevant engineering-policy semantics are unchanged; or
- the change proposes a normative policy delta and identifies the exact rule being changed.

A policy delta requires rationale, migration/compatibility impact, negative/fail-closed tests, and evidence demonstrating that the change is intentional rather than a workaround for a failing gate.

## 9. Documentation and machine enforcement

This policy is the normative human/agent contract. ADRs provide decision history; OpenSpec provides change-specific deltas; Harness-Gate configuration and Rust policy code provide executable enforcement.

Documentation and machine enforcement must not knowingly drift. Where a normative rule can be checked deterministically, repository consistency/contract tests should guard it. A documentation mismatch must not be resolved by weakening the machine gate without an approved policy delta.

## 10. Source decisions

This policy consolidates durable rules already established by accepted repository decisions, especially:

- ADR-0039, required coverage/risk/traceability gates;
- ADR-0040, language-agnostic evidence/policy and fail-closed authority boundary;
- ADR-0049, project-owned validation and generic extension boundaries;
- the accepted Rust generic-core authority transfer and Python retention policy.

Those records remain the detailed source for measurement identities, historical acceptance evidence, migration chronology, and extension-boundary rationale.