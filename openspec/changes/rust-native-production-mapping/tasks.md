# Tasks (GH-220)

Parent: [spec](specs/rust-native-production-mapping/spec.md). Each numbered
subtask is a bounded implementation/review chunk under four hours; parent IDs
remain open until their full acceptance criteria have evidence.

- [x] N0 [P0/S] Create specification and close M4.2 only with PR #222 merge/check evidence. Strict OpenSpec validation passed; see [merge evidence](../../../docs/quality/gh-220/pr-222.json).
- [x] N1 [P0/M] Fetch pinned Arc-Admin inside this workspace; verify all 47 source hashes and reproduce committed syntax failures using the rebuilt baseline analyzer. See [baseline replay](../../../docs/quality/gh-220/source-replay-baseline.json).
- [ ] N2 [P0/M] Prove reproducible syntax and expansion provenance for every listed macro, derives and cfg.
  - [x] N2.1 [P0/S] Add bounded metrics/anyhow/try_join grammars and record unresolved macro/attribute syntax; final syntax replay retains three failing files.
  - [x] N2.2 [P0/M] Retain real expanded hygiene contexts and cfg in a single-file experiment; reject missing context edges.
  - [ ] N2.3 [P0/M] Resolve compiler definition IDs and invocation spans for tracing/select, task_local, macro_rules, derives and generated dependency code. Syntax allowlists do not prove resolution.
- [ ] N3 [P0/M] Certify complete production function/closure/async/generated LLVM ownership and test exclusion.
  - [x] N3.1 [P0/M] Join real fixture MIR and LLVM with independent closure/async counters and generic-instance deduplication; preserve raw native artifacts.
  - [x] N3.2 [P0/M] Compile and sample existing backend contract tests; audit production-library MIR and retain unobserved/ambiguous owners.
  - [ ] N3.3 [P0/M] Observe derived/generated functions currently absent from coverage instrumentation; reject missing constructor MIR.
  - [ ] N3.4 [P0/M] Join all backend targets/dependencies through compiler owner IDs and verifiable production/test exclusion.
- [ ] N4 [P0/M] Integrate complete production metrics under an independent versioned series.
  - [x] N4.1 [P0/S] Calculate bounded raw line/region/function coverage and rational CRAP; test incompatible tool/rule/cfg selection; preserve failing thresholds and old baselines.
  - [ ] N4.2 [P0/M] Integrate complete production evidence with immutable historical debt/delta rules. Experimental fixture identity is not a production collector.
- [ ] N5 [P0/M] Validate full production mapping positives and negatives.
  - [x] N5.1 [P0/M] Compile native fixture positives and cfg/derive cases; reject omissions, duplicate/ambiguous owners, tampered exports and incompatible history.
  - [ ] N5.2 [P0/M] Supply complete real backend evidence for production/generated/test-exclusion acceptance; fixture coverage cannot substitute.
- [ ] N6 [P0/M] Complete local checks and submit accepted-scope delivery.
  - [x] N6.1 [P0/M] Run lifecycle, analyzer, OpenSpec and complete critical_paths checks; 392 nextest and 390 Python tests passed, analyzer tests/fmt/Clippy and harness fmt/Clippy passed. Exact results, environment corrections and original failures are in [validation.json](../../../docs/quality/gh-220/validation.json); project-local config/verify are not applicable.
  - [ ] N6.2 [P0/S] Submit indexed PR and normal handoff only after all unresolved production acceptance gaps are closed.

Acceptance requires controller review. Current [evidence and blockers](../../../docs/quality/gh-220/README.md)
do not complete N2–N6, GH-215, or release GH-221. No completion declaration is
written for this incomplete implementation.
