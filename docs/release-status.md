# Published releases and acceptance

Verified on **2026-09-13**. Core 0.4.2 and its compatible optional Rust delivery are published and installed on the acceptance host. Engineering Policy semantics are unchanged.

| Component | Published version | Source / verification |
| --- | --- | --- |
| Core | [0.4.2](https://github.com/musutrade/Harness-Gate/releases/tag/v0.4.2) and [crates.io 0.4.2](https://crates.io/crates/harness-gate/0.4.2) | `718bc5a1ea82f8364f91fc72f179ea0db689f6b5`; [release workflow 34738554209](https://github.com/musutrade/Harness-Gate/actions/runs/34738554209) |
| Rust collector | [0.1.0-rc.3](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-v0.1.0-rc.3) | Same source; [publication 34739430177](https://github.com/musutrade/Harness-Gate/actions/runs/34739430177); six signed assets, independent draft and public readbacks |
| Rust installer | [0.1.0-rc.4](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-installer-v0.1.0-rc.4) | Same source; [publication 34739962352](https://github.com/musutrade/Harness-Gate/actions/runs/34739962352); compressed, cached delivery of RC3 |

[The downloaded crate](quality/release-0.4.2/crate-public-verification.json) also matches the registry checksum, release source commit and report implementation.

Core Linux SHA-256: `eda5179d96b32269124150980ffcaa7e57878605133bc6ead51e5aeb8a6ad0a0`. Crate checksum: `0cf22fddc34242c4e6eb59369573e0ba7fe994cc13ca70f9a19ffdabddf7784e`. Installer SHA-256: `fdd277b6ec48ec89b4155d0df5d2bf6f8e6c2b15c41f0ea711cdb7eee479298a`.

The root installer at immutable source `9ffa2b829ec25ca54fcb00a9bc720284c3913924` selects installer RC4. Historical Core tags retain their original installer pins and are never rewritten. Follow the [installation and offline guide](quality/rust-collector-installation.md).

## What was accepted

The first local attempt correctly failed execution because the operator service PATH omitted Cargo; its [failed acceptance receipt](quality/release-0.4.2/missing-cargo-service-environment.json) is retained. Only the launch environment was corrected. The unchanged exact release-build Linux artifact then completed Arc Admin full and actual staged hook acceptance before delegated publication approval. After publication, the public installer verified Core SHA-256 and Sigstore and installed the identical binary. The plugin's RSA/Sigstore, exact inventory, clean offline installation, selection/rollback and authenticated configured Core collection were verified. [Public install](quality/release-0.4.2/public-user-install.json), [artifact acceptance](quality/release-0.4.2/core-artifact-acceptance.json), [draft readback](quality/release-0.4.2/draft-verification.json), [public readback](quality/release-0.4.2/published-verification.json).

## Compatibility

The [RC3 reviewed matrix and incremental materials](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-materials-718bc5a-v1) bind this exact Core 0.4.2 binary and Linux x86_64 host: glibc 2.43, kernel `7.0.0-31-generic`, library fingerprint `f9b213877a2ea9aa44e6119e6fe2e1361f84f9a8084abd410ab8ef34b74e2872`. This is exact tested ABI support, not general Linux certification.

The 1,061,498,880-byte runtime archive remains byte-identical. Compressed tools and plugin layers total about 282 MB; first use with bootstrap, verifier and metadata is about 444 MB. Verified unchanged layers are cached. New-version installation and Core binding are fresh checks; unchanged native capture and adversarial lifecycle/cryptographic receipts are explicitly reused, not relabeled as rerun. Installed disk usage remains larger than transfer size.

## Arc Admin and issues

[Full acceptance and scoped cost evidence](quality/arc-native-20260913/README.md) replace the old missing-input blocker: 27 execution checks pass, three producers supply 1,921 records, the real Git baseline resolves, all 5,760 decisions are retained and 33 report artifacts verify. Hook correctly reports full quality not_collected. Coverage/CRAP still cause actual **quality FAIL**; execution and delivery acceptance are not a quality waiver.

[#249](https://github.com/musutrade/Harness-Gate/issues/249) was completed by the earlier RC2/Core 0.4.1 delivery. The integration and documentation scope of [#215](https://github.com/musutrade/Harness-Gate/issues/215) is complete: receipts cover baseline, real full/hook, routing, environment/service and bounded original-topology cost. Arc Admin #40/#41 and [acceptance documentation PR #42](https://github.com/musutrade/arc-admin/pull/42) are merged and preserve all original cargo-flow gates. No stable collector promotion, policy weakening, automatic series migration or gate authority transfer is claimed.

## Retained history

Core 0.4.1 could not finalize the large native report and could not load staged partial host inputs. [PR #251](https://github.com/musutrade/Harness-Gate/pull/251) and [PR #253](https://github.com/musutrade/Harness-Gate/pull/253) ship both fixes in 0.4.2. The [old binary failure](quality/arc-native-20260913/released-core-large-report-failure.json) and candidate results retain their original identities.

Earlier [RC2 delivery record](https://github.com/musutrade/Harness-Gate/blob/34724396c9cec4ac14899d97a5145c8197bbfe5b/docs/release-status.md), [RC2 operator continuation](quality/release-rc2/operator-continuation.json), and [RC1 HTTP/2 recovery](quality/release-0.4.1/recovery-result.json) preserve failed runs and their independently verified continuations. Current collector uploads use HTTP/1.1 through the authorized local proxy.

[Original corresponding sources, notices and relink materials](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-materials-7165558-v1) and [per-file engineering license review](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-review-7165558-v1) remain applicable to identical payloads. The actual incremental reviewer is Codex. Private keys and Arc Admin business source are excluded from public handoff.
