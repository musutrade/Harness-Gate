# Implementation Tasks

All tasks belong to this independent [proposal](proposal.md) and
[acceptance spec](specs/typescript-angular-reference-adapter/spec.md).
Nothing below is implemented by GH-119. S <1h; M 1–2h; L 2–<4h.

## 1. Freeze the real fixture

- [x] 1.1 [P1][M] Select and lock the Angular/Node/TypeScript/builder/runner/provider versions; acceptance: reproducible install and recorded measurement boundaries, argv and configuration digests. [GH-129 evidence](../../../tools/quality/fixtures/typescript-angular/evidence/README.md): two clean locked installs with identical package inventories, exact runtime/tool records and hashed configuration.
- [x] 1.2 [P1][M] Create the CLI-generated application and Rust contract fixture from the design; acceptance: real build, tests and raw coverage, with tested/untested branches, duplicate method names, external template and generated client. [GH-129 evidence](../../../tools/quality/fixtures/typescript-angular/evidence/README.md): production build, seven Angular tests against a live Rust provider, native Istanbul counters, production/test source maps and a retained real compiler failure. Hosted required CI remains pending at submission; no adapter certification is claimed.

## 2. Resolve identity and measurement semantics

- [x] 2.1 [P1][M] Implement versioned source identity and source-map validation; acceptance: TS-01/TS-02 decisions recorded, unique original-source subjects, rejected ambiguity/staleness/tampering and explicit template limitations. [GH-130 evidence](../../../docs/quality/gh-130/README.md): pinned compiler AST identities and source-map validation pass native replay and adversarial provenance tests; [TS-01/TS-02 decisions](../../../docs/quality/typescript-source-semantics.md) retain template/generated measurements as unsupported.
- [x] 2.2 [P1][M] Define coverage counters, capability matrix and measurement series; acceptance: exact native counter fixtures, explicit absent/unsupported states, no invented CRAP or cross-series baseline reuse. [GH-130 evidence](../../../docs/quality/gh-130/README.md): exact native file/function/method counters, unavailable zero denominators, unsupported complexity/CRAP and incompatible TypeScript/Rust series covered by 18 focused tests. Collector/policy integration and certification remain unchecked below; hosted required CI is pending at submission.

## 3. Implement the adapter and reuse generic policy

- [ ] 3.1 [P1][L] Implement the versioned collector adapter; acceptance: retained artifacts validate through the existing runner, TS-03 resolved, failure/timeout and every requested capability covered.
- [ ] 3.2 [P1][M] Evaluate native frontend coverage with generic policy and ratchets alongside Rust; acceptance: thresholds, regression, lineage and debt tests pass without core language branches.
- [ ] 3.3 [P1][L] Integrate real OpenAPI compatibility/generated-client checks; acceptance: a breaking contract blocks project pass despite passing component-local gates, with raw provenance in the report.

## 4. Accept only proven capability

- [ ] 4.1 [P1][M] Retain two base/head acceptance pairs and negative fixtures; acceptance: one compatible pass, one deliberate regression and all design failure cases reproduce, with no unexplained native/generic differences.
- [ ] 4.2 [P1][M] Add advisory CI and publish bounded certification evidence; acceptance: required Rust aggregate is unchanged, all mismatch dispositions link evidence, declared local and required hosted CI checks pass.
- [ ] 4.3 [P1][S] Review and close this follow-up; acceptance: every claimed capability is evidenced, deferred capabilities stay unavailable, any generic contract amendment has separate review plus Rust/frontend validation.
