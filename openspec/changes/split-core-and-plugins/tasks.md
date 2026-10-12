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
- [x] 3.3 [P1][M] Split `tools/release/` into core-release and plugin-release groups by target; acceptance: each script has a single owning repository.
- [ ] 3.4 [P1][M] Move plugin build/sign/publish machinery and the four collector workflows to GateProbes; acceptance: GateProbes tests, packages and publishes independently, and core no longer references them.
- [x] 3.5 [P1][M] Re-verify signing identity after the move; acceptance: Sigstore certificate identity and provenance reference the GateProbes repository/workflow, not only a new URL.

> **3.5 verified with a real publication (2026-10-12):** GateProbes published
> `rust-collector-v0.1.0-rc.8` (prerelease, [run 38218575101](https://github.com/musutrade/GateProbes/actions/runs/38218575101))
> from its own workflow. The certificate SAN and provenance signer workflow are
> `https://github.com/musutrade/GateProbes/.github/workflows/native-collector-release.yml@refs/tags/rust-collector-v0.1.0-rc.8`;
> independent local re-verification passed 10/10 `cosign verify-blob`, 10/10
> `gh attestation verify`, the certificate SAN, `SHA256SUMS` and the exact
> inventory. The consuming side switched to this identity in 5.6 on the same
> date; the old repository entry and pinned artifacts are kept. The remaining
> collector workflows and the protected production chain stay under 3.4/4.3.
>
> **3.1/3.2 complete (2026-10-11):** `harness-gate-rust-measure-0.3.1-linux-amd64.tar.gz`
> is published with the `v0.4.8` release (signed, SBOM, recorded in the release
> inventory). The required risk stage now downloads and digest-verifies the
> pinned artifact through `tools/quality/rust-measure-release.json` and records
> the exact release/asset/digest in the candidate report and command log; it no
> longer compiles `tools/quality/rust-measure`. The supplemental
> `rust-measure-native` workflow still builds the analyzer on Windows/macOS for
> native validation and moves with 3.4.
>
> **3.3 complete (2026-10-11):** `docs/quality/release-ownership.{md,json}` and
> `tools/quality/tests/test_release_ownership.py` assign every release script one
> owner. GateHouse keeps `.github/workflows/release.yml` and `install.sh`;
> GateProbes owns the collector lifecycle, candidate staging/signing, production
> release and the Rust collector installers (`production_installer.py`,
> `build_user_installer.py`, `publish_user_installer.py`).
> `release_policy.py`, `release_inventory.py` and `generate-sbom.py` keep one
> editable source in GateHouse; plugin copies are generated from a pinned
> revision. No repository move or signing-identity change is included.
>
> **Remaining blocker (2026-10-11):** 3.4/3.5 are cross-repository moves whose
> signed provenance must be produced and re-verified in GateProbes.

## 4. Remove compatibility paths and handle the rest

- [ ] 4.1 [P1][M] Keep the `harness-gate` command, crate and legacy install entry working through the move; acceptance: existing install and verify commands still succeed.
- [x] 4.2 [P1][M] Isolate the Postgres recipe (`service/postgres.rs`, `ExternalValuePolicy::IsolatedPostgres`) out of the generic config model; acceptance: no Postgres-specific rule remains in the generic core config, and generic process execution/resource reclamation still work.

> **4.2 complete (2026-10-12):** Postgres-specific code is centralized in the
> closed `recipes` module (`src/recipes/postgres.rs`): external-value URL
> validation (loopback/remote override, `_test`/`-test` suffix, `DATABASE_URL`
> conflict), the frozen legacy v1 database mapping and the v1 service
> construction. The generic service layer calls the dispatcher in
> `src/recipes/mod.rs` selected by the closed `ExternalValuePolicy` enum; the
> migration layer passes a narrow `LegacyPostgres` view and no longer contains
> Postgres literals. Docker/Podman execution, timeouts, leases and cleanup stay
> in `service/`; no generic service platform or dynamic plugin system is
> introduced. Startup and doctor paths both keep validation; unknown policy
> values still fail closed. `schema/flow.schema.json` and
> `schema/quality.schema.json` are unchanged. URL positives/negatives, remote
> restriction, production conflict, migration output, preset compatibility and
> affected cleanup tests pass. The reviewed removal exemption covers only the
> confirmed deletion of `src/service/postgres.rs` (present at base, absent at
> head); recreating or modifying that path fails the measurement review, with
> regression tests. The exemption does not establish function-level continuity
> across the move: the new recipe files are measured as new functions, and no
> old-to-new comparison is claimed.
- [ ] 4.3 [P1][L] Handle the Rust complex release chain separately; acceptance: the Rust collector release chain is owned by GateProbes with an explicit, documented process.

> **4.3 disposition recorded (2026-10-12):** the legacy six-asset RSA/Sigstore
> production chain is a paused bundled-runtime product, not the RC8 native
> plugin chain. It stays in GateHouse, unprovisioned and unpublished (its own
> hold step suspends signing/publication); no RSA key, runner group, environment
> approval or publication path is configured. GateProbes owns any future
> revival, which requires a product decision and a reviewed measurement
> migration. Workflow ownership and disposition are recorded and enforced in
> `docs/quality/release-ownership.json` and `test_release_ownership.py`.
- [x] 4.4 [P1][M] Remove compatibility paths only after both repositories are proven green; acceptance: removal is a separate reviewed change and rollback remains possible until then.
- [x] 4.5 [P1][M] Run final strict OpenSpec validation and update related records; acceptance: the change validates and no claim implies new ecosystem certification or new measurement semantics.

> **4.4 completed for the retired entry only (2026-10-12):** reviewed change
> [#343](https://github.com/musutrade/Harness-Gate/pull/343) retired the old
> native publication entry (archived at
> `retired/workflows/native-collector-release.yml`, restore commit
> `6aaa234249574c9a321a8819213b4c936c02605e`). Current main no longer triggers
> publication from it, and pull requests and main pushes no longer start it.
> This does not prohibit creating tags, and a `rust-collector-v*` tag pointing
> at a historical commit still runs the workflow as it existed at that commit;
> no tag protection or historical rewrite is claimed here. Compatibility reads
> (rust-measure source and its core tests, collector fixtures, paused
> bundled-chain hold protections) are deliberately retained. The wider removal
> set stays open: 4.2 Postgres isolation is not done, and the paused bundled
> product stays disabled-retained per the 4.3 disposition; neither blocks this
> change's closure.

> **Old native entry retired (2026-10-12):** `.github/workflows/native-collector-release.yml`
> moved to `retired/workflows/` with its restore commit `6aaa2342` recorded.
> Current main no longer triggers it; tag creation is not prohibited and
> historical-commit tags still run that commit's workflow. Historical releases,
> signatures and source are retained. Compatibility reads stay: rust-measure
> source and its core tests, collector fixtures, and the paused bundled-chain
> hold protections are untouched and must not be deleted without their own
> reviewed change (4.4).

> **4.5 closed against existing acceptance (2026-10-12), no historical suites re-run:**
> - Strict OpenSpec validation: `openspec validate split-core-and-plugins --strict`
>   → `valid` on tree `4ca947b59d0e85a712fb2eb9e2aa118c207fd67c`; the main run for
>   that tree is
>   [38235224213](https://github.com/musutrade/Harness-Gate/actions/runs/38235224213)
>   (first attempt, Required Quality Aggregate pass).
> - Core CI and pinned-artifact consumption checks pass: main
>   [run 38235224213](https://github.com/musutrade/Harness-Gate/actions/runs/38235224213)
>   at `4ca947b59d0e85a712fb2eb9e2aa118c207fd67c`, plus the pinned measurer
>   release-pin test (`tools/quality/tests/test_rust_measure_release_pin.py`).
> - The PR #343 run
>   [38231628449](https://github.com/musutrade/Harness-Gate/actions/runs/38231628449)
>   failed once on an unrelated lease-test flake and passed on rerun; that is
>   flake evidence, not a root-cause fix, and is kept as the observation record
>   in [#344](https://github.com/musutrade/Harness-Gate/issues/344). The main
>   run above passed on its first attempt.
> - A plugin version bump needs no core rebuild or re-release: GateProbes published
>   independently in [run 38218575101](https://github.com/musutrade/GateProbes/actions/runs/38218575101)
>   ([release rust-collector-v0.1.0-rc.8](https://github.com/musutrade/GateProbes/releases/tag/rust-collector-v0.1.0-rc.8),
>   source commit `155b233fb2f525f520b4fcc0ed73e017154345e0`); core consumption is a
>   pinned-artifact switch ([PR #340](https://github.com/musutrade/Harness-Gate/pull/340),
>   `77b0109472441095b20e5e0ae70b83100755fc77`).
> - Core and plugin release/rollback independently: core identity unchanged
>   (0.4.8 binary `888fa1527eae37e3163aac2e1479e91ec03a0c8380b3c49da6e9266575597418`);
>   plugin rollback materials retained (archived entry restore commit `6aaa2342`,
>   pinned artifacts; installer signer fix `892d6974a9fa3dd36c9c8f1bee20bd1a4d3b82dd`).
> - An incompatible protocol version fails explicitly: covered by the existing
>   GateProbes chain acceptance (helper 8/8 zero-skip, accept 23/23 on all four
>   platforms, [run 38215704216](https://github.com/musutrade/GateProbes/actions/runs/38215704216))
>   and not re-run here.
> - CI time and artifact disk do not increase versus the pre-split baseline:
>   routing measurement recorded in `docs/quality/ci-docs-routing.md`; no new
>   ecosystem certification or measurement-semantic claim is made.

> **Open acceptance gap (not completed):** "Core builds and tests with no
> plugin source checked out" is still not demonstrated: CI checks out the full
> repository, so the aggregate cannot prove it. It stays tracked in the
> Validation list and is not claimed by 4.4/4.5; it does not block 4.2.
>
> **Isolation acceptance, partial (2026-10-12):** the repeatable entry point
> `tools/quality/core_isolation_check.py` ran commit
> `398b0b2f5807092b462aa799c402ef40a3c54bea` with the 26 plugin implementation
> paths removed: `cargo build` and all 492 cargo tests pass in an isolated
> target directory. The Python core selection (41 modules, 363 tests) still
> fails on four core-owned couplings: core docs linking plugin paths, the
> measurer pin reading `rust-measure/plugin.json`, `test_source_measure`
> building the analyzer, and the gh-146 inventory requiring every module.
> Findings, ownership judgments and proposed minimal fixes are recorded in
> `docs/quality/core-without-plugins-acceptance.md`; the gap stays open until
> those fixes land and the entry point passes.

## 5. GateProbes bootstrap, CI classification and migration scope

Decisions D10–D13 in the design govern this section. Development order: scaffold
and classification framework first, then one low-dependency plugin, then the
remaining plugins, and the release/signing identity switch last.

- [ ] 5.1 [P0][M] Scaffold GateProbes minimal source and the always-on CI classification framework; acceptance: the workflow always starts, classifies changes, selects subtasks, and ends in one fixed aggregate check; unknown changes fall back to the conservative set and no required workflow uses `paths-ignore`.
- [x] 5.2 [P0][M] Split pure-documentation checks from executable-example checks and classify GateHouse paths; acceptance: a pure-doc change does not invoke Cargo, and `ownership.json`, protocol specifications and test inputs under `docs/` are classified as inputs, not documentation.
- [ ] 5.3 [P0][M] Migrate one low-dependency plugin and validate the independent flow; acceptance: the plugin repo builds, tests, runs its pinned-Core contract acceptance and clean-installs without the core source.
- [ ] 5.4 [P1][M] Migrate the remaining plugins under D10 (source, manifests/lockfiles, tests, minimal fixtures, protocol copies, licenses, build/install/release scripts; no history, caches, old binaries); acceptance: each migration records source repository, full commit SHA and path mapping.
- [ ] 5.5 [P1][S] Keep the rust-reference equivalence suite and `rust-generic-shadow` in core; acceptance: no historical input under `docs/quality/gh-96`/`gh-97` is deleted, and no moved test is skipped for a missing input.
- [x] 5.6 [P1][M] Switch plugin release and signing identity last; acceptance: provenance and certificate identity reference GateProbes explicitly, and core keeps consuming pinned artifacts without recompiling.

> **5.6 complete (2026-10-12):** `install.sh` now downloads the Rust/MIR
> collector from the GateProbes releases, verifies the provisioned GateProbes
> workflow identity for the exact tag, and defaults to `0.1.0-rc.8`; Core
> binary installs keep the Harness-Gate `release.yml` identity and the core CI
> risk stage keeps consuming the pinned rust-measure artifact without
> recompiling. The offline installer contract tests pin the new repository and
> identity, including wrong-workflow, wrong-tag and environment-pollution
> negatives. The protected production chain (RSA/Sigstore, six assets, x86_64
> Linux) is a separate paused product and stays unprovisioned; see the 4.3
> disposition note.

## Validation

- Core builds and tests with no plugin source checked out.
- A plugin version bump needs no core rebuild or re-release.
- Core and plugin release/rollback independently.
- An incompatible protocol version fails explicitly.
- CI time and artifact disk do not increase versus the pre-split baseline.
