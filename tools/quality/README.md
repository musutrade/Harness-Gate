# Quality evidence and required CI gates

This directory contains Python CI tooling, ecosystem adapters, generic shadow
semantics and migration references. The [frozen product-boundary inventory](../../docs/quality/gh-146/python-boundary.md)
classifies every production module and records its callers, authority and final
disposition. These modules are not linked into, packaged with, or executed by
the `harness-gate` release binary. Generic semantics are scheduled to move into
Rust after differential acceptance; this freeze does not transfer authority.
The [shared compatibility corpus](fixtures/generic-core/README.md) retains exact
inputs, full expected outputs and native/source bytes for that migration.

[GH-151](../../docs/quality/gh-151/README.md) prepares the Rust product command and
opt-in differential workflow. Its explicit transfer review awaits
hosted required CI and Rust shadow acceptance; the presence
of that command does not accept or replace required CI authority.

Run from the repository root with Python >=3.12, stable Rust plus
`llvm-tools-preview`, cargo-nextest and cargo-llvm-cov **0.9.0** installed.
The collector uses a fresh workspace-local target; standalone commands should
also override an ambient shared/read-only Cargo target:

```bash
export CARGO_TARGET_DIR="$PWD/target/build"
python3 -m unittest discover -s tools/quality/tests -v
python3 -m py_compile tools/quality/*.py tools/quality/tests/*.py
python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json

# Commit product source changes first; provide a full base SHA already in Git.
# CI supplies the PR base (or push before), the tested SHA, and run ID/attempt.
BASE_SHA=$(git rev-parse origin/main)
HEAD_SHA=$(git rev-parse HEAD)
RUN_ID=local-$(date +%s)
python3 tools/quality/ci_quality.py collect \
  --base-sha "$BASE_SHA" --head-sha "$HEAD_SHA" --run-id "$RUN_ID" \
  --output "target/quality/$RUN_ID"
python3 tools/quality/ci_quality.py verify \
  --base-sha "$BASE_SHA" --head-sha "$HEAD_SHA" --run-id "$RUN_ID" \
  --output "target/quality/$RUN_ID"
```

This is exactly CI's collection entry point and candidate schema 1. It retains
base/head/run identity, commands/exits/timings, four required stage results and
SHA-256 references to raw evidence. Existing output directories, stale artifacts,
missing base objects and incomplete collections fail. A candidate is never
accepted automatically. Full raw artifacts are uploaded with `always()`.

The stages preserve the six-module 80% gate, evaluate eleven production
boundaries and aggregate at 80%, run the versioned function-risk ratchet,
and collect isolated matrix evidence (all mandatory paths and >=95% applicable
rows). Changed production Rust files outside the supported risk boundary fail
with a measurement-review diagnostic; these reports do not certify whole-project
CRAP. See [ADR-0039](../../docs/adr/0039-required-risk-and-traceability-gates.md).

GH-151 extends the inventory to `production-source-2`: `src/` and the linked
`quality-core/` are production, including the public replay module. The core is
an eleventh blocking boundary. Test-only modules retain explicit cfg(test)
exclusions. The unpublished `quality-replay/` comparator is migration tooling.
Declaration-only files without LLVM records remain production with pinned hashes;
no executable counters are synthesized. The `gh151-generic-production/1` risk
selection measures 23 sources on both commits with analyzer 0.3.0 / mccabe-rust-3,
including vec repetition expressions. Both workspace packages are covered on the
base. Newly added sources are explicitly absent from its manifest and verified
against Git. The original selected functions, 80% line/region and CRAP <=30
thresholds, exact arithmetic, lineage and historical-debt rules are unchanged.

Individual diagnostic commands use the same report formats:

```bash
python3 tools/quality/coverage.py --output target/quality/coverage.json
python3 tools/quality/coverage.py --production \
  --raw target/quality/coverage.raw.json --lcov target/quality/coverage.lcov \
  --output target/quality/production.json
python3 tools/quality/critical_paths.py --collect \
  --evidence target/quality/critical-path-runs/bundle.json \
  --output target/quality/critical-paths.json
python3 tools/quality/measurement_contract.py
python3 tools/quality/contracts.py
python3 tools/quality/benchmarks.py --output target/quality/benchmarks.json --samples 5
```

The matrix rejects aggregate nextest JSONL/module coverage as a replacement for
isolated evidence. The collector continues other stages after an ordinary gate
failure, retains the failure and exits nonzero. Candidate timings describe this
collection environment; hosted CI overhead comes from the uploaded CI candidate.
This repository has no project-local `.harness-gate/flow.toml` declaring `ci`:
`harness-gate config check` and `harness-gate verify --profile ci --all` are not
applicable here.

The helper tests and bytecode compilation run in the `Quality Script Tests`
CI job and are included in `Required Quality Aggregate`. Quality scripts are
reviewed as production-like policy code: a failing or untestable script cannot
silently approve a release.

`measurement_contract.py` records the commit, target directory/triple, Cargo
profile, tool versions, exact nextest test-binary paths and digests, and the
`cargo-llvm-cov` instrumentation environment. Its child-process rule is
explicit: `LLVM_PROFILE_FILE` is inherited, but a child executable contributes
coverage only when that executable was built with `-C instrument-coverage`.
The command reports branch coverage as `unsupported` unless the baseline command
is explicitly changed to request branch instrumentation. The output is
candidate evidence, not an automatic baseline acceptance.

## Complexity analyzer and quality evidence

The locked development complexity analyzer
(`complexity_analyzer.py`, identity `harness-gate-complexity` 0.1.0, MIT)
is a stdlib-only, in-repository Python tool used for quality evidence only. It
is never linked into the release binary. Its frozen rule is `mccabe-rust-1`
version 1, and its supported Rust fixture subset, raw-count contract, series
identity, and source-symbol identity are documented in
[`docs/quality/complexity-analyzer.md`](../../docs/quality/complexity-analyzer.md).
The versioned machine schema is
[`schema/quality-evidence.schema.json`](schema/quality-evidence.schema.json).

Regenerate the candidate evidence and validate it:

```bash
python3 tools/quality/complexity_analyzer.py \
  --source tools/quality/fixtures/complexity/controls.rs \
  --source-root tools/quality/fixtures/complexity \
  --output target/quality/complexity/controls.json
python3 tools/quality/quality_evidence.py validate \
  --record target/quality/complexity/controls.json
python3 tools/quality/complexity_analyzer.py \
  --source tools/quality/fixtures/complexity/controls.rs \
  --source-root tools/quality/fixtures/complexity \
  --output target/quality/complexity/controls-again.json
python3 tools/quality/quality_evidence.py compare-series \
  --record target/quality/complexity/controls.json \
  --record target/quality/complexity/controls-again.json
```

`compare-series` rejects records whose canonical series key differs, so a
generated record can only be compared with the committed expected fixture when
their toolchain fields match; the unit tests normalize target/Python and drop
`commit` to compare fixture structure reproducibly. Unsupported Rust syntax
raises `ComplexitySyntaxError` and the analyzer exits non-zero, so the frozen
subset fails closed instead of silently widening.

The benchmark runner executes the checked-in no-network fixture in both serial
and opt-in parallel modes. Its JSON keeps per-mode raw samples, the configured
concurrency limit, fixture-observed peak concurrency, report-derived step
timings, scheduler overhead, and the shareable-service startup/reuse counts.
A median comparison is under
`verification.serial`, `verification.parallel`, and `verification.comparison`.

`post_remediation_benchmarks.py` is a local evidence generator for the R-16
wait/backoff and scheduler scenarios and the R-17 validation-allocation
change. It alternates samples between a pre-change binary (for example the
v0.3.5 or pre-hardening commit) and the post-change v0.3.6 binary, then writes
per-sample JSON and reviewable Markdown under `docs/benchmarks`. It is a
manual review tool; the periodic quality-baseline workflow intentionally does
not run it.

`contracts.py --accept` writes the Linux textual golden snapshot and is a
reviewed local operation; CI never passes that flag. `contracts.py --structured`
is used for macOS and Windows to assert exit status, error code, reports, and
the no-ANSI policy without accepting platform-specific text.

The scheduled `Refresh Quality Baseline` workflow creates a pull request for a
new candidate baseline rather than rewriting a canonical result on its own.
Review its JSON, Markdown, and uploaded raw reports together before merging.

## Baseline Exceptions

An exception never converts a failed quality result into a pass. It is a
temporary review record that must include:

- a tracked issue or pull request;
- one accountable owner;
- the measured failure and a concrete rationale;
- an expiry date; and
- approval from a repository code owner.

The exception is recorded in the pull request description and linked from the
evidence summary. The threshold, raw result, and aggregate check remain
unchanged. At expiry the owner either lands a fix, submits a reviewed baseline
change, or closes the exception; expired exceptions cannot be carried forward
silently.

The first accepted baseline is recorded in
`docs/benchmarks/phase-1/README.md`. New candidates are accepted only after
the `Required Quality Aggregate` check and the corresponding raw artifacts
have been reviewed.

The CI workflow keeps coverage, contract, benchmark, and documentation jobs
independent for diagnostics, then runs `quality-required` with `always()`. That
aggregate job fails closed when any dependency fails, is cancelled, or is
skipped, and is the single check to select in repository branch protection.

## Standalone generic project model

The GH-111 [project model](../../docs/quality/project-model.md) and
[migration compatibility inventory](../../docs/quality/migration-compatibility.md)
implement only OpenSpec tasks 0.3 and 1.1–1.4. Synthetic fixtures need no language
toolchains. The current Rust required gates remain authoritative; this model does
not evaluate policy or collect generic evidence.

## Shadow project contracts and reporting

[`project_report.py`](project_report.py) checks an opt-in
`.harness-gate/project.json` and emits project/component/gate reports from retained
normalized evidence. [Configuration, contract metrics and CLI examples](../../docs/quality/project-reporting.md)
include a single Rust topology and a synthetic Angular + Rust + Python + Java
project whose green local gates are blocked by a breaking API contract. These
fixtures do not certify additional adapters or replace required Rust CI.
