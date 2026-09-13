# Design and migration requirements

Our installable collector, dependency doctor, process orchestration, evidence
validation/normalization and user-facing lifecycle implementation must all be Rust.
Repository-only release/test automation may retain existing languages; it must not
be shipped as an interpreter dependency or invoked by the installed plugin.

Use public stable coverage instrumentation and matching external LLVM/cargo-llvm-cov
interfaces. Analyze source syntax with a maintained Rust parser. Do not substitute
nightly tools, parse unstable MIR text, or hide rustc_private behind another executable.
External tools must expose supported public interfaces; preview distribution status
and LLVM format/version limitations must be documented rather than misrepresented.

Choose and document explicit source boundaries for macro expansion, derives, async,
closures, cfg/features, generated code and unexecuted functions. Function-level CRAP
is supported only where complexity and coverage have demonstrably identical owners
and scope. Failed or uncertain mapping cannot become zero, inherited parent coverage,
or an optimistic skip. New measurements get a new series and independent evidence.

Keep policy authority in released Rust Core. Preserve signed adapter requests,
artifact inventories, source/config/tool identities and all failure semantics.
Use existing evidence to characterize differences, not to claim equivalence by renaming
series. Compare stable and historical measurements explicitly before reviewed adoption.
Historical replay may run separately; required CI must not build/run the unstable driver.

Publish per-target binaries and necessary resources with existing verification,
atomic install/update and rollback guarantees. List required externally installed tools
and actionable missing/incompatible dependency commands. No forced global toolchain
changes, bundled compiler/Python default, routine user download of review archives,
or publisher OS/kernel fingerprints. Do not promise platform support without actual
native tests. Rust version changes require supported-interface/format validation,
not unconditional full toolchain redistribution or baseline reset.

## Implementation checkpoint

The detailed legacy inventory, executable/dependency contract, exact source
boundaries, Core v2 integration design and lifecycle plan are recorded in
[the candidate design](../../../docs/quality/stable-rust-collector.md).
[Validation](../../../docs/quality/stable-rust-collector-validation.md) records
actual results separately from pending acceptance. This checkpoint does not
authorize a required-series replacement or release-hold removal.

The candidate now implements signed-argument capture bindings and generic Core
evidence for verified lexical complexity, with explicit unsupported coverage/CRAP.
Actual Core acceptance uses a test-only key and preserves required-risk blocking.
The [historical comparison](../../../docs/quality/stable-rust-collector-migration.md)
uses original source bytes and anchored report members without running the old
backend or changing historical evidence. This remains partial T4/T6 acceptance.

Rust offline release verification, installation, upgrade and rollback now implement
the [lifecycle contract](../../../docs/quality/stable-rust-collector-lifecycle.md).
The local suite uses real RSA signatures and an explicitly mocked Sigstore command;
it does not establish production trust or T5 completion. No release hold is removed.

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
