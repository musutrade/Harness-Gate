# Published releases and remaining work

Verified on **2026-09-13**. This record distinguishes released delivery, reused
native evidence and remaining project integration. Engineering Policy semantics
and historical failed runs remain unchanged.

## Releases

| Component | Version and source | Status |
| --- | --- | --- |
| Core | [0.4.1](https://github.com/musutrade/Harness-Gate/releases/tag/v0.4.1), `62313a251f1862707a887480cbf593130da2ba4a` | Signed GitHub binaries; [crates.io 0.4.1](https://crates.io/crates/harness-gate/0.4.1) published |
| Rust collector | [0.1.0-rc.2](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-v0.1.0-rc.2), `19d7ed59851d5f042730eb5cb7cf9e0aa67a1345` | Six signed assets; both production readback verifications passed |
| Rust installer | [0.1.0-rc.3](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-installer-v0.1.0-rc.3), same source as collector RC2 | Compressed, cached delivery of collector RC2 |

The checked root installer now pins installer RC3. README commands use its fixed
source revision, since the historical `v0.4.1/install.sh` correctly retains its
old RC2-installer/RC1-plugin pin. Existing tags are never rewritten.

Core Linux binary SHA-256: `fb2d79704b4fd31b7372484b5af1f20651906f21020ff1f42027ff9bc39dc0dd`.
Crate checksum: `0a03984be110b4b174c0c4b8ad68925695bfde950549c490cf47e2f904fb20f6`.
Installer RC3 SHA-256: `e69025454cedd659ad91d2522c7636e6c4a53c164e477ffafd2ca75d8cf17760`.

The unchanged 1,061,498,880-byte signed runtime archive is delivered through
approximately 282 MB of compressed layers. First use with bootstrap, verifier
and metadata totals approximately 444 MB; previously verified tools are cached.
Installed disk usage remains larger than compressed transfer size.

## Compatibility

The [RC2 reviewed matrix and acceptance materials](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-materials-19d7ed5-v1)
bind **Core 0.4.1** and its exact binary above. The host is Linux x86_64, glibc
2.43, kernel `7.0.0-31-generic`, with library inventory fingerprint
`f9b213877a2ea9aa44e6119e6fe2e1361f84f9a8084abd410ab8ef34b74e2872`.
Matching a distribution name or glibc version alone is insufficient.

The incremental acceptance installs RC2, verifies clean installation and
selection/rollback, then authenticates configured Core collection using the
retained original capture. Every retained runtime payload matches RC2. This
preserves the capture's original tool paths and anchors; it does not rename or
reseal old captures or claim a new native compilation. The exact 1,778-function
measurement and real numerical pass/fail controls are retained. Changed
source/tool/series identities still require explicit review.

The [original RC1/Core 0.4.0 matrix](https://github.com/musutrade/Harness-Gate/releases/download/rust-collector-review-7165558-v1/compatibility.json)
and its [native/source/bootstrap materials](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-materials-7165558-v1)
remain historical evidence. The empty development matrix is not the published
reviewed matrix. Core still owns generic trust, requiredness, thresholds,
baseline/lineage, debt/ratchet and final decisions; the optional plugin owns Rust
measurement. Installation does not provision a project's trusted state.

## Publication receipts

- Core/crates.io [publication 34706873399](https://github.com/musutrade/Harness-Gate/actions/runs/34706873399) passed; Core is already available through both channels.
- RC2 [private signing 34728799308](https://github.com/musutrade/Harness-Gate/actions/runs/34728799308) passed. [Publication 34729745154](https://github.com/musutrade/Harness-Gate/actions/runs/34729745154) was interrupted after HTTP/2 uploaded only about 39 MB in 24 minutes. HTTP/1.1 uploaded the same signed archive successfully. The original failure is preserved in the [operator continuation](quality/release-rc2/operator-continuation.json).
- The unchanged production verifier passed on both [draft](quality/release-rc2/draft-verification.json) and [public](quality/release-rc2/published-verification.json) downloads. SHA-256, RSA, Sigstore, provenance, approved packet and exact-source requirements were not bypassed.
- Installer RC3 passed the existing protected [publication workflow 34731395535](https://github.com/musutrade/Harness-Gate/actions/runs/34731395535). The public script also passed Sigstore verification and [installed RC2 on the host](quality/release-rc2/public-user-install.json). Publication uploads now select HTTP/1.1, matching the bounded range-download path.
- Original RC1's failed HTTP/2 readback and successful continuation remain in the [2026-09-12 record](quality/release-0.4.1/recovery-result.json); later success does not rewrite that run.

See the [installation/offline/lifecycle guide](quality/rust-collector-installation.md).
Private signing keys and Arc Admin business source are not part of this handoff.

## Core 0.4.2 preparation

Core 0.4.2 is being prepared to ship both verified fixes; it is not yet published.
Core 0.4.1 and RC2 remain the published tuple until the new exact binary, plugin
manifest, installer and compatibility receipts are published together. Existing
versions and assets are retained.

## Issues and Arc Admin

| Issue | Current disposition |
| --- | --- |
| [#239](https://github.com/musutrade/Harness-Gate/issues/239), [#240](https://github.com/musutrade/Harness-Gate/issues/240), [#225](https://github.com/musutrade/Harness-Gate/issues/225) | Completed RC preparation/publication; historical evidence retained. |
| [#249](https://github.com/musutrade/Harness-Gate/issues/249) | RC2 provides the reviewed Core 0.4.1 tuple and new immutable delivery. |
| [#215](https://github.com/musutrade/Harness-Gate/issues/215) | Arc Admin #40/#41 integrate all three producers and the current lockfile series. The [source-pinned candidate full run](quality/arc-native-20260913/arc-fixed-full-acceptance.json) passes 27 execution checks, collects 1,921 records and resolves the real Git baseline; quality remains fail. Released Core still needs the staged-hook and large-report fixes. |
| [#251](https://github.com/musutrade/Harness-Gate/pull/251) | Merged fix for real staged partial quality input loading. Released Core 0.4.1 fails that case; the patched candidate passes. Candidate validation is not a claim that the old binary changed. |

Completed Arc Admin work is reused: the existing 25 execution gates plus two
prelude checks and unchanged lifecycle/cryptographic negative coverage. Current
main updates chacha20 to 0.10.2, so its backend was freshly captured and re-exported
twice (1,778 mapped functions); the old lockfile evidence was correctly rejected.
The retained frontend measurement covers all
141 production TS files: 24 test files and 85 tests pass, but line coverage is
44.15% and function coverage 41.34%, below the existing 80% threshold. API
breaking-change, generation-drift and compatibility policies pass. Full quality
remains fail because of actual coverage/CRAP results. Existing `cargo flow` gates
remain in place; stable collector promotion and authority transfer are not claimed.

Released Core 0.4.1 completes the large native evaluation but fails publication
of its 72,060,628-byte quality JSON at the 16 MiB untrusted-text limit.
[PR #253](https://github.com/musutrade/Harness-Gate/pull/253) fixes the Core-owned
JSON boundary without expanding external evidence limits. The [verified candidate
report](quality/arc-native-20260913/final-report-verification.json) retains all
5,760 gate outcomes and 33 verified manifest artifacts. This candidate is not a
new published Core version; the [old binary failure](quality/arc-native-20260913/released-core-large-report-failure.json) remains recorded.
