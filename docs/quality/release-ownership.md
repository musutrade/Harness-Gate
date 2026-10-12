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
GateHouse**. GateProbes carries generated copies from a pinned GateHouse
revision and never edits its own copy independently. The `generated_copies` map
in [`release-ownership.json`](release-ownership.json) records each copy's
repository, revision and exact digest. `release_policy.py` and
`release_inventory.py` were copied at revision `dcc9194d` for the native
collector release chain; `generate-sbom.py` is fetched at release time from the
pinned GateHouse source commit and needs no committed copy.

## Plugin release identity

GateProbes declares its own repository, tag prefix, workflow path and environment
rules. GateHouse trusts a signing identity that is explicitly provisioned, never
one inferred from a substituted download URL.

The native Rust/MIR collector now publishes from GateProbes. Its prerelease
`rust-collector-v0.1.0-rc.8` carries the Sigstore certificate identity
`https://github.com/musutrade/GateProbes/.github/workflows/native-collector-release.yml@refs/tags/rust-collector-v0.1.0-rc.8`
with matching provenance, re-verified independently. Core install and trust
still pin the previous identity: switching the consuming side is a separate
explicit step (5.6) and is never inferred from the new download URL. The other
collector workflows and the protected production chain remain tasks 3.4/4.3.

## Enforcement

`tools/quality/tests/test_release_ownership.py` checks that every top-level
`tools/release` file has exactly one owner, that GateHouse keeps
`release.yml`/`install.sh`, that every shared tool's editable source is
GateHouse, and that each recorded generated copy still matches the digest of
the GateHouse source it was copied from. `tools/release/tests` follows the
scripts it covers and is not classified here.
