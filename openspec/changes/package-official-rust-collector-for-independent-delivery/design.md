# Design

## Baseline and decisions
Planning baseline: 5b0313117cefd854dea61d4cd86a86a0be2f3e6f.
Source stays in Harness-Gate; release versioning is independent.
Initial target is x86_64-unknown-linux-gnu only. Minimum glibc/kernel and runtime
dependency versions must be measured and pinned before release, not guessed.
Proposed first immutable tag: rust-collector-v0.1.0-rc.1.

Ship a private Python runtime, required measurement modules, native driver and
matching compiler/LLVM runtime components in a version directory. A small launcher
uses only this private runtime. No ambient Python packages, source checkout or
global rustup default is required. Build-time source retrieval is pinned and
verified; normal collection does not download hidden tooling. Project Cargo
dependencies remain project/host inputs; offline installation does not promise
offline Cargo dependency resolution.

The release dependency inventory must prove closure and redistribution compliance.
Failure to obtain a distributable runtime is a release blocker, not permission to
silently substitute host Python. Frozen C/D helpers retain their documented roles;
do not ship a second generic Python policy/report engine as an authoritative path.

## Command and protocol boundary
Proposed command examples (not available commands yet):
```sh
harness-gate-rust-collector doctor --json
harness-gate-rust-collector collect < request.json > response.json
harness-gate-rust-collector certify --evidence RAW --anchor REVIEWED_SHA
harness-gate-rust-collector classify --evidence RAW --anchor REVIEWED_SHA
```
collect uses existing v1 stdin/stdout envelopes, exclusive evidence/error outcomes,
exact context and artifact validation. Diagnostics go to stderr. It reports facts,
never final release decisions. Operational success returns 0; operational errors
are nonzero and cannot supply usable policy evidence. Capability errors remain
explicit and blocking under existing requiredness.

Current developer certify exits 1 both for bad measurement and threshold failures.
Do not reinterpret every exit 1 as success. The new measurement-only entry must
separate complete authenticated facts from collection errors through typed internal
results, preserve legacy behavior/tests, and prove that genuinely low coverage is
forwarded unchanged for Core evaluation. Historical threshold-derived fields may
remain in retained raw reports but cannot become new transport release decisions.
certify/classify success means evidence integrity/scoped classification only.
A wrapper invoking Core must preserve its nonzero decision and must not implement
threshold/debt logic itself.

## Identity and installation
GH-227's [measurement and delivery contract](../../../docs/quality/rust-collector-delivery-contract.md)
inventories both existing protocol envelopes and native/legacy exits, defines the
typed completion/error boundary, and specifies the strict manifest, exact tested
matrix and conservative capture identity. The reviewed delivery matrix is empty;
no tested independently delivered Core/ABI version is inferred from source pins.
Its synthetic contract tests do not establish native positive measurement.

Manifest binds collector version, source commit, dependency digests, supported Core
versions, protocol versions, capabilities, measurement identities, compiler commit,
LLVM, platform/ABI, normalization/classifier identity and runtime payload inventory.
Exact Core compatibility versions are established by the test matrix before RC.

Current pins: rustc 1.97.1 commit 8bab26f4f68e0e26f0bb7960be334d5b520ea452,
LLVM 22.1.6, rustc-mir-block-inventory/3. Verify pins during implementation.
Package version is not measurement-series compatibility. Tool paths currently
participate in identity: relocation or repackaging must reject historical comparison
unless separately reviewed evidence proves a compatible contract. No path stripping,
resealing, aliasing or baseline reset to manufacture compatibility.
Historical llvm-file-summary-unfiltered/1 remains distinct from MIR production data.

Installer verifies exact tag, trusted workflow identity, signature, inventory,
checksums and provenance before extraction/activation. Reject archive traversal,
unsafe links, extra payloads and incompatible hosts. Use private staging, atomic
activation, explicit version selection and version-isolated directories.
Interrupted install preserves the active version. Rollback selects a previously
verified compatible version; it does not rewrite evidence or accepted baselines.
Uninstall removes only manifest-owned selected-version assets, never project
captures, shared tools or unrelated directories. Never mutate global Rust defaults.

## Distribution and execution trust
Use one explicit release inventory to derive payload, SBOM, signature/provenance
subjects and upload verification. Extend existing release tooling only where its
contracts fit. New tag/workflow eligibility must preserve protected release
environment approval and required checks; do not reuse Core version equality
blindly for independently versioned assets. No tags or publishing in this change's
planning PR. RC publication requires controller approval after implementation CI.

Distribution signatures authenticate package origin. Host-approved capture anchors,
signed requests/state, source identities and baseline lineage are separate inputs.
Installer must not generate trusted keys, accept baselines or fabricate runtime state.
Collectors run with host permissions; the protocol is not an OS sandbox.

## Evidence and GH-215 handoff
Fresh captures retain complete fixed source, binaries, profiles, LLVM exports,
compiler inventory, Cargo selection, commands, tool hashes and independent anchors.
Hash indices alone do not substitute for missing re-export bytes. Document a durable
artifact destination/retention policy before acceptance; expensive binary bundles
need not be committed to Git. Never access historical external workspace paths.
GH-221's truncated archive and missing seven binaries remain disclosed history.

Fixture acceptance certifies the declared fixture scope only. A separately authorized
Arc-Admin owner obtains complete fixed source and makes fresh captures, then pursues
signed full/hook, baseline, frontend/API, negatives, routing and cost work in GH-215.
Packaging is not that acceptance. Stable promotion waits for accepted integration;
genuine low coverage may still block project policy without invalidating collection.

## Alternatives
- crates.io-only install: rejected initially; does not deliver compiler-private ABI,
  LLVM and runtime closure.
- Link compiler tooling into Core: rejected; breaks independent delivery boundary.
- Rewrite all Python first: deferred; not required for measurement-only delivery.
- General registry/plugin manager: excluded; exact signed bundles suffice.
- Broad platform support: deferred until separately evidenced.

## Sequence and estimated effort
P0/P1 contracts precede P2/P3 runtime work; P4/P5 precede P6 integration;
P7 acceptance precedes P8 RC handoff. Each task is at most three focused hours;
split any task exceeding that bound before execution. Estimates exclude external
approvals, hosted build time and Arc-Admin owner scheduling; no calendar promise.

## Validation status
The original planning environment had no OpenSpec CLI and recorded NOT RUN.
That historical state was superseded for exact planning head
`7b6aeb46cd168746686bcc5e48945fcc34c95545` by the
[PR #226 validation receipt](https://github.com/musutrade/Harness-Gate/pull/226#issuecomment-5627974720):
```sh
npx --yes @fission-ai/openspec@1.13.0 validate package-official-rust-collector-for-independent-delivery --strict --no-interactive
```
Exit 0, change valid, using an isolated copy of the exact four change files and
openspec/config.yaml. This was change-scoped validation, not full-repository
`validate --all` or native acceptance. Hosted Documentation Consistency and
Required Quality Aggregate passed in [run 34550353785](https://github.com/musutrade/Harness-Gate/actions/runs/34550353785).
The [controller planning review](https://github.com/musutrade/Harness-Gate/pull/226#issuecomment-5628146966)
found no blockers; it was not an independent GitHub approval. PR #226 merged as
`94b1243f25275b26b2edf3d11f0e12c28eb16eaa` on 2026-09-11.

The later explicit execution handoff authorizes #227–#231 serially, superseding
backlog-only activation wording without authorizing publication or GH-215.
GH-227's actual checks and genuine failures are recorded in
[validation evidence](../../../docs/quality/gh-227/validation.md).
GH-228's [private runtime and local bundle](../../../docs/quality/gh-228/runtime.md)
implement P2/P3 with [actual validation](../../../docs/quality/gh-228/validation.md).
The observed host is pinned exactly; repeated assembly is byte-identical, but
compiler rebuild reproducibility and clean-host acceptance are unproven. The
empty reviewed compatibility matrix continues to reject installed-host requests
until P6. GH-229 implements P4/P5's [verified lifecycle and independent release
contract](../../../docs/quality/rust-collector-installation.md), with
[actual validation](../../../docs/quality/gh-229/validation.md). Its disposable-key
dry-run is nonpublishing and uses synthetic payloads. Administrator-provisioned
host verifier/key trust is required before package code can execute; clean-host
bootstrap acceptance and production key provisioning remain explicit P7/P8
prerequisites. P6 onward remains incomplete. Required CI and controller acceptance
still apply to each implementation PR; planning acceptance is not native/runtime,
baseline or release acceptance. Relevant Engineering Policy semantics are unchanged.
