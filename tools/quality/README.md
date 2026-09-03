# Phase 1 quality evidence

The files in this directory are Python standard-library orchestration for
quality evidence. They invoke the Rust CLI and Cargo tooling, retain their raw
output, and turn it into reviewable JSON/Markdown summaries. They are not
linked into, packaged with, or executed by the `harness-gate` release binary.

Run the gates from the repository root:

```bash
python3 tools/quality/coverage.py
NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 cargo nextest run \
  --manifest-path tools/harness-gate/Cargo.toml --locked --no-fail-fast \
  --message-format libtest-json-plus --message-format-version 0.1 \
  > target/quality/nextest.jsonl
python3 tools/quality/critical_paths.py \
  --evidence target/quality/nextest.jsonl \
  --coverage target/quality/coverage.json
python3 tools/quality/contracts.py
python3 tools/quality/benchmarks.py
python3 tools/quality/docs_consistency.py
python3 -m unittest discover -s tools/quality/tests -v
python3 -m py_compile tools/quality/*.py tools/quality/tests/*.py
```

The helper tests and bytecode compilation run in the `Quality Script Tests`
CI job and are included in `Required Quality Aggregate`. Quality scripts are
reviewed as production-like policy code: a failing or untestable script cannot
silently approve a release.

The benchmark runner executes the checked-in no-network fixture in both serial
and opt-in parallel modes. Its JSON keeps per-mode raw samples, the configured
concurrency limit, fixture-observed peak concurrency, report-derived step
timings, scheduler overhead, and the shareable-service startup/reuse counts.
A median comparison is under
`verification.serial`, `verification.parallel`, and `verification.comparison`.

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
