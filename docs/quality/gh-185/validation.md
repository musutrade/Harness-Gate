# GH-185 validation

Scope: OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 7.1–7.5.
Engineering Policy semantics are unchanged. See [ADR-0047](../../adr/0047-composable-quality-preset-packs.md)
and [preset defaults and migration](../../quality-presets.md).

## Behavior

Reference presets consume reusable ecosystem and independent contract pack data.
Full/CI require selected producers and policies; hook explicitly provides partial
assurance. Rust preserves the accepted series, limits and requiredness; TypeScript
CRAP remains unsupported. The mixed preset aggregates components and an API
relationship while PostgreSQL remains an execution service. A synthetic unknown
ecosystem composes and cross-validates without a catalog entry or generic branches.
Generic initialization preserves opt-in quality; flow-only migration adds none.
Output conflicts fail before any configuration files are committed.

## Local checks

The commands below ran from the GH-185 workspace on Linux x86_64 with Rust
1.97.1 and Python 3.14.4. Cargo and Python subprocesses used
`CARGO_TARGET_DIR="$PWD/target"`: the environment's default `/home/gem/cargo-target`
is read-only. The first Cargo attempt failed with
`failed to open: /home/gem/cargo-target/debug/.cargo-build-lock` and
`Read-only file system (os error 30)`. The workspace-local override resolved this;
no check is counted as passed from that failed attempt.

| Command | Final result | Artifact under `target/quality/gh-185/` |
| --- | --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | PASS: 378 passed, 0 skipped | `nextest-complete.log` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | PASS | `fmt.log` |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | PASS | `clippy.log` |
| `python3 -m unittest discover -s tools/quality/tests -v` | PASS: 341 tests | `python-tests-validated.log` |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | PASS: all four presets, opt-in migration, schemas, links and Engineering Policy anchors | `docs-consistency.log`; JSON at the requested output path |
| `openspec validate integrate-generic-quality-into-project-workflow --strict` | PASS | `openspec.log` |

Full logs are uncommitted workspace artifacts. Earlier runs exposed two obsolete
test assumptions (reference init was flow-only; CI was undeclared), the new source
inventory entry and the declaration-only module hash. Those were corrected without
changing production coverage thresholds or policy semantics. A temporary missing
validation-record link was resolved before the successful docs check.

Root `harness-gate config check` and `harness-gate verify --profile ci --all` are
not applicable: this source checkout has no `.harness-gate/flow.toml`. No synthetic
root project was created. Documentation consistency initializes temporary public
preset projects and cross-validates their real generated configurations.

## Submission environment

Normal staging with `git add README.md docs
openspec/changes/integrate-generic-quality-into-project-workflow
tools/harness-gate tools/quality` failed with
`Unable to create '<workspace>/.git/index.lock': Read-only file system`.
Delivery uses a bare/shared clone of this checkout under the ignored
`target/quality/gh-185/delivery.git`, with `--work-tree=.` and the same
`symphony/GH-185` branch and prepared parent commit. Commit/push operations use
that writable Git administration directory; the original read-only `.git`
metadata remains at the controller-prepared commit. No other workspace is used.

## Limitations

Initialization provisions configuration and reference series metadata, not signed
requests, keys, runtime state or live collector evidence. Actual verification
requires host provisioning and matching installed series. Missing inputs fail
closed. Baseline/ratchet adoption is explicit; no debt waiver is generated.
The long-term pack registry/package manager and full execution-pack composition
are not implemented. Hosted integration belongs to OpenSpec section 8; no CI job
or duplicate measurement owner is added. Required Quality Aggregate and hosted
acceptance remain CI/controller owned and pending at submission.
