# Documentation-only CI routing

This note records how the core repository routes pure documentation changes and
how the fixed aggregate evaluates the selection. It accompanies the frozen
[CI topology contract](../../tools/quality/fixtures/ci-topology.json) and does
not change gate names, thresholds, measurement series or release authority.

## Classification

The always-started `classify` job runs
`python3 tools/quality/ci_quality.py classify --base-sha <before> --head-sha
<after>` and writes `run_full`, `docs_only` and `files` outputs.

A path is a pure document only when it is prose markdown:

- a root-level `*.md` file, or
- a `docs/**/*.md` file **except** `docs/dogfood/**`, `docs/benchmarks/**` and
  `docs/quality/**`.

Those excluded trees are test fixtures, retained records, specifications and
ownership manifests — not documentation. Protocol files (`schema/**`), presets,
`tools/**`, `.github/**`, install entries and every non-markdown file also keep
the full change class. Renames classify both the source and target path.

`docs_only` is true only when the change set is non-empty and every path is a
pure document. Unknown paths, an empty diff, an unresolvable base or any
classifier error select the conservative full set; a classifier failure fails
the aggregate.

## Execution

- The `docs-consistency` job always runs. With `run_full=true` it installs Rust
  and runs the complete checker; with `run_full=false` it runs
  `docs_consistency.py --pure`, which performs link, heading, policy-anchor and
  sandbox-wording checks without invoking Cargo.
- Every other required job is gated by
  `needs.classify.outputs.run_full == 'true'` (push-only jobs additionally keep
  their `github.event_name == 'push'` condition). No required workflow uses
  `paths`/`paths-ignore`; job-level selection always creates a check run, so a
  required check can never stay Pending.
- The aggregate receives `RUN_FULL`. Full mode requires every event-applicable
  child to succeed. Documentation-only mode requires `classify` and
  `docs-consistency` to succeed and accepts a skipped heavy child, but a heavy
  child that ran and failed still fails the aggregate. A missing or invalid
  `RUN_FULL` falls back to full mode, so a failed classifier cannot pass.

## Independent workflows

`native-collector-release.yml` now applies the same path set to `main` pushes as
to pull requests, so a documentation-only push no longer starts the four-platform
collector build and acceptance. Tag pushes (`rust-collector-v*`) are unaffected:
path filters are not evaluated for tag pushes. `rust-generic-shadow.yml` keeps
its opt-in condition and `rust-measure-native.yml` already filters its pull
requests.

## Measured routing

The documentation-only and full paths are measured on the hosted runs of the
change that introduced this routing; timings are observations, not guarantees.
Full code changes continue to require the complete set of Linux, macOS and
Windows jobs, coverage, risk, contracts and release governance checks.
