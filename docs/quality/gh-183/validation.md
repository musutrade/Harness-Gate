# GH-183 validation

Scope: OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 5.1–5.5.
See [ADR-0045](../../adr/0045-quality-verification-composition.md) and the
[workflow contract](../../quality-verification.md).

Full command logs are retained under `target/quality/gh-183/`. Cargo-dependent
commands use `CARGO_TARGET_DIR=$PWD/target` because the environment's default
target is outside the writable workspace. No thresholds or required CI jobs
are changed.

| Command | Actual result | Log |
| --- | --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 360 passed, 0 skipped | `nextest-final.log` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed | `fmt-final.log` |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed | `clippy-final.log` |
| `python3 -m unittest discover -s tools/quality/tests -v` | 330 passed | `python-tests-final.log` |
| `python3 -m unittest discover -s tools/quality/tests -p test_verify_quality_architecture.py -v` | 2 passed after adding the arbitrary-identifier switch guard control | `architecture-final.log` |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed | `docs-consistency-final.log` |
| `openspec validate integrate-generic-quality-into-project-workflow --strict` | Passed | `openspec-final.log` |

The Rust tests exercise a signed collector with ecosystem
`nebula-unregistered-2049`, verify scope selection (all, changed files and
components), immutable retained baseline resolution, compilation, the released
evaluator and both authoritative and unified reports. Entire direct CLI reports
and decisions match. The success/failure matrix covers independent execution and
quality outcomes, malformed evidence, configuration mutation, invalid trusted
state/selection/baseline, and an audit failure despite generic success. A CRAP
case retains base 35, head 40, threshold 30, ratchet, evidence and remediation in
the human report. The architecture guard checks generic verify and project-report
sources for closed ecosystem enums, language dispatch and ecosystem switches.

The initial Rust run caught a stale generated quality schema; regeneration and
the full final run passed. The initial Python run caught the changed declaration
file's source-inventory hash. AST inspection confirmed it still has no handwritten
production functions or closures; updating the hash and rerunning passed. The
format check caught one test line that was subsequently formatted. Initial logs
are retained alongside the final logs.

Production coverage passes: **13,219 / 14,887 lines (88.796%)**, with every
blocking boundary passing its unchanged threshold. The instrumented nextest run
also passed all 360 tests. The [coverage summary](coverage-summary.json) retains
aggregate and boundary results. Its source inventory includes the new verify
composition source; source-risk selection advances to
`gh183-quality-verification/1` for both base and head without threshold changes.
New composition functions have measured cyclomatic complexity at most 14
(`verify-quality-ast.json` and `complexity-summary.json` in the log directory).

Coverage commands use `CARGO_TARGET_DIR=$PWD/target/gh183-coverage`,
`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_TEST_DEBUG=0` to keep artifacts within the writable workspace and
limit disk use:

```bash
cargo llvm-cov nextest --package harness-gate --manifest-path tools/harness-gate/Cargo.toml --locked --test-threads 2 --no-fail-fast --json --output-path target/quality/gh-183/coverage.raw.json
cargo llvm-cov report --package harness-gate --manifest-path tools/harness-gate/Cargo.toml --lcov --output-path target/quality/gh-183/coverage.lcov
cargo llvm-cov report --package harness-gate --manifest-path tools/harness-gate/Cargo.toml --cobertura --output-path target/quality/gh-183/coverage.cobertura.xml
python3 tools/quality/coverage.py --production --raw target/quality/gh-183/coverage.raw.json --lcov target/quality/gh-183/coverage.lcov --output target/quality/gh-183/production.json
```

All four commands passed. Logs: `coverage.log`, `coverage-export.log` and
`coverage-production.log`. Hosted source-risk comparison remains pending;
local coverage does not establish hosted Required Quality Aggregate status.

`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable**, not passed: this source checkout has no
`.harness-gate/flow.toml` declaring a `ci` profile. No synthetic root project
configuration was added. Hosted Required Quality Aggregate is **CI pending**;
the controller owns that check and acceptance.

Local staging is blocked: `git add tools/harness-gate/src/verify/quality.rs`
failed with `Unable to create '/home/gem/symphony-workspaces/GH-183/.git/index.lock':
Read-only file system`. Publication therefore uses GitHub's Git tree/commit/ref
API on `symphony/GH-183`, preserving the controller's prepared parent and exactly
the validated workspace files. Local Git metadata remains unchanged; the runtime
handoff identifies the actual remote commit. No other checkout is accessed.
