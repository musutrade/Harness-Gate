# Release and install ownership

This note assigns release and install machinery to one owning repository for the
core/plugin split (OpenSpec task 3.3). It is a classification only: nothing moves
between repositories here, no signing identity changes, and the Postgres recipe,
repository rename and compatibility-path cleanup remain later steps.

| Owner | Path | Why |
| --- | --- | --- |
| GateHouse | `.github/workflows/release.yml` | Core tag release workflow: build, quality gates, publish and crates.io |
| GateHouse | `install.sh` | Core install entry for `harness-gate` |
| GateProbes | `tools/release/bootstrap_collector.sh`, `build_collector_bootstrap.py` | Collector bootstrap/toolchain delivery |
| GateProbes | `tools/release/collector_*.py` | Collector lifecycle: assets, components, transport, store, receipts, policy, Sigstore, install and maintenance |
| GateProbes | `tools/release/install_collector.py`, `download_collector_release.py`, `friendly_collector_install.py` | Collector install, download and user-facing selection |
| GateProbes | `tools/release/package_independent_collectors.py`, `prepare_collector_candidate.py`, `stage_collector_candidate.py`, `sign_collector_candidate.py`, `production_collector_release.py` | Collector packaging, candidate staging/signing and production release |
| GateProbes | `tools/release/production_installer.py`, `build_user_installer.py`, `publish_user_installer.py` | Rust collector production bootstrap, user installer build and publication |
| GateProbes | `tools/release/rust-collector-release.production.yml` | Collector release workflow template |

## Shared tools

`release_policy.py`, `release_inventory.py` and `generate-sbom.py` are used by
both core and plugin release paths. They keep **one editable source in
GateHouse**. When GateProbes is created it carries generated copies from a pinned
GateHouse revision and never edits its own copy independently. The
`generated_copies` map in [`release-ownership.json`](release-ownership.json)
tracks those copies; it is empty while both sides still share one tree.

## Plugin release identity

GateProbes declares its own repository, tag prefix, workflow path and environment
rules. GateHouse trusts a signing identity that is explicitly provisioned, never
one inferred from a substituted download URL. This is a design rule only in this
step; repository moves, workflow migration and provenance re-verification remain
tasks 3.4 and 3.5.

## Enforcement

`tools/quality/tests/test_release_ownership.py` checks that every top-level
`tools/release` file has exactly one owner, that GateHouse keeps
`release.yml`/`install.sh`, and that every shared tool's editable source is
GateHouse. `tools/release/tests` follows the scripts it covers and is not
classified here.
