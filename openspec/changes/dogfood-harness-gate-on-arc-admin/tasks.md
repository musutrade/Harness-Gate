# Tasks: Dogfood Harness-Gate on Arc-Admin

This change includes an explicit normative Engineering Policy delta for project-owned validation boundaries. Existing CRAP/coverage/debt/ratchet/fail-closed/measurement-series/Rust-authority semantics remain unchanged.

## 1. Freeze durable architecture policy

- [ ] 1.1 Add the project-owned validation principle to `docs/engineering-policy.md`.
- [ ] 1.2 Document the three extension layers: command hooks, structured result adapters, and quality collector plugins.
- [x] 1.3 Add a regression/consistency guard ensuring application test frameworks are not required as generic-core special cases merely to execute project-owned commands.
- [ ] 1.4 Record the decision in ADR-0049 and strict-validate this OpenSpec.

## 2. Freeze Arc-Admin assurance baseline

- [x] 2.1 Inventory every existing required Arc-Admin step, component, scope rule, service, parser, timeout, environment rule and CI route.
- [x] 2.2 Record current `cargo flow` hook/full behavior and required delivery semantics.
- [x] 2.3 Capture current self-hosted CI cost/timing and evidence/artifact behavior as the before state.
- [x] 2.4 Mark project-specific E2E/API/full-stack/generation/deployment checks explicitly as project-owned validations, not Harness-Gate missing built-ins.

GH-201 evidence: [frozen source/inventory, semantics, CI timing/artifacts and ownership](../../../docs/dogfood/arc-admin/README.md), reproduced by `python3 docs/dogfood/arc-admin/reproduce.py` and its three passing quality-script tests. Task 1.3 is exercised by the passing `project_owned_runner_replacement_preserves_generic_command_gate` integration test (two unknown runners, success/failure, logs and sealed evidence) and the Engineering Policy anchors in documentation consistency. Strict validation: `openspec validate dogfood-harness-gate-on-arc-admin --strict`. This baseline evidence covers the task IDs authorized by GH-201; it does not establish migration, shadow parity or acceptance of the complete proposal.

## 3. Import/migrate the execution plane

- [x] 3.1 Design and implement a deterministic migration/import path for structurally compatible `.arc-flow/flow.toml` execution configuration.
- [x] 3.2 Preserve aliases, components, scope, services, parsers, profiles, required steps, dependencies, environment handling and timeouts where semantics match.
- [x] 3.3 Fail visibly on unsupported or lossy mappings; never silently drop a required Arc-Admin blocker.
- [x] 3.4 Measure manual migration effort and configuration duplication as product UX metrics.

GH-202 evidence: [deterministic import, loss detection, full declaration parity and UX counts](../../../docs/dogfood/arc-admin/import/README.md), [actual local validation results and limitations](../../../docs/dogfood/arc-admin/import/validation.json). Five Rust CLI import tests pass in the 385-test nextest run; independent Python fixtures compare all 25 blockers and every preserved source value against GH-201. Regenerated TOML/report fixtures are byte-identical. Strict OpenSpec validation passes. Runtime shadow parity, Required Quality Aggregate CI and acceptance of the complete proposal are not claimed by these task checkboxes.

## 4. Configure Arc-Admin generic quality

- [ ] 4.1 Add Arc-Admin Harness-Gate quality configuration for Angular frontend, Rust backend and relevant component relationships.
- [ ] 4.2 Bind the accepted Rust coverage/risk/CRAP series and trusted baseline provider without changing thresholds or lineage semantics.
- [ ] 4.3 Keep Angular/TypeScript CRAP explicitly unsupported unless separately certified.
- [ ] 4.4 Keep E2E/API/smoke/generation checks as execution hooks rather than fake quality collectors.

## 5. Run observation and shadow parity

- [ ] 5.1 Run Arc-Admin's current `cargo flow` and Harness-Gate against equivalent source states without transferring merge authority.
- [ ] 5.2 Compare selected components and all required traditional gate outcomes.
- [ ] 5.3 Compare PostgreSQL service behavior, environment isolation, diagnostics, reports and artifacts.
- [ ] 5.4 Classify every discrepancy as Arc-Admin issue, Harness-Gate capability gap, Harness-Gate UX gap or expected stricter generic-quality difference.

## 6. Exercise controlled negative scenarios

- [ ] 6.1 Prove a required project-owned E2E/API/smoke command failure blocks through the generic command-hook path.
- [ ] 6.2 Prove required collector/evidence failure fails closed.
- [ ] 6.3 Prove stale/incompatible baseline cannot reset ratchet/debt lineage.
- [ ] 6.4 Prove accepted Rust CRAP regressions block and Angular unsupported CRAP remains non-fabricated.
- [ ] 6.5 Prove hook/full/ci omissions remain honest and do not become fake PASS.

## 7. Evaluate CI topology and cost

- [ ] 7.1 Measure shadow-mode added wall/runner cost on Arc-Admin's self-hosted CI.
- [ ] 7.2 Identify duplicate command execution and duplicate authoritative measurement collection.
- [ ] 7.3 Define a no-duplication authority-transfer topology that preserves current assurance.
- [ ] 7.4 Do not delete existing CI/arc-flow gates until replacement ownership and fail-closed behavior are evidenced.

## 8. Product gap remediation

- [ ] 8.1 Fix or separately track any Harness-Gate capability gap revealed by Arc-Admin rather than weakening Arc-Admin.
- [ ] 8.2 Fix or separately track migration/configuration/diagnostic UX gaps that make compatible project validation unnecessarily difficult.
- [ ] 8.3 Add regression fixtures/tests for every generic product gap fixed through dogfood.
- [ ] 8.4 Re-run parity and negative evidence after any Harness-Gate remediation.

## 9. Authority-transfer recommendation and closure

- [ ] 9.1 Produce an evidence-backed recommendation on whether Arc-Admin can transfer required workflow authority from `cargo flow` to Harness-Gate.
- [ ] 9.2 If transfer is not yet safe, keep shadow mode and list exact blockers; do not force closure.
- [ ] 9.3 If transfer is safe, create a separate follow-up change for removal/consolidation of old Arc-Admin workflow infrastructure.
- [ ] 9.4 Retain final migration effort, parity, negative, cost, rollback and product-gap evidence; strict-validate and close this dogfood change only when all claims are supported.
