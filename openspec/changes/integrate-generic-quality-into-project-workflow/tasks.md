# Tasks: Integrate Generic Quality into Project Workflow

All tasks inherit `docs/engineering-policy.md` and the accepted CI topology. CRAP/coverage/debt/ratchet/fail-closed/measurement-series/Rust-authority semantics are unchanged unless a separate explicit normative policy delta is approved.

## 1. Define quality.toml v1 and cross-plane validation

- [x] 1.1 Define a language-neutral `.harness-gate/quality.toml` v1 schema/model for project/components, source/artifact boundaries, subjects/discovery, relationships, collector bindings, capability/series expectations, policy bindings, profile participation, baseline provider, and report intent.
- [x] 1.2 Enforce authority constraints: collectors cannot declare final requiredness, thresholds, ratchets, aggregate PASS/FAIL, or release authority.
- [x] 1.3 Extend `config check/print` and schema/docs tooling to validate flow/quality cross-references, unresolved components/subjects/relationships, incompatible series, missing required producers, invalid profile participation, and baseline/ratchet contradictions.
- [x] 1.4 Preserve backward compatibility for repositories without `quality.toml`; no implicit new collectors or policy are imposed on legacy flow-only projects.

GH-179 local evidence (2026-09-09): `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` passed 344 tests, including the new config/schema/CLI authority and cross-reference cases and the frozen Rust/Python comparison suites. `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` and `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` passed. `python3 -m unittest discover -s tools/quality/tests -v` passed 328 tests; `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` passed, validating both schemas, the quality example and all legacy presets. Cargo-dependent checks used `CARGO_TARGET_DIR=$PWD/target` because the environment default `/home/gem/cargo-target` is read-only. Full local logs are in `target/quality/gh-179/`. Root `harness-gate config check` and `harness-gate verify --profile ci --all` are not applicable: this source checkout has no `.harness-gate/flow.toml`. `openspec validate integrate-generic-quality-into-project-workflow --strict` passed. Hosted Required Quality Aggregate and controller acceptance remain pending; tasks 2 onward are not implemented by GH-179.

## 2. Compile project configuration to trusted generic inputs

- [ ] 2.1 Implement deterministic compilation from validated quality configuration + repository/source/profile/scope state into `harness-project/v1`, `harness-policy/v1`, trusted expected context, roots, and optional selection/mapping/exception inputs.
- [ ] 2.2 Define stable identity/digest rules for compiled inputs and reject mixed/stale source, config, tool, or artifact identity.
- [ ] 2.3 Add equivalence tests proving direct `quality evaluate` and the compiled path produce identical generic Rust decisions/reports for equivalent inputs, including fail-closed negative cases.

## 3. Orchestrate collectors through the trusted adapter boundary

- [ ] 3.1 Add project-level collector binding/orchestration over the existing signed out-of-process adapter protocol, including compatible protocol/version, executable/package identity, roots, limits, component/subject selection, and expected capabilities.
- [ ] 3.2 Validate capability honesty and normalized evidence: supported/unsupported/not-collected/error remain distinct; no synthetic CRAP or favorable values are invented.
- [ ] 3.3 Fail closed for required collector launch/signature/protocol/timeout/crash errors, missing expected capability, malformed evidence, incompatible series, identity mismatch, stale/mixed artifacts, or duplicate/conflicting authoritative producers.
- [ ] 3.4 Retain low-level `adapter run` as an advanced/debug protocol interface; project users should not manually construct adapter requests for normal `verify`.

## 4. Implement trusted baseline providers

- [ ] 4.1 Implement a deterministic Git base-ref/merge-base provider that materializes exact source identity without mutating the working tree and supplies compatible base context required by the Rust evaluator.
- [ ] 4.2 Implement/define retained CI artifact baseline resolution with exact base commit/config/tool/series identity plus manifest/hash validation under the accepted CI artifact trust model.
- [ ] 4.3 Enforce existing debt/ratchet lineage semantics: missing/incompatible baseline cannot silently reset debt or create a favorable fresh baseline.
- [ ] 4.4 Add positive/negative tests for rename/move lineage, missing base, incompatible series, stale artifact, changed config/tool identity, and optional-vs-required baseline policy.

## 5. Integrate generic quality into verify and unified reporting

- [ ] 5.1 Extend `harness-gate verify` phases to load quality config, resolve selection, run traditional gates, orchestrate applicable collectors, validate evidence, resolve baseline, compile trusted inputs, invoke the released Rust generic core, and emit one final status.
- [ ] 5.2 Ensure traditional step exit codes cannot override generic policy authority; any required traditional or generic failure blocks final verify.
- [ ] 5.3 Define/extend stable machine reporting to link execution outcomes, collector/capability state, evidence/artifact identities, component/local results, cross-component contracts, baseline/ratchet/debt, exceptions, authoritative project report, and final combined status.
- [ ] 5.4 Improve human diagnostics, including CRAP failures with subject, base/head CRAP, threshold/ratchet state, supporting evidence, and remediation context.
- [ ] 5.5 Preserve direct `quality evaluate` as the low-level advanced interface and prove report/decision equivalence with the verify path.

## 6. Enforce profile semantics and quality cost boundaries

- [ ] 6.1 Define explicit hook/full/ci quality participation. Hook may omit expensive full coverage/CRAP collection but must not fabricate full-quality PASS.
- [ ] 6.2 Make certified required Rust CRAP/risk/coverage participate by default in applicable `full`/`ci` preset policy without changing accepted thresholds or series.
- [ ] 6.3 Keep uncertified Angular/TypeScript CRAP explicitly unsupported; do not derive an invented CRAP series from unrelated metrics.
- [ ] 6.4 Ensure CI consumes existing authoritative retained quality evidence where available rather than launching duplicate equivalent collection; record any unavoidable new cost using the accepted CI topology evidence model.

## 7. Upgrade presets and migration UX

- [ ] 7.1 Upgrade `rust-api` to generate coherent flow + quality configuration with accepted Rust quality capabilities and full/CI CRAP policy.
- [ ] 7.2 Upgrade `angular-only` with certified Angular/TypeScript capabilities while keeping CRAP unsupported.
- [ ] 7.3 Upgrade `angular-rust-postgres` with frontend/backend components, ecosystem-specific collectors, relationship/contract modeling, service/execution separation, and one project aggregate.
- [ ] 7.4 Decide/document backward-compatible behavior for `generic` preset and existing flow-only repositories; provide explicit migration/enablement guidance instead of silently imposing policy.
- [ ] 7.5 Extend preset/config/docs consistency tests so generated flow + quality files cross-validate and retain Engineering Policy anchors.

## 8. CI integration and hosted acceptance

- [ ] 8.1 Integrate verify/generic orchestration with the optimized CI topology while preserving pinned tools, one-owner measurement series, immutable artifact validation, lightweight Required Quality Aggregate, and current native platform assurance.
- [ ] 8.2 Prove no duplicate authoritative coverage/risk/CRAP collection is introduced for CI merely because verify now orchestrates generic quality.
- [ ] 8.3 Capture hosted timing/runner-work impact for the new integration and reject/rework execution choices that add avoidable cost without assurance benefit; do not reintroduce previously reverted ineffective compiled-cache/build-once optimizations without new evidence.
- [ ] 8.4 Run semantic parity/negative acceptance across Rust, Angular/reference, and mixed Angular+Rust project shapes; retain exact evidence and Required Quality Aggregate success.

## 9. Documentation, CLI positioning, and closure

- [ ] 9.1 Update README/quick start/architecture/configuration/CLI help so `init -> verify -> one project decision` is the primary product story and `quality evaluate` / `adapter run` are clearly advanced interfaces.
- [ ] 9.2 Document quality.toml schema, capability states, collector-vs-policy authority, baseline providers, profile behavior, CRAP support boundaries, artifact trust, migration, and troubleshooting.
- [ ] 9.3 Strict-validate this OpenSpec and run all required repository quality checks; record final equivalence, fail-closed, hosted CI, performance/cost, and rollback evidence.
- [ ] 9.4 Keep Java/Python/third-ecosystem adapters, compat deprecation, risk-driven conditional hosted platforms, and real application dogfood outside this change; create separate follow-ups only after acceptance.