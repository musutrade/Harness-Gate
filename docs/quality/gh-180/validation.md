# GH-180 local validation

Date: 2026-09-09. Scope: OpenSpec tasks 2.1–2.3, ADR-0042.
These results describe the GH-180 changes on `symphony/GH-180`,
based on `caeeca3a34fb450e5de742fb8e09ff550d81a6bd`.

## Reproduction and compiler acceptance

Before implementation, the existing quality configuration suite passed 11 tests
(334 skipped); log: `target/quality/gh-180/before.log`.
The new retained compiler corpus is invoked by the shipped Rust CLI integration
suite. [Acceptance results](acceptance.json) contain 50 cases, with zero
unexplained semantic mismatches. Twenty cases compare direct and compiled
Rust reports byte-for-byte and their exit status; 25 negative cases reject
stale/mixed/malformed inputs; four reject output aliases without damaging inputs;
one proves context changes affect compilation identity. Every equivalence case
also verifies deterministic repeat compilation. Fixtures include an arbitrary
ecosystem, custom collector and series, with both supported and unsupported
metrics. No Python implementation decides these results.

## Required checks

All Cargo-dependent checks use `CARGO_TARGET_DIR="$PWD/target"`; the final
nextest rerun also uses `TMPDIR=/tmp` for the disk-space mitigation below.

| Command | Actual result | Log/artifact under `target/quality/` |
| --- | --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 346 passed, 0 skipped, 161.416 seconds on final code | `gh-180/nextest-retry.log` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 on final code | `gh-180/fmt.log` |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 on final code | `gh-180/clippy-final.log` |
| `python3 -m unittest discover -s tools/quality/tests -v` | 329 tests, OK, 231.949 seconds | `gh-180/python.log` |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0, status pass | `docs-consistency.json`, `gh-180/docs.log` |
| `openspec validate integrate-generic-quality-into-project-workflow --strict` | Exit 0, valid | `gh-180/openspec.log` |
| `python3 tools/quality/fixtures/workflow/compiler/acceptance.py --harness-gate target/debug/harness-gate --output target/quality/gh-180/corpus-final-50` | Exit 0, 50 cases, zero unexplained mismatches on final binary | `gh-180/corpus-final.log`, `gh-180/corpus-final-50/acceptance.json` |

The Python source-measurement contract test includes the new compiler module;
the production inventory and versioned measured source selection include it.
The AST analyzer reports maximum function complexity 24 in the changed CLI
module and 16 in the new compiler. This is a local structural check, not a
replacement for source-verified base/head CRAP evaluation.

## Additional production coverage

`TMPDIR=/tmp CARGO_TARGET_DIR="$PWD/target" python3 tools/quality/coverage.py --production --output "$PWD/target/quality/gh-180/production-final.json"`
exited 0 on final code. Every blocking boundary passes the unchanged 80%
threshold. Config line coverage is 2939/3256 (90.26%); aggregate line coverage
is 12385/14048 (88.16%). [Boundary results](production-coverage.md) are retained.
Full source hashes, inventory, regions and raw export references are in
`target/quality/gh-180/production-final.json`; execution is logged in
`target/quality/gh-180/production-final.log`. This local coverage result does
not establish a source-verified base/head risk comparison or hosted CI success.

## Environment and delivery limitations

Cargo-dependent commands use `CARGO_TARGET_DIR="$PWD/target"` because the
inherited `/home/gem/cargo-target` is read-only. Logs and complete generated
corpus reports are retained under `target/quality/gh-180/` in this workspace.

`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable**: this source checkout has no `.harness-gate/flow.toml`.
No project-local configuration was invented for these checks.

Git staging and commit were attempted, but Git could not create its lock:

```text
fatal: Unable to create '/home/gem/symphony-workspaces/GH-180/.git/index.lock': Read-only file system
```

The initial staging command, `git add docs/quality-compilation.md`, exited
128 with that error (`target/quality/gh-180/staging.log`). On retry, the existing
Git metadata was copied into `target/quality/gh-180/delivery.git` inside this
same workspace. Delivery commands use that writable metadata with the current
workspace as their work tree, preserving `symphony/GH-180` and its baseline.
The original read-only `.git` is unchanged; no other workspace or source
checkout was accessed. The retry verified the retained full-suite logs and
`git diff --check`; unchanged code was not unnecessarily remeasured.

The source-verified base/head risk comparison has not been run locally.
Hosted Required Quality Aggregate and controller acceptance remain pending;
local checks are not a claim of hosted acceptance.
