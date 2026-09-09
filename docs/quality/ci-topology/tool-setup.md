# GH-165: pinned tool setup

OpenSpec `optimize-ci-execution-topology` tasks 2.1–2.3 implement design D3,
under the unchanged [Engineering Policy](../../engineering-policy.md) and
[frozen assurance contract](baseline.md). No new policy delta or ADR is needed.

## Installation contract

The transparent [composite action](../../../.github/actions/install-ci-tool/action.yml)
uses [one version map and verifier](../../../.github/actions/install-ci-tool/tool.sh):

| Tool | Version | Consumers |
| --- | --- | --- |
| cargo-nextest | 0.9.143 | Linux/macOS/Windows full tests, quality collection, push benchmarks, scheduled/manual baseline refresh |
| cargo-llvm-cov | 0.9.0 | Quality collection; existing coverage version retained |
| cargo-audit | 0.22.2 | Required security audit and release governance audit |

Nextest and audit previously floated. These pins match the available local
validation tools; downloaded Linux releases independently passed the verifier.
No coverage/CRAP series identifiers, collector commands, thresholds, supported
platforms, event conditions, release authority, or aggregate behavior change.
Both audit commands retain `--deny warnings` and their original lockfile scope.

The installer is pinned to
[`taiki-e/install-action` d438492cf8a250514fa2d34b30bc3c0dc37c65ff](https://github.com/taiki-e/install-action/tree/d438492cf8a250514fa2d34b30bc3c0dc37c65ff).
Its immutable manifests supply release URLs and SHA-256 checksums. Checksums
are explicitly enabled. Supported releases are downloaded on every invocation,
overwriting any old executable restored by the existing Cargo cache. The
ineffective standalone audit cache was removed. No new binary cache or mutable
artifact authority is introduced; broader Cargo cache changes remain task 3.

If a version/platform is absent from that installer's manifests, explicit
`fallback: cargo-install` invokes `cargo install --locked tool@version` from the
crate registry. Download or checksum failures stop installation; they do not
silently select another binary. Installation failure or failed/missing/wrong
`cargo <subcommand> --version` evidence fails the job before its gate can run.
There is no `continue-on-error` or cache-hit condition. Output retains the full
effective version, including nextest build metadata. Audit 0.22.2 reports
`cargo-audit-audit`; the verifier accepts that release's name explicitly.

To diagnose failures, expand the named resolve/install/verify steps. The
installer reports the selected platform, release, checksum verification and
fallback reason. A version mismatch reports expected and actual output.
When updating pins, review the installer commit and platform manifest hashes,
update the version contract tests, and require native hosted tests. Nextest's
macOS asset at this pin is x86_64; the installer supports its existing macOS
architecture fallback. Native macOS/Windows job execution remains mandatory.

## Hosted evidence and remaining acceptance

The retained [hosted baseline](hosted-baseline.json) contains three successful
pre-change runs (34303413318, 34301967575, 34291282719). Audit installs took
169/183/172 seconds; combined nextest/llvm-cov source installs took
279/236/215 seconds. Existing prebuilt nextest test setup took 1 second on
Linux/macOS and 2–3 seconds on Windows. That already-cheap prebuilt path is
retained, now with explicit version and installer identity. The unconditional
source installs in quality, audit, benchmarks, refresh and release are replaced.
Unrelated tarpaulin setup remains outside tasks 2.1–2.3.

These are before-state measurements, not a claimed speedup. The submitting
agent must not poll CI. The controller must confirm the submitted commit's
`Required Quality Aggregate` is green and inspect hosted install steps for
setup reduction (or record a platform fallback's reason). Comparable after-state
capture remains tasks 6.1–6.2; this change does not claim whole-proposal acceptance.

## Local validation

Before editing, all 3 frozen topology tests passed, confirming the inherited
GH-164 event contract. Source inspection reproduced the ineffective audit cache:
restoring it was unconditionally followed by `cargo install --force`.

Validation logs and downloaded manifest/binary smoke evidence are retained under
`target/quality/gh-165/`. The retained [prebuilt smoke evidence](tool-setup-smoke.json) records that
Linux prebuilt nextest, llvm-cov and audit were fetched
from the immutable installer's manifest URLs, checked against its SHA-256 hashes,
and executed through the same Cargo version verifier (all exit 0). This verifies
the Linux assets locally; hosted action execution and other OSes remain CI work.
Regression tests cover blank, missing, failing, wrong-name, wrong-version and
prerelease evidence, unknown tool selection, explicit pins, installer integrity
settings and workflow audit semantics.

The required Rust and documentation commands use
`CARGO_TARGET_DIR="$PWD/target/gh-165-build"`, because the inherited target
`/home/gem/cargo-target` is outside the permitted workspace. No other checkout
or project configuration is used. Command results are recorded below.

`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable**: this checkout contains no `.harness-gate/flow.toml` declaring
a `ci` profile. No generic config was invented.

Submission uses the GitHub Git database API with the controller-prepared HEAD
as parent because `git add` returned exit 128:
`fatal: Unable to create '/home/gem/symphony-workspaces/GH-165/.git/index.lock': Read-only file system`.
The handoff records the actual pushed SHA; local Git metadata remains unchanged.

| Command | Result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 332 passed, 0 skipped; workspace target override above. |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0. |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0; workspace target override above. |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; report status pass; workspace target override above. |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 307 tests passed. |
| `python3 -m unittest discover -s tools/release/tests -v` | Exit 0; 23 tests passed. |
| `python3 -m unittest discover -s tools/quality/tests -p test_ci_tool_setup.py -v` | Exit 0; 4 tests passed. |
| `openspec validate optimize-ci-execution-topology --strict` | Exit 0. |
| `bash -n .github/actions/install-ci-tool/tool.sh` | Exit 0. |
| `git diff --check` | Exit 0. |

PyYAML parsed the action and all workflows successfully. A structural comparison
of all three edited workflows against the prepared HEAD, excluding only the
scoped tool setup steps, found identical remaining definitions (including all
job commands, conditions, matrices and aggregate dependencies).
