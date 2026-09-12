# Published releases and remaining work

Verified on **2026-09-12**. This is the current delivery record and Arc-Admin
handoff for GH-239/GH-240/GH-225; earlier preparation documents retain their
historical status. Relevant Engineering Policy semantics remain unchanged.

## Releases

| Component | Version and source | Status |
| --- | --- | --- |
| Core | [0.4.1](https://github.com/musutrade/Harness-Gate/releases/tag/v0.4.1), `62313a251f1862707a887480cbf593130da2ba4a` | GitHub binaries for Linux x86_64, macOS x86_64/arm64 and Windows x86_64; [crates.io 0.4.1](https://crates.io/crates/harness-gate/0.4.1) published, not yanked |
| Rust collector | [0.1.0-rc.1](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-v0.1.0-rc.1), `7165558f5e9f5ca5a0de8c9cd33a11d733909f48` | Six immutable assets; original RSA/Sigstore signatures verified before and after publication |
| Rust installer | [0.1.0-rc.2](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-installer-v0.1.0-rc.2), same source as Core 0.4.1 | Twelve assets; compressed and cached transport installs unchanged collector 0.1.0-rc.1 |

The installer reconstructs the byte-identical signed collector archive from
282,243,324 bytes of compressed layers (original archive: 1,061,498,880 bytes).
First-use download including bootstrap, verifier and metadata is approximately
444 MB. This reduces transfer size, not installed disk usage to the same extent.

Core Linux binary SHA-256: `fb2d79704b4fd31b7372484b5af1f20651906f21020ff1f42027ff9bc39dc0dd`.
Crate checksum: `0a03984be110b4b174c0c4b8ad68925695bfde950549c490cf47e2f904fb20f6`.
Installer SHA-256: `0a1136ede55682d1163ac8588a2d5d637b5ce57b1952b3d1885fc585e942219b`.

## Compatibility

The collector's signed manifest and [reviewed compatibility material](https://github.com/musutrade/Harness-Gate/releases/download/rust-collector-review-7165558-v1/compatibility.json)
bind **Core 0.4.0**, source `ee544690662645806cda7dd4cd2c9192566f7929`, Linux binary
SHA-256 `8e3df8303ca8f650d4ef768b29cfefb60ca115122a47b116245cc19e4649bbdd`.
The exact host is Linux x86_64, glibc 2.43, kernel `7.0.0-31-generic`, with runtime
library inventory fingerprint `f9b213877a2ea9aa44e6119e6fe2e1361f84f9a8084abd410ab8ef34b74e2872`.
Matching only the distribution name or glibc version is insufficient.

Core 0.4.1 and the new installer passed installation/signature verification, but
this does **not** extend native measurement certification to Core 0.4.1. A new
real capture/re-export/Core evaluation and reviewed compatibility entry are
required for that pair; changing the immutable plugin manifest requires a new
plugin version. Retain Core 0.4.0 for the certified measurement path meanwhile.
The repository's empty development matrix is not the published reviewed matrix.

Rust-specific rustc/MIR/LLVM collection is delivered by the optional plugin.
Core retains generic execution, trust validation, requiredness, coverage/CRAP
thresholds, baseline/lineage, debt/ratchet and the final project decision.
Installing the plugin does not provision project-signed requests or policy state.

## Acceptance and immutable handoff

- [Main CI 34704575706](https://github.com/musutrade/Harness-Gate/actions/runs/34704575706), [installer publication 34706631425](https://github.com/musutrade/Harness-Gate/actions/runs/34706631425), and [Core/crates.io publication 34706873399](https://github.com/musutrade/Harness-Gate/actions/runs/34706873399) passed.
- The collector's [original publication run 34692373716](https://github.com/musutrade/Harness-Gate/actions/runs/34692373716) failed during an HTTP/2 readback after signing/upload. Authorized operator continuation used HTTP/1.1 and the unchanged production verifier. Both draft and public readbacks passed; the original Actions failure remains recorded in [recovery receipt](quality/release-0.4.1/recovery-result.json), with the [six-asset sizes and digests](quality/release-0.4.1/published-verification.json).
- [Immutable acceptance/source/bootstrap materials](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-materials-7165558-v1) include `fresh_host_install.json`, `native_capture_reexport.json`, `generic_core.json`, `lifecycle_negatives.json`, `bootstrap_negatives.json`, and `real_sigstore_positive_and_negatives.json`, plus corresponding sources, notices, relink materials, checksums and retention record.
- [Immutable review materials](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-review-7165558-v1) include compatibility, rebuild equivalence and per-file license reviews. The engineering reviewer is recorded as Codex; this is not an invented independent human review.
- Public installer download matched the pinned digest and passed real Sigstore verification against the collector workflow identity on `refs/heads/main`. Running it verified the selected plugin using the cached verifier. Core 0.4.1 was installed from its signed public binary and its version checked; Core 0.4.0 was retained for rollback and certified measurement.

For installation, offline kit, trust and rollback, use the [installation guide](quality/rust-collector-installation.md).
For native acceptance inspect the immutable receipts above: project source,
original binaries/profiles and independent capture anchors remain project/operator
owned. Package signatures are not runtime capture signatures. No Arc-Admin
business source or signing private keys are added to this documentation.

## Issue disposition

| Issue | Disposition and remaining boundary |
| --- | --- |
| [#239](https://github.com/musutrade/Harness-Gate/issues/239) | Completed: production prerequisites, exact candidate/bootstrap certification, license/retention records and approved publication packet are evidenced above. |
| [#240](https://github.com/musutrade/Harness-Gate/issues/240) | Completed: approved RC publication, two verified readbacks and this immutable-source-linked owner handoff. Historical failed workflow is preserved. |
| [#225](https://github.com/musutrade/Harness-Gate/issues/225) | RC delivery P0–P8 completed; no claim of stable promotion or Arc-Admin gate transfer. |
| [#249](https://github.com/musutrade/Harness-Gate/issues/249) | Open: exact Core 0.4.1/plugin native compatibility acceptance and any required new immutable plugin version. |
| [#215](https://github.com/musutrade/Harness-Gate/issues/215) | Remains open: trusted full/hook integration, authenticated baseline lineage, frontend/API evidence, failure/routing parity and complete workload costs. Fresh backend capture/re-export is now available but does not satisfy the whole issue. |

Next delivery work is Core 0.4.1/plugin native compatibility certification, then
GH-215's project-owned complete integration and cost acceptance. Stable collector
promotion and replacement of Arc-Admin's existing gates wait for that evidence.
