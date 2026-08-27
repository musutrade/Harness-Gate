# Phase 2 Results

**Status:** In progress - performance and error-boundary tracks completed locally on 2026-08-27.

This report supplements [the Phase 2 baseline](phase-2-baseline.md). Historical
performance figures remain in that report; the values below are the final local
verification for the current refactoring.

| Area | Result | Evidence |
| --- | --- | --- |
| Test execution | 79 tests passed | `TMPDIR=/tmp cargo nextest run --manifest-path tools/harness-gate/Cargo.toml` (0.482s warm run) |
| Distribution binary | 3,138,360 bytes | `cargo build --profile release-small`; below the 3.5 MB target |
| Default release build | 4,622,776 bytes | `cargo build --release`; preserves the default unwind behavior |
| Lint and format | Clean | `cargo clippy --all-targets -- -D warnings`; `cargo fmt -- --check` |
| CLI smoke test | Passed | `init`, `config check`, `secrets`, and `verify --all` using the release-small binary |

## Error-Boundary Refactoring

- Added typed public errors for audit, secret scanning, scope detection, and
  verification, while retaining `anyhow` context within implementation details.
- Added a top-level CLI boundary that renders stable error codes in the form
  `ERROR [E####]: message`.
- Added `utils/fs` and `utils/git` to consolidate report output and the shared
  NUL-delimited Git path/staged-file handling used by scope detection and secret
  scanning.
- Added CLI contract tests for the audit, secrets, scope, and verification error
  categories.

## Remaining Validation

- CI must still validate the change on macOS and Windows.
- Post-merge monitoring remains required by the OpenSpec change; it cannot be
  completed from a local run.
