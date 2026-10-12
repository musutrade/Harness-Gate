# Implementation Tasks

All tasks belong to [proposal](proposal.md), [design](design.md), and the
acceptance delta under `specs/core-plugin-boundary/`. This change cuts
dependencies first and moves repositories second. No task changes measurement
semantics, gate names, branch protection or release authority.

## 1. Tidy ownership in the current repository (no algorithm change)

- [x] 1.1 [P0][M] Assign a source of truth for each schema role (host contract / generic evidence / plugin raw output) across `schema/`, `quality-core/schema/`, `tools/quality/schema/`; acceptance: each duplicated basename has exactly one editable source and any copy is marked generated.
- [x] 1.2 [P0][S] Move `tools/quality/schema/collector-protocol.schema.json` to the core host-contract location; acceptance: the protocol schema is authoritative in core and referenced by path.
- [x] 1.3 [P0][M] Re-house the decision-engine fixture currently referenced by `src/config/quality/collectors/ci_acceptance.rs` into core; acceptance: core builds with no `include_str!` that reaches `tools/quality/fixtures/**`.
- [x] 1.4 [P0][M] Split fixture ownership: keep generic decision / invalid-evidence / timeout / replay fixtures in core, mark collector-specific fixtures for GateProbes; acceptance: a reviewed list shows each fixture's owner and no core test depends on a plugin fixture.
- [x] 1.5 [P1][S] Update `tools/quality/README.md` and any boundary note so the roles match D4/D5; acceptance: documentation states each schema role and fixture ownership without implying a merge by filename.

## 2. Prove one independent plugin chain (TypeScript-risk)

- [x] 2.1 [P0][M] Package the TypeScript-risk collector as a versioned artifact that core can consume by pinned reference; acceptance: the artifact installs and runs without core source and reports its exact version.
- [x] 2.2 [P0][M] Run core against the pinned TypeScript-risk artifact; acceptance: verify/decision path works end-to-end with the collector present only as a pinned artifact.
- [x] 2.3 [P0][S] Define the explicit-error behavior for an incompatible collector protocol version; acceptance: an incompatible version fails closed with a clear diagnostic instead of a silent or partial result.
- [x] 2.4 [P1][S] Record the independent chain evidence (commands, versions, artifacts); acceptance: reviewable record shows the chain is independent of core source.

## 3. Core CI consumes pinned plugin artifacts, then move GateProbes release

- [x] 3.1 [P0][L] Remove the core-CI step that builds the Rust measurer from collector source; acceptance: core CI no longer compiles collector source and uses a pinned plugin artifact instead.
- [x] 3.2 [P0][M] Make core CI select a pinned collector version explicitly (no "latest source"); acceptance: the exact version is recorded in CI config and logs.
- [ ] 3.3 [P1][M] Split `tools/release/` into core-release and plugin-release groups by target; acceptance: each script has a single owning repository.
- [ ] 3.4 [P1][M] Move plugin build/sign/publish machinery and the four collector workflows to GateProbes; acceptance: GateProbes tests, packages and publishes independently, and core no longer references them.
- [ ] 3.5 [P1][M] Re-verify signing identity after the move; acceptance: Sigstore certificate identity and provenance reference the GateProbes repository/workflow, not only a new URL.

> **3.1/3.2 complete (2026-10-11):** `harness-gate-rust-measure-0.3.1-linux-amd64.tar.gz`
> is published with the `v0.4.8` release (signed, SBOM, recorded in the release
> inventory). The required risk stage now downloads and digest-verifies the
> pinned artifact through `tools/quality/rust-measure-release.json` and records
> the exact release/asset/digest in the candidate report and command log; it no
> longer compiles `tools/quality/rust-measure`. The supplemental
> `rust-measure-native` workflow still builds the analyzer on Windows/macOS for
> native validation and moves with 3.4.
>
> **Remaining blocker (2026-10-11):** 3.4/3.5 are cross-repository moves whose
> signed provenance must be produced and re-verified in GateProbes. 3.3 remains
> doable in-repo but needs the 🟨 core/plugin decision for the shared
> `release_policy.py`, `release_inventory.py`, `generate-sbom.py`,
> `production_installer.py`, `build_user_installer.py` and
> `publish_user_installer.py`.

## 4. Remove compatibility paths and handle the rest

- [ ] 4.1 [P1][M] Keep the `harness-gate` command, crate and legacy install entry working through the move; acceptance: existing install and verify commands still succeed.
- [ ] 4.2 [P1][M] Isolate the Postgres recipe (`service/postgres.rs`, `ExternalValuePolicy::IsolatedPostgres`) out of the generic config model; acceptance: no Postgres-specific rule remains in the generic core config, and generic process execution/resource reclamation still work.
- [ ] 4.3 [P1][L] Handle the Rust complex release chain separately; acceptance: the Rust collector release chain is owned by GateProbes with an explicit, documented process.
- [ ] 4.4 [P1][M] Remove compatibility paths only after both repositories are proven green; acceptance: removal is a separate reviewed change and rollback remains possible until then.
- [ ] 4.5 [P1][M] Run final strict OpenSpec validation and update related records; acceptance: the change validates and no claim implies new ecosystem certification or new measurement semantics.

## Validation

- Core builds and tests with no plugin source checked out.
- A plugin version bump needs no core rebuild or re-release.
- Core and plugin release/rollback independently.
- An incompatible protocol version fails explicitly.
- CI time and artifact disk do not increase versus the pre-split baseline.
