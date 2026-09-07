# GH-94 draft implementation evidence

Scope: OpenSpec tasks 6.1–6.6 in
[`strict-json-results-and-risk-based-quality-gates`](../../openspec/changes/strict-json-results-and-risk-based-quality-gates/tasks.md).
Dependencies #92 and #93 were closed with `state_reason=completed` before work
started. **Acceptance is blocked by production function measurement errors.**
The tasks remain unchecked. This draft is not a production risk baseline or a
completed delivery.

## Changes and compatibility

| Original hotspot | Extracted responsibilities |
| --- | --- |
| `doctor/checks.rs::run_check` | Path, environment, environment/file, Git configuration and version checks |
| `app/commands.rs::run` | Doctor, cleanup, scope, secrets, audit, verify and hook handlers; existing `CliError` return contract retained |
| `verify/mod.rs::run_selected` | Service status, ordered result normalization and report publication |
| `verify/steps.rs::configured_task` | Runner construction, isolation allocation, shard metadata and final environment injection |
| `process/adapter.rs::run_with_cancel` | Request preparation, process execution/waiting, repeated termination/reaping, output budgets and response validation |
| `verify/parser.rs::count_json_results` | Explicit-path counting and recursive recognized-result discovery |

The verification coordinator retains planning/waiver checks before allocation,
cleanup before publication, and cancellation/primary/cleanup error precedence.
Adapter waiting retains child-exit, stdin failure, stdout overflow, stderr
overflow, cancellation and timeout ordering. Reader/writer deadlines share the
same elapsed budget. Runner environment construction retains service injection
before explicit removals. JSON traversal and valid-input count semantics are
unchanged. The Issue #93 regression suite ran before and after the changes.

## Blocking evidence

The unchanged analyzer from #92 is `harness-gate-complexity 0.1.1`, rule
`mccabe-rust-1` version 1. It fails on **all six selected source files at both
base and head**. See the committed
[commands, exit statuses and source SHA-256 identities](gh-94-measurement-errors.json).
All six retained base files were verified byte-for-byte against Git commit
`764482ca64c754cdb56e3a3f97bc267023497736`. Head hashes identify the draft's
production source bytes, independently of the documentation commit.

For example:

```bash
python3 tools/quality/complexity_analyzer.py \
  --source tools/harness-gate/src/doctor/checks.rs \
  --source-root tools/harness-gate \
  --output target/quality/gh-94/head-doctor-checks.rs.complexity.json
```

Exit 1: `src/doctor/checks.rs:34:28: non-braced closure body is outside the
frozen syntax subset (braced closures only)`.

Other failures include `expected an item, found 'const'` in task construction
and adapter code, and `unterminated item` in verification orchestration. These
failures precede the refactor. A temporary, uncommitted expression-closure probe
still failed on production syntax; its output is not accepted evidence.

The [frozen analyzer contract](complexity-analyzer.md) and
[function mapping contract](function-risk.md) prohibit silently dropping
unsupported symbols or mixing measurement series. Consequently this draft
does **not** claim function line/region coverage >=80%, `crap_line <=30`,
complete old/new function identity mapping, or an enumerated historical debt
baseline. Those are still required for every task's acceptance. Module or suite
coverage cannot substitute for the missing function evidence.

To unblock acceptance, the production syntax and source-location mapping must
be supported by a versioned, fixture-validated analyzer. Both base and head
must then be measured with that same series, all extracted functions must be
checked individually, and historical debt must be reported separately. The
frozen risk tooling and its thresholds are unchanged by this draft.

## Validation

Complete local logs and raw exports are retained under `target/quality/gh-94/`.
`CARGO_TARGET_DIR=target/gh-94-cargo` keeps builds inside this workspace.

| Exact command | Actual result |
| --- | --- |
| `CARGO_TARGET_DIR=target/gh-94-cargo cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | PASS before refactoring: 308 tests, 0 skipped (`baseline-nextest.log`); PASS on final source: 308 tests, 0 skipped (`nextest-final.log`) |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | PASS (`fmt.log`) |
| `CARGO_TARGET_DIR=target/gh-94-cargo cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | PASS (`clippy.log`) after removing two redundant borrows introduced by extraction |
| `python3 -m unittest discover -s tools/quality/tests -v` | PASS: 85 tests (`quality-tests.log`) |
| `CARGO_TARGET_DIR=target/gh-94-cargo python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | PASS: examples, migration, links and schemas (`docs-consistency-final.log`) |
| `openspec validate strict-json-results-and-risk-based-quality-gates --strict` | PASS (`openspec.log`) |
| `CARGO_TARGET_DIR=target/gh-94-coverage cargo llvm-cov nextest --manifest-path tools/harness-gate/Cargo.toml --locked --json --output-path target/quality/gh-94/base-coverage.json` | PASS: baseline instrumented suite, 308 tests, 0 skipped (`base-coverage.log`); 467 raw profiles retained in `base-profiles/` |
| `CARGO_TARGET_DIR=target/gh-94-coverage cargo llvm-cov nextest --manifest-path tools/harness-gate/Cargo.toml --locked --json --output-path target/quality/gh-94/head-coverage.json` | PASS: final instrumented suite, 308 tests, 0 skipped (`head-coverage.log`); raw LLVM export retained, not accepted function-risk evidence |
| `harness-gate config check` | NOT APPLICABLE: `.harness-gate/flow.toml` is absent |
| `harness-gate verify --profile ci --all` | NOT APPLICABLE: no project-local configuration declaring a `ci` profile |

The environment's inherited Cargo target directory is read-only. The first
unmodified docs-consistency invocation failed its Cargo-backed checks; using
the workspace target directory resolved them. No project configuration was
invented. During measurement investigation,
`python3 -m venv target/quality/gh-94/venv` also failed because `ensurepip is not
available` (the system suggests `python3.14-venv`). No alternate parser was
installed; this attempt supplies no measurement evidence.

Toolchain: Rust 1.97.1, LLVM `22.1.6-rust-1.97.1-stable`, Python 3.14.4,
target `x86_64-unknown-linux-gnu`. No macOS/Windows acceptance or complete branch
coverage is claimed. [ADR-0025](../adr/0025-phase-1-quality-baseline-gates.md)
and its historical measurements remain unchanged.

Local staging is also blocked by the workspace's read-only `.git` mount:
`git add` exits 128 with `Unable to create
'/home/gem/symphony-workspaces/GH-94/.git/index.lock': Read-only file system`.
The draft is published with the GitHub Git Data API on `symphony/GH-94`, using
the prepared baseline as its parent. Local Git metadata remains at the
baseline. No completion handoff is written for this unfinished work.
