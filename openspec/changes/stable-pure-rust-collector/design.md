# Design and migration requirements

## Source-level remediation and macro accuracy

Do not conflate the macro library implementation with its per-invocation generated
code. Fix dependency version, macro input and relevant cfg/features/target/build inputs.
Measure verified generated code or use explicitly validated macro measurement rules;
retain owner mapping and declared metric definitions. Obtain coverage through execution,
never infer it from source availability or from a parent invocation having executed.

For a concrete gap, produce a minimal reproducer and regression fixture, identify the
responsible layer and prepare a focused collector/dependency PR. Prefer upstream fixes;
track submission, acceptance and deployed fixed versions separately. A temporary patch
must retain source/version provenance and an exit plan. Missing stable compiler metadata
requires a tracked capability request or validated alternative, not a claim that source
access solves every case. Neither macro presence nor library popularity determines support.

Validation must include the same derive/macro source used with different invocation
inputs/configuration, generated owners with different execution outcomes, and a faulty
mapping repaired by a source-level patch. Do not change business behavior or metric
semantics solely to make a gate pass. Unresolved required measurements remain blocking.

## Shared generation and structured observations

For a diagnosed procedural-macro gap, prototype extracting generation into a normal
Rust library called by both the macro entry and the analysis integration. Prefer
optional structured observations from that same generation path over a duplicated
expansion implementation. Bind observations to dependency/source versions, invocation
inputs, cfg/features/target/build context and generated owners. Do not assume that
TokenStream text preserves hygiene, nested expansion or final coverage ownership.

The bounded Rust-only fixture at tools/quality/fixtures/rust-macro-observation proves
shared generation and execution under different invocations/features only. Production
adoption additionally requires real coverage alignment, unexecuted and ambiguous
owners, nested macros and a faulty-mapping regression. Keep missing evidence blocking.
No third-party library modifications are justified until a concrete defect is reproduced.

The stable macro span diagnostic now compares original/logging-only wrappers on
both fixture configurations. Equal generated body/interior ranges and unchanged
consumer exports narrow the source-level gap to a possible compiler span-filter
interaction; final lowered spans/contexts remain unobserved. See
`docs/quality/stable-rust-macro-observation.md` for pinned compiler source links,
actual evidence and inference limits. No third-party fix or generated coverage is
claimed, and T4 remains open.

Keep code expansion size and cognitive complexity separate from the cyclomatic
complexity used by CRAP. Do not introduce macro-stats, cargo-expand or other unstable
compiler options into official collection or required CI. Visualization and tracing
ideas may guide implementation without adding those tools as user dependencies.

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

Before activation, the authenticated staged executable must launch successfully
within the process deadline and report the exact signed version through `--version`.
This happens after both signature checks and before repeated payload/trust identity
validation. Repository automation builds a separate test-version executable to
exercise upgrade by the installed old program and rollback by the installed new
program. Its source delta is version metadata only; it is not historical release
compatibility or a replacement for the two-toolchain/two-system acceptance.

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
format contract: complete counter reconciliation, broad certified source owners and
normalized coverage/CRAP are not complete. T4, T6–T8 remain unchecked; required
metrics, migration review and the release hold are unchanged.

The earlier bounded T4 owner checkpoint added `rust-llvm-exact-root-owner/v1-candidate`.
ASCII source files containing unannotated, nongeneric root functions can join an
exact unique LLVM region envelope to a source span; explicit test spans are
excluded. Core accepts the resulting per-function execution ratio (1/1 or 0/1),
not an intra-function coverage fraction. Missing/duplicate/cross-file owners and
inconsistent entry counts are measurement errors; unsupported syntax and `impl
Trait` never acquire coverage. At that checkpoint line/region coverage and all CRAP
were unsupported; the v2 code-region checkpoint below supersedes the region limit.
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

The bounded compiler-input checkpoint extends T3/T4 with the same Rust executable
acting through Cargo's public RUSTC_WRAPPER protocol. It records unchanged compiler
arguments, pinned rustc, actual working directory and exit status, then reconciles
stable Makefile dep-info with workspace/registry source identities. External
includes and path modules fail before evidence publication; generated input bytes
are retained without certifying generated owners. Verification requires the same
producer and input sets, even after a test manifest is re-anchored. Real registry
builds establish relative-path resolution from the observed compiler cwd. This is
not arbitrary build-script/environment closure or a transitive process audit.
The validation record retains the original external-input reproduction and all
limits. T3–T8 and the publication hold remain open.

The bounded T4 aggregate checkpoint reconciles public LLVM JSON total count and
covered values against the sum of all exported file summaries, using checked
integer addition. Percentages and notcovered retain their independent arithmetic
checks. LLVM 22.1.6 CoverageExporterJson::renderRoot and
CoverageReport::prepareFileReports define this relation. Eleven internally valid
but mutually inconsistent mutations of real exports are required acceptance
cases. This does not certify region/segment counter semantics, broaden function
owners or enable intra-function coverage/CRAP. T3–T8 remain open.

## Bounded code-region checkpoint (T4 remains incomplete)

`rust-llvm-exact-root-owner/v2-candidate` extends the previous execution-only
checkpoint with standard LLVM code-region coverage for the same exact root
owners. Each distinct code-region span contributes one denominator entry; only
its own nonzero counter contributes to covered. Duplicate spans and disagreement
between all owner regions (including excluded tests) and the file summary fail
measurement. Source activation/expansion boundaries do not broaden. Core receives
`coverage.region` alongside lexical complexity and `coverage.function`.

The real partial fixture exercises function execution 1/1 with region coverage
5/6, plus an exported never-called owner at 0/1 and 0/3. Test code contributes to
raw summaries but never to these production-owner ratios. This supersedes the
execution-only checkpoint's region-unsupported statement; line coverage and all
CRAP remain unsupported. The candidate does not select Core's CRAP model, change
requiredness/thresholds, or authorize migration/baseline adoption. LLVM region
coverage is not the historical MIR basic-block metric. T3–T8 remain open.


## Bounded T5 HTTPS transport

The Rust `download-install` entry point takes a caller-pinned strict request listing
only five assets with HTTPS URLs, SHA-256 and exact byte counts, plus separately
pinned host signing trust. ureq/rustls performs bounded streaming, verified TLS and
at most three validated absolute HTTPS redirects. The host may explicitly pin a
private PEM root; a release cannot supply trust. No archives, interpreter, toolchain,
latest-version discovery, unsigned fallback or dependency installer is involved.

The installation lock spans network transfer, existing dual signature/support/
launch checks and atomic selection. Each asset has a 1–120-second overall deadline;
program/metadata caps are 64/8 MiB. The five assets consume at most 96 MiB before
verification. Exact request/TLS/trust identities are rechecked. Received application
body bytes, cache absence and abandoned interrupted staging are recorded separately;
network framing is not claimed as measured. Failures preserve the active version.

Real loopback TLS and test-RSA lifecycle fixtures validate this transport. Mocked
Sigstore cannot satisfy production signing acceptance. Trusted public bootstrap,
protected publication, two toolchains/systems and complete acceptance still block
T5/T8; no release hold is removed by the downloader.

## T7 execution trace completeness

The repository-only required audit consumes untimestamped `strace -f` execve
records. It joins unfinished/resumed calls by process ID before checking decoded
arguments for forbidden runtimes/interfaces. Abbreviated, unreadable, unmatched,
duplicate or incomplete records fail the audit; signal/exit records are allowed
without discarding pending calls. The default environment pointer/count is not
environment-content evidence. Synthetic parser regressions cannot establish
actual process tracing or cross-host acceptance; T7 remains incomplete.

## Inline-module owner checkpoint (T4 remains incomplete)

`rust-llvm-exact-free-owner/v3-candidate` extends exact-span matching to ordinary
free functions inside unannotated inline modules. Source scope qualification
distinguishes repeated basenames; every source owner still needs one unique,
single-file LLVM owner and reconciled code-region counters. Annotated/external
modules, impl/trait scopes, cfg, generics and expansion remain uncertified.
The actual fixture distinguishes two same-named functions with different
execution/region results and a nested never-called function; test spans are
excluded explicitly. Missing/duplicate owner and positive-count inheritance
mutations fail measurement, while compilable annotated/cfg variants return
unsupported. The rule version changes; metric definitions, Core authority and
migration requirements remain unchanged. The new binary-bound normalization
produces a different full Core measurement-series identity. Signed support
metadata must name this rule; no existing baseline compatibility is assumed.

## Strict identity decoding

The candidate uses one recursive duplicate-key validator before decoding request,
manifest, doctor and dependency archive maps into Rust types. Typed Serde map
decoding alone silently retained the last identity for repeated paths; a locally
captured, test-reanchored manifest reproduced this acceptance. Cargo metadata
passes the same validator on capture and verification. SHA authentication remains
required and does not excuse ambiguous JSON. Test-only re-anchoring isolates this
semantic defect; it is not signature forgery or producer-authentication evidence.
Both key orders must fail, and ambiguous collection input must fail before output
creation. Package preparation requires those actual CLI regression observations.
