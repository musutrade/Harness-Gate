# Tasks

- [x] T1: Record the user-approved normative policy and guard legacy publication.
- [x] T2: Inventory legacy collector/runtime/installer behavior and stable CI; define the pure-Rust replacement contract and supported measurement boundaries.
- [ ] T3: Implement pure-Rust entry, dependency checks, stable coverage collection and source complexity analysis without compiler-private APIs or Python runtime dependencies.
- [ ] T4: Implement protocol-compatible validation/normalization, preserved Core authority and explicit capability/identity failures.
- [ ] T5: Implement lightweight verified publication/installation/upgrade/rollback with user-installed dependencies and measured artifact sizes.
- [ ] T6: Run real multi-toolchain, cross-host, negative, source-boundary and old/new measurement comparisons; review the new series transition without baseline reset.
- [ ] T7: Update CI with stable-only acceptance, publish support/limitations documentation and complete all required checks.
- [ ] T8: Remove the legacy publication hold only with reviewed replacement acceptance; production publication remains separately protected.

Documentation or a prototype alone does not complete T2–T8. The policy PR closes
only T1; the implementation issue remains open until the runnable replacement,
release preparation and actual acceptance are complete.

## GH-259 candidate checkpoint

T2 inventory and implementation design: `docs/quality/stable-rust-collector.md`.
Actual candidate execution and remaining acceptance:
`docs/quality/stable-rust-collector-validation.md`. T3 has a runnable Rust capture
and AST implementation, but the second toolchain remains unverified. T4 has
a Core v2 adapter whose real signed transport and evidence checks accept verified
lexical complexity while required line/region coverage and CRAP remain blocked. The same-source
GH-220 historical probe preserves original anchors and records unequal measurement
boundaries. T5 has Rust offline dual-verification transactions and real RSA tests;
Sigstore cryptography, downloader, trusted bootstrap and protected release preparation
remain pending. T6 reviewed migration/full matrix, complete T7 acceptance
and T8 remain unchecked. No existing required binding or baseline is replaced.

The next T5 checkpoint adds a Rust-validated support schema and locked offline
unsigned candidate preparation with archive/source-authenticated dependency
notices. Real lifecycle checks consume that payload and reject unsigned packages
and re-signed overstated support metadata. This is review preparation only:
protected production signing, trust bootstrap, downloader, license review and
multi-toolchain/system acceptance remain open. T5 and T8 stay unchecked.

The T4 raw-evidence checkpoint validates the exercised public LLVM JSON structure,
integer domains, region IDs, segments and summary arithmetic with duplicate-key
rejection. Thirteen mutations of real exports now fail after test-only manifest
re-anchoring, and package preparation requires those checks. This remains a raw
format contract: aggregate/counter reconciliation, certified source owners and
normalized coverage/CRAP are not complete. T4, T6–T8 remain unchecked; required
metrics, migration review and the release hold are unchanged.

The bounded T4 owner checkpoint adds `rust-llvm-exact-root-owner/v1-candidate`.
ASCII source files containing unannotated, nongeneric root functions can join an
exact unique LLVM region envelope to a source span; explicit test spans are
excluded. Core accepts the resulting per-function execution ratio (1/1 or 0/1),
not an intra-function coverage fraction. Missing/duplicate/cross-file owners and
inconsistent entry counts are measurement errors; unsupported syntax and `impl
Trait` never acquire coverage. Line/region coverage and all CRAP remain unsupported.
Actual owner mutations and authenticated Core checks are recorded in the validation
record. T3–T8, migration review and the legacy release hold remain open.

The bounded registry checkpoint extends T3/T4 with explicit user-provided `.crate`
archive paths keyed by Cargo.lock SHA-256. Pure Rust streaming gzip/tar validation
compares every cached dependency source file with the authenticated archive and
rechecks the manifest-anchored dependency proof at collection and verification.
Cargo metadata format 1 and lock format 4 must describe exactly the same packages;
crates.io normal libraries are eligible, while registry build scripts/proc macros,
inactive lock entries, custom registries, Git and external path dependencies remain
unsupported. Runtime code does not infer Cargo's private cache layout or download
inputs. The isolated real itoa fixture and cache/archive/proof mutations extend
acceptance without changing Core thresholds, requiredness or migration policy.
T3–T8 remain unchecked pending the complete matrix and release contract.
