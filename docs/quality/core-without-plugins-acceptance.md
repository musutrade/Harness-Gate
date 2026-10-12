# Core build and tests without plugin sources (isolation acceptance)

Status: **partial** (2026-10-12). With the plugin implementation paths removed,
core builds and every cargo test passes; the Python core selection still fails on
four core-owned couplings. This record diagnoses those couplings and proposes
minimal fixes; it does not fix them.

- Commit: `398b0b2f5807092b462aa799c402ef40a3c54bea` (main, includes #348).
- Entry point: `python3 tools/quality/core_isolation_check.py --commit <full sha>`
  (manual acceptance tooling; not wired into any workflow).
- Method: `git archive` snapshot into a temporary directory; the plugin
  implementation paths are removed and their absence asserted; a local git HEAD
  is created inside the copy because quality tools record commit metadata;
  required core inputs are checked explicitly and fail the run when missing;
  cargo runs with an isolated `CARGO_TARGET_DIR` and the existing dependency
  cache; the source repository is never modified.

## Measured results

| Step | Result |
| --- | --- |
| `cargo build --locked --manifest-path tools/harness-gate/Cargo.toml` | pass |
| `cargo test --locked --manifest-path tools/harness-gate/Cargo.toml` | pass — lib 27, bin 396, integration 69, quality-core 0; 492 passed, 0 failed |
| Python core selection | 41 modules, 363 tests; 5 failures in 4 modules (below) |
| Plugin-owned test modules | 17 excluded with an ownership reason (listed in the entry point) |
| Core-owned test modules run | 41 (the remaining modules of `tools/quality/tests`) |

Cargo uses the same locked dependencies as required CI; no test in the isolated
tree can fall back to building plugin source because the plugin implementation
paths do not exist there.

## Findings (core-owned couplings)

| # | Test | Call site | Owner | Judgment | Minimal fix (proposed) |
| --- | --- | --- | --- | --- | --- |
| 1 | `test_docs_consistency.py::test_all_presets_migration_and_schema_use_locked_cargo`, `::test_pure_mode_never_invokes_cargo_and_marks_executable_checks_skipped` | 20 local links in 11 core documents point at plugin implementation paths (for example `docs/quality/source-measure-v2.md` → `../../tools/quality/rust-measure/Cargo.toml`) | core documents, plugin targets | keep the documents in core; retarget the links | point links at the GateProbes repository paths (or the pinned release) instead of local plugin sources |
| 2 | `test_rust_measure_release_pin.py::test_pin_matches_the_plugin_identity_and_asset_name` | reads `tools/quality/rust-measure/plugin.json` | core consumption pin | keep the pin check in core, drop the plugin-source read | validate the pin against the pinned release asset digest and version (already in `tools/quality/rust-measure-release.json`); plugin identity checks live in GateProbes |
| 3 | `test_source_measure.py::setUpClass` | builds `tools/quality/rust-measure` (`Cargo.toml`, `Cargo.lock`, `src/**`) and hashes its sources | mixed core consumer / plugin analyzer | split | core keeps consumer tests against the pinned artifact for host targets; analyzer build and native-evidence tests move to GateProbes |
| 4 | `test_generic_core_corpus.py::test_inventory_covers_all_production_python_modules` | requires `docs/quality/gh-146/python-boundary.json` to equal every `tools/quality/*.py` | core inventory | keep the inventory, allow absent plugin modules | accept declared-but-absent plugin modules while still failing on present-but-unclassified files |
| 5 | 17 test modules importing plugin code (for example `test_rust_native_driver` → `rust_native_driver`, `test_preset_packs` → `typescript_semantics`, `test_native_artifact_paths` → `rust_native_driver`) | module-level imports of plugin implementation | plugin tests | migrate | move these test modules to GateProbes; keep data-level preset assertions in core |

Until findings 1–5 are fixed, the entry point exits non-zero. The gap stays open
in the OpenSpec task list; the four cargo results and the module split above are
the verified part.

## Verified scope and non-claims

- Verified: one exact commit builds and passes every cargo test with the plugin
  implementation paths absent, using locked dependencies and an isolated build
  directory; the Python failure set is exactly the four modules listed above.
- Not verified: any release chain, signing identity, cross-platform product
  matrix, performance baseline or the paused bundled product. No threshold,
  baseline, schema, branch protection or required-check change is implied.
- Required CI still checks out the full repository; this entry point is the
  repeatable acceptance for the isolation gap, not an always-on gate.
