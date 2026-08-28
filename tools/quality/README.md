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
```

`contracts.py --accept` writes the Linux textual golden snapshot and is a
reviewed local operation; CI never passes that flag. `contracts.py --structured`
is used for macOS and Windows to assert exit status, error code, reports, and
the no-ANSI policy without accepting platform-specific text.

The scheduled `Refresh Quality Baseline` workflow creates a pull request for a
new candidate baseline rather than rewriting a canonical result on its own.
Review its JSON, Markdown, and uploaded raw reports together before merging.
