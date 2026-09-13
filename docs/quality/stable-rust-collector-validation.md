# GH-259 candidate validation record

This is a partial implementation checkpoint, not release acceptance. T2 is
complete; T3–T8 remain open. The [design](stable-rust-collector.md) defines the
boundary. No Core required binding, threshold, baseline or historical evidence
was changed. No compiler-private backend was built or executed for these results.

## Compiler dep-info output identity checkpoint

A real preceding capture accepted an added `env-dep` comment after a test-only
outer manifest re-anchor. The parser verified the file-input set but ignored
comments, and the compiler invocation had no separately observed output digest.
The original proof and manifest were restored byte-for-byte. This reproduced an
internal output-consistency defect, not a producer-signature bypass.

The Rust compiler wrapper now hashes each successful dep-info output immediately
after the unchanged compiler command completes. Capture and verification compare
the complete raw bytes against that producer record. Real `env!("CARGO_PKG_NAME")`
output supplies the positive case. Added, changed and omitted environment comments,
and missing or inconsistent producer identities, all block. Package preparation
requires all five new regressions. [The evidence](stable-rust-candidate-evidence/dep-info.json)
records before/after binary identities, exact commands, log hashes and limitations.
This authenticates observed bytes within the capture; environment semantics,
arbitrary build-script reads, environment closure and adversarial build isolation
remain uncertified.

The stable 1.97.1 release SHA256 is
`62feafa4831df83843fa71e7384d15c7bd79fb6c679525bcccfb853d47607409`,
3,496,016 bytes. Candidate tests pass 22; formatting and all-target clippy pass.
[125 real capture checks](stable-rust-candidate-evidence/acceptance-dep-info.json),
26 shared-generator macro checks, historical comparison and six authenticated Core
fixtures pass against this exact executable. Quality discovery passes 449 with
36 manual legacy skips; release discovery passes 91. Unchanged Core source and
dependencies retain the prior 397-test nextest, formatting and clippy evidence.
The two project-local gate commands remain not applicable because this checkout
has no `.harness-gate/flow.toml` declaring a `ci` profile.

Documentation consistency initially failed because the inherited Cargo target
`/home/gem/cargo-target` was read-only; a direct Cargo probe retained the
`.cargo-build-lock` error. The rerun passes using this workspace's existing Core target.

Package preparation, 38 lifecycle checks and 29 loopback HTTPS checks also pass.
The unsigned payload is 4,809,626 bytes; initial and upgrade test bundles transfer
4,810,095 and 4,810,121 bytes. Two installed versions occupy 9,620,216 bytes;
interrupted staging and metadata bring the installation to 14,430,965 bytes.
The HTTPS matrix receives 37,607,035 response-body bytes across successful and
failed transfers, with zero cache and observed abandoned downloads. Headers, TLS
overhead and unwritten audit from the killed process are excluded. RSA signatures
and loopback TLS use real test trust; Sigstore is mocked. The package excludes
acceptance archives and external toolchains and is not production-ready.

Only Rust 1.97.1 / LLVM 22.1.6 on Ubuntu 26.04 Linux x86_64 has actual acceptance.
The second compiler/system environments, full process tracing, authenticated
generated-function coverage, reviewed CRAP/migration and production signing remain
incomplete. No support claim, threshold or baseline was expanded. T3–T8 remain
unchecked and release stays paused.

## Preceding output-directory isolation checkpoint

The preceding release rejected output inside the measured project only after
creating its directory. Real CLI reproductions retained five empty directories:
three doctor paths and two collection paths using a symlink or `..`. Direct
collection paths already failed before creation. File-only identity checks did
not expose these empty directories. The [output-isolation evidence](stable-rust-candidate-evidence/output-isolation.json)
preserves both original observations and fresh post-fix directory/file snapshots.

The Rust runner now resolves the project and existing output parent, checks the
boundary, then creates the output. Six rejected doctor/collect paths leave the
measured tree unchanged; a relative sibling output still succeeds. These seven
checks are required by package preparation and independently tested for omission.
This covers path resolution before creation; concurrent filesystem replacement
is not certified.

That checkpoint's final stable 1.97.1 release is
`adeaf2d1a53e182f810097161c93ec83d8b32bcc5afefd9ec67ed20f87402ed3`,
3,488,488 bytes. It passes [120 capture checks](stable-rust-candidate-evidence/acceptance-output-isolation.json),
26 macro checks, six authenticated Core fixture cases, 38 lifecycle checks and
29 loopback HTTPS checks. Package preparation passes with this exact program.
Candidate tests pass 21, formatting and all-target clippy pass, quality discovery
passes 449 with 36 manual legacy skips, and release discovery passes 91.
Unchanged Core source/dependencies retain the preceding 397-test nextest,
formatting and clippy evidence. The initial formatter failure and package
identity rejection after that formatting change are retained; the full runtime
matrix was rerun against the final binary.

The unsigned four-file payload is 4,802,098 bytes. Signed test bundles download
4,802,567 bytes initially and 4,802,593 on upgrade. Two installed versions occupy
9,605,160 bytes; interrupted staging adds 4,802,593, totaling 14,408,381 including
installation metadata. The HTTPS matrix receives 37,554,339 response-body bytes
across successful and failed transfers; cache and observed abandoned-download
bytes are zero. These exclude headers, TLS overhead and unwritten audit from the
killed process. RSA and loopback TLS use real test trust; Sigstore is mocked.

Only the existing Rust 1.97.1 / LLVM 22.1.6 Ubuntu 26.04 Linux x86_64 environment
was exercised. Second-toolchain/system validation, complete process tracing,
generated-owner/input certification, production signing and reviewed migration
remain incomplete. T3–T8 remain unchecked and release remains paused. The
following records preserve earlier binaries and their separate evidence.

## Duplicate identity decoding checkpoint

A fresh plain-fixture capture reproduced a concrete collector defect: when its
manifest listed `candidate.json` twice with different identities, typed map
decoding retained the last entry and verification succeeded. The test explicitly
supplied the changed manifest's SHA anchor. This demonstrates ambiguous decoding,
not forged producer authentication. The same pinned bytes now fail with
`measurement_error: duplicate key`; the original capture manifest was restored.
Both binaries and exact input/command identities are recorded in
[the JSON checkpoint](stable-rust-candidate-evidence/json-identities.json).

Requests, manifests, doctor reports, archive maps and captured Cargo metadata now
pass recursive duplicate-key validation before typed decoding. Eight actual CLI
regressions cover both key orders in manifest files, captured request source
identities, incoming collection requests and archive identities. Incoming
ambiguous requests fail before output creation. Package preparation requires all
eight cases; its unit regression also rejects each missing case independently.

The initial release build using the `stable` alias produced
`dcb1902a07ee2e3213f8a103b0419f1c71b9685dd42840543937b65709d2713d`.
Release preparation's sanitized explicit-1.97.1 build produced a different binary,
`8c925c2cf93b39990a93add07de000ef08081f7b07becd529cf3cf602e5261ff`;
its mismatched acceptance record correctly blocked preparation. Both compiler
queries report the same 1.97.1 commit and LLVM 22.1.6. The difference is not
counted as a second supported toolchain, and no reproducibility across these build
invocations is claimed. Fresh runtime validation uses the latter exact program.

That final 3,486,656-byte program passes 113 capture checks, 26 macro checks,
six authenticated Core fixture cases, 38 lifecycle checks and 29 real loopback
HTTPS checks. Package preparation passes with that exact binary identity. The
[113-case capture summary](stable-rust-candidate-evidence/acceptance-json.json)
is separate from all prior checkpoint records. Candidate unit tests pass 20;
Core nextest passes 397; both crates pass formatting and all-target clippy.
Quality discovery passes 449 tests with 36 manual legacy skips. Release discovery
passes 91 after correcting its stale acceptance fixture; the initial expected-case
failure is retained. The final package-specific six tests also pass.

The unsigned payload is 4,800,266 bytes. Signed test bundles download 4,800,735
bytes initially and 4,800,761 bytes on upgrade. Two installed versions occupy
9,601,496 bytes; interrupted staging adds 4,800,761 bytes, for 14,402,885 bytes
including installation metadata. The HTTPS matrix receives 37,541,515 response-body
bytes across successful and failed transfers; cache and observed abandoned-download
bytes are zero. These figures exclude headers, TLS overhead and unwritten audit
from the killed download process. RSA verification and loopback TLS are real with
repository test trust; Sigstore is mocked and production signing is unverified.

ELF inspection records the final executable and its system library dependencies.
Command logs record actual validation invocations, but unavailable process tracing
prevents a complete child-process audit. Only Rust 1.97.1 / LLVM 22.1.6 and the
existing Ubuntu 26.04 Linux x86_64 environment were exercised. Second-toolchain and
second-system execution, complete generated-owner/input certification, real
production signing and reviewed migration remain incomplete. T3–T8 stay unchecked
and release remains paused.

## Inline-module owner checkpoint

The release binary at this checkpoint was
`6addb9cf8c716304464f756c3f6e37e402bf65823cb0ba418f610572770e6701`,
3,475,840 bytes, built with stable Rust 1.97.1. Package preparation reproduced
that digest. The previous binary and this one compiled and measured the exact
same fixture source/manifest/lock identities. The previous owner rule returned
unsupported for all three functions; the v3 rule now verifies them independently:

| Source owner | Lexical complexity | Executions | Function coverage | Code-region coverage |
| --- | --- | --- | --- | --- |
| `left::classify` | 2 | 1 | 1/1 | 4/5 |
| `left::nested::never_called` | 1 | 0 | 0/1 | 0/3 |
| `right::classify` | 2 | 2 | 1/1 | 5/5 |

Exact spans and qualified source names distinguish repeated basenames. Missing
owners, duplicate owners and positive region counts on the unexecuted function
fail verification. Those three negative checks mutate actual LLVM exports and
re-anchor the test manifest to exercise semantic verification; they are not
claims of compiler-generated corrupt data. Real annotated/cfg module variants
compile and execute but remain unsupported. Two explicit test-module spans are
excluded from production owners. Core accepts all three authenticated records
and blocks required CRAP, as well as identity, signature, expiry and replay errors.

That checkpoint executable passed 20 Rust tests, 105 generic capture checks, 26 macro
checks, six authenticated Core fixture cases, 38 local lifecycle checks and
29 real loopback HTTPS checks. Core nextest passes 397 tests; candidate/Core
formatting and all-target clippy pass. Quality discovery passes 449 tests with
36 explicitly manual legacy skips; release discovery passes 91 tests. Exact
commands, environment adjustments, source/binary identities and original log
anchors are in the [module evidence](stable-rust-candidate-evidence/module-owners.json).
The [capture summary](stable-rust-candidate-evidence/acceptance-modules.json)
retains all 105 check results. CI and package preparation require the new cases.

The unsigned four-file payload is 4,789,450 bytes. Signed test bundle downloads
are 4,789,919 bytes initially and 4,789,945 bytes for a separately compiled
upgrade. Local installation retains 9,579,864 bytes for two versions, plus
4,789,945 bytes from interrupted staging, totaling 14,370,437 with metadata.
The separate HTTPS matrix reads 37,465,803 response-body bytes, including failed
transfers; headers, TLS/proxy overhead and the killed process's unwritten audit
are excluded. Observed abandoned download bytes and download cache are both zero.
TLS and test-key RSA are real; Sigstore is mocked. The local lifecycle raw
summary retains an obsolete descriptive “downloader not implemented” note; its
zero network bytes describe that local-input matrix. The automation note is now
corrected, and the separate HTTPS record proves the download behavior.

Metric definitions are unchanged, but the new binary-bound Core normalization
creates a different full measurement-series identity. No baseline compatibility,
CRAP model selection or historical install/upgrade compatibility is implied.
Only the existing Rust 1.97.1/LLVM 22.1.6 Ubuntu 26.04 Linux x86_64 environment
was exercised. The second toolchain/system, actual process tracing, real Sigstore,
complete generated-owner/input certification and reviewed migration remain
unverified. T3–T8 and the release hold remain open. The records below describe
previous checkpoints and preserve their original binary and evidence identities.

## Execution audit parser checkpoint

The required CI audit previously skipped resumed `execve` lines and accepted
truncated or unfinished calls. Synthetic records reproduce all three gaps. The
repository audit now reconstructs calls by PID, requires complete executable and
argument strings, decodes escapes before checking forbidden tools/flags, and
rejects unmatched, duplicate, unreadable or incomplete records. Six focused tests
cover those failures, interleaved processes, normal signals, failed executable
lookups and literal ellipses. The [regression evidence](stable-rust-candidate-evidence/execution-audit.json)
records the original acceptance and corrected rejection.

The full quality suite passes 449 tests with 36 manual legacy skips. After the
final separator-whitespace refinement, all six affected parser tests pass again.
Docs consistency passes. Unchanged Core/release/runtime checks retain their
preceding HTTPS checkpoint evidence.

This validates the parser only. No actual process tracing became available, and
strace's default environment pointer/count does not reveal environment contents.
The Rust binary, package and previous runtime results below are unchanged. T3–T8
remain incomplete; second-toolchain/system, real process tracing/signing, complete
input/owner certification and reviewed migration still block release acceptance.

## HTTPS lifecycle checkpoint

The candidate now implements `download-install` in Rust using pinned ureq/rustls.
The preceding executable rejects the same complete invocation; its original
digest and error are retained in the [HTTPS evidence](stable-rust-candidate-evidence/https-download.json).
The new stable release is `b876d72b536ff2a809bea7da62a7d4e1b33a670666cca99438462e456f3e3b95`,
3,476,032 bytes. Its exact five-asset request is separately pinned, bounded and
downloaded over verified HTTPS before the existing signature and activation checks.
No release discovery or trust bootstrap is implied. The
[lifecycle contract](stable-rust-collector-lifecycle.md#explicit-https-download-and-installation)
defines redirects, deadlines, host trust and interrupted staging.

Real loopback HTTPS acceptance passes 29 checks: installation, distinct compiled
upgrade, rollback, certificate/hostname failures, HTTP/redirect/length/encoding/hash
failures, timeout, truncation, signature failure and SIGKILL. Request and TLS-root
changes during transfer also fail before any verifier/launch command, despite all
five downloaded assets having matching hashes. Every failed update preserves the
previous selection and executable digest. TLS uses an actual test CA/server leaf;
RSA uses real test signatures; Sigstore is explicitly mocked. Neither public
hosting nor production signatures have been accepted.

The unsigned four-file payload is 4,789,642 bytes. The signed test bundle downloads
4,790,111 bytes initially and 4,790,201 bytes for the independently compiled
upgrade. The network matrix reads 37,467,211 response-body bytes including failed
partial transfers; HTTP headers, TLS/proxy overhead and the killed process's
unwritten audit are excluded. The killed download leaves zero file bytes in this
run; it can leave bounded unselected files in other executions. Download cache is
zero. The separate local lifecycle matrix retains 9,580,312 bytes for two versions,
4,790,201 interrupted staging bytes, and 14,371,141 bytes including installation
metadata. Compiler/build caches, test keys and acceptance files stay outside the
release payload. The new TLS dependencies' archives, source identities and notices
passed package preparation; production license review remains pending.

The final binary passes 19 Rust tests, 92 real generic capture checks, 26 macro
checks, five authenticated Core fixture cases, and 38 local lifecycle checks.
Core nextest passes 397 tests with only test-process proxy variables removed;
quality discovery passes 444 tests with 36 manual legacy skips; release discovery
passes 91 tests. Candidate/Core formatting and clippy pass. Exact commands and
original log identities are retained in the HTTPS evidence. ELF inspection lists
only libc/libgcc runtime dependencies; it does not replace the transitive process
trace, which remains unavailable locally and required in CI.

Earlier failed invocations are retained: explicit empty compiler flag variables
caused one Rust runtime test to reject its environment; unsetting those variables
passed without weakening the test. The CI step had inherited the same empty
release-build flags: its unit command now unsets those variables, and the runtime
phase unsets them after the traced build. The exact unit command under the CI
parent environment passes all 19 tests; runtime environment rejection remains
strict. The repository shell installer integrity suite also passes. The first TLS fixture incorrectly used a CA
certificate as its leaf and was rejected; a separate leaf fixed the fixture.
An initial Core harness invocation omitted the capture directory and returned a
usage error; the corrected five invocations passed. The preceding 27-check network
run used older signed fixtures and is superseded by the final 29-check run.

Rust 1.98.1, a second runnable Linux system, actual process tracing and real
Sigstore acceptance remain unavailable as documented below. Generated-owner/CRAP
certification and reviewed migration remain unfinished. T3–T8 stay unchecked and
the release hold stays active. Subsequent sections retain historical checkpoint
identities and are not acceptance for this new executable.

## Macro span diagnostic follow-up

The [stable span diagnostic](stable-rust-macro-observation.md#stable-span-diagnostic)
ran eight commands, including four real test/coverage runs with original and
logging-only wrappers in both configurations. Generated body/interior ranges
coincide; the two consumer test-function exports are identical before/after
logging. Source inspection suggests a compiler coverage span-filter interaction,
but final internal spans/contexts remain unobserved. No upstream fix or generated
coverage is claimed. The diagnostic uses explicit existing matching LLVM tools;
the initial missing-component installation failure and subsequent pre-Cargo
missing-tool rejection are preserved in the
[diagnostic evidence](stable-rust-candidate-evidence/macro-span-diagnostic.json).
Plugin source, linked generator and release binary are unchanged from the next
section. T4 and release acceptance remain incomplete.

## Shared-generator checkpoint

That checkpoint's executable adds the [bounded macro source observer](stable-rust-macro-observation.md),
sharing our ordinary Rust generator with the actual procedural macro. Its release
SHA-256 is `8227f231adea15a433f194e67bfd45b0e2647911e628cece5f8b54a234b94f43`
and its size is 2,084,496 bytes. The [macro regression record](stable-rust-candidate-evidence/macro-observation.json)
contains 26 successful checks and the actual consumer LLVM function records for
default and branching configurations. Both configurations execute generated code,
but the export has only two consumer test functions and no generated business
function record. No zero coverage or CRAP is inferred. Authenticated macro capture
separately remains blocked on registry build-script/proc-macro input certification.

The final binary passed 92 generic capture checks in `target/gh-259/acceptance26/`
and five authenticated Core fixtures in `target/gh-259/core-*-18/`. The unsigned
four-file package is 3,215,130 bytes (`target/gh-259/package13/`); it includes the
same 1,129,147 bytes of notices, 924 bytes of support metadata and 563-byte inventory.
It carries no fixture source, test automation, compiler or evidence archive.
The source/build inventory now binds the linked first-party generator and model
manifests/lock. Production signature, license and cross-environment acceptance
remain pending. The summaries below describe the preceding checkpoint and retain
their original binary/artifact identities; they are not evidence for this binary.

Validation for the new checkpoint: 19 Rust unit tests, four fixture workspace tests,
26 macro checks, 92 generic checks and all five Core fixtures passed. The repository
Core nextest suite passed 397 tests with only proxy variables removed from its
process environment; no Core behavior changed. Quality unittest discovery reports
444 tests, OK with 36 explicitly manual legacy skips. Release tests report 91 OK.
Core/candidate clippy and formatting checks passed. Full commands, current byte
measurements and remaining limitations are indexed in the
[checkpoint evidence](stable-rust-candidate-evidence/macro-checkpoint.json).
The lifecycle suite passed 38 checks using test RSA keys and mocked Sigstore.
Its signed test packages are 3,215,599 bytes initially and 3,215,705 bytes for the
separately compiled upgrade. Two installed versions occupy 6,431,304 bytes;
including interrupted staging and installation metadata the measured footprint
is 9,647,637 bytes. Actual network download bytes are zero: this is local lifecycle
validation, not a production download or signature acceptance claim.
Local process tracing remains unavailable; CI requires actual tracing. Rust 1.98.1,
a second runnable Linux environment and real Sigstore acceptance remain unverified.
No project `.harness-gate/flow.toml` exists: config check and verify ci are not applicable.

## Preceding environment and artifact checkpoint

The same release binary ran all collection and Core checks on Ubuntu 26.04, Linux x86_64
GNU, glibc 2.43. The host kernel is recorded only as an observation, not an
installation fingerprint. Rust 1.97.1 reports commit
`8bab26f4f68e0e26f0bb7960be334d5b520ea452`, LLVM 22.1.6;
matching external LLVM tools report `22.1.6-rust-1.97.1-stable`.
External cargo-llvm-cov is 0.9.0. The actual export format is LLVM JSON 3.1.0.

The candidate binary is **2,002,000 bytes**, SHA-256
`e2e3296e90b8da095bfe98f504589faca27ade64565262697747567183cd3e6c`.
ELF `NEEDED` entries are `libgcc_s.so.1` and `libc.so.6`; no Python or compiler
private shared library appears. This observation alone is not a transitive
process audit. The plain capture occupies 48,587 bytes. Its temporary Cargo
build directory is removed and persistent candidate cache is zero bytes.
The actual unsigned candidate package occupies 3,132,634 bytes: program 2,002,000,
collected licenses 1,129,147, support metadata 924 and inventory 563 bytes. The
[preparation record](stable-rust-candidate-evidence/preparation.json) binds the
locked build, 74 registry dependency notice inventories, Rust library notices and
acceptance for that exact executable. It retains conservative build-dependency
notices; production license review remains open. Full notices and build logs are
in the workspace review directory, without repackaged raw dependency archives.

The local test-only signed package and one installed version occupy 3,133,103
bytes each. Two retained versions occupy 6,266,248 bytes. Interrupted staging
occupies 3,133,145 bytes; the exercised root totals 9,400,021 bytes including
verification metadata. Local input operations downloaded zero bytes; a network
downloader is not implemented. The prepared package is deliberately unsigned;
the lifecycle package uses test RSA and mocked Sigstore metadata. These sizes are
actual candidate measurements, not a production archive/download claim. Build
targets and acceptance logs are not runtime payloads.

The checked-in [acceptance summary](stable-rust-candidate-evidence/acceptance.json)
contains actual tool hashes, binary identity, all 92 checks, raw totals and
capture/request anchors. [Environment probes](stable-rust-candidate-evidence/environment.json)
record exact commands, errors and outputs. [ELF dependencies](stable-rust-candidate-evidence/dynamic-libraries.txt)
record the binary inspection. The [log identity index](stable-rust-candidate-evidence/log-identities.json)
records hashes and sizes of retained workspace logs. These are new candidate observations, not repackaged
historical evidence. The separate [plain](stable-rust-candidate-evidence/core-plain.json),
[partial](stable-rust-candidate-evidence/core-partial.json),
[boundaries](stable-rust-candidate-evidence/core-boundaries.json) and
[features](stable-rust-candidate-evidence/core-features.json) and
[registry](stable-rust-candidate-evidence/core-registry.json) summaries record
authenticated Core acceptance using an explicitly test-only key, not production signing. Full captures/logs remain
under `target/gh-259/acceptance-24/` in the execution workspace; the summary is
not a standalone verifiable capture archive. CI uploads its own complete captures.

## Aggregate consistency checkpoint

The [reproduction](stable-rust-candidate-evidence/aggregate-reproduction.json)
records exit 0 from the preceding binary when a real export's line total was
changed from 9/12 to 1/1. Only the test manifest was reanchored; originals were
restored. The Rust verifier now checks count and covered totals against every
exported file summary with overflow rejection. Individual percentages and
notcovered counts remain independently validated. This follows LLVM 22.1.6's
[JSON exporter](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.6/llvm/tools/llvm-cov/CoverageExporterJson.cpp)
and [file report aggregation](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.6/llvm/tools/llvm-cov/CoverageReport.cpp).

Eleven new real-export mutations preserve each summary's internal arithmetic
while changing totals or file summaries; all must fail verification. Package
preparation requires these cases. A Rust regression covers multiple-file sums,
covered/count disagreement and integer overflow. These checks establish summary
consistency only. At that checkpoint intra-function coverage remained unsupported;
the v2 checkpoint below adds bounded code-region coverage. Full segment/counter
reconciliation and CRAP remain incomplete.

## Real fixture observations

Plain and boundary fixtures are dependency-free Cargo projects checked into
`tools/quality/fixtures/rust-stable`. They compile and test using the project's
selected stable toolchain. Each capture records subprocess arguments, environment,
stdout/stderr, exit status and hashes. Positive collection does not use mocks.

| Capture | Raw lines | Raw regions | Raw functions | Source observation |
| --- | --- | --- | --- | --- |
| Plain | 9/12 | 12/15 | 2/3 | `classify` lexical CC 3; `never_called` CC 1 and actual zero execution |
| Boundaries, default features | 16/18 | 30/37 | 6/8 | Async constructor executes once, body zero; closure/generic/macro owners unsupported |
| Boundaries, `extra` feature | 18/20 | 35/42 | 7/9 | Feature function appears in raw export; lexical cfg owner remains unsupported |

Raw coverage is test-inclusive, while source analysis lexically excludes tests.
These denominators must not be joined for CRAP. The boundary fixture also
executes macro-expanded and build-generated functions, derives traits and leaves
an async future unpolled. Its assertions inspect known fixture symbols solely
to establish observations; that suffix matching is not a certified production
owner mapping. Uninstantiated generic ownership remains unproven. The exact root-owner rule separately certifies plain function execution ratios
(1/1 and 0/1) after excluding explicit test spans. All candidate function CRAP
and normalized line coverage remain unsupported. The v2 owner rule additionally
certifies code-region ratios as documented below.

The 92 actual checks include valid capture/integrity verification, stale output,
wrong request/manifest anchors, damaged and mixed artifacts, changed source,
restored source, boundary/features captures, wrong tool identity, injected Cargo
configuration, missing tools, inherited compiler flags, unavailable project pin,
symlink source and a real failing test subprocess. These checks confirm the
implemented failure boundaries; the separate real Core acceptance below exercises signature/replay protection.
The raw LLVM validator previously accepted a negative function execution count
when the test manifest was deliberately re-anchored; the [reproduction](stable-rust-candidate-evidence/coverage-reproduction.json)
records exit 0 from the preceding binary (`f4f71cc58f51194ab4b9c9c976bd34360ddb0bf471c414f7c7439949353ee81e`).
This exposed a semantic check missing after integrity verification, not a forged
signature. The current release rejects that input. Thirteen additional checks
mutate actual LLVM output, update the test-only manifest anchor, and assert
nonzero failure: negative/overflow/fractional counts, missing regions, invalid
file IDs, reversed spans, unknown region kinds, impossible coverage totals,
inconsistent percentages/notcovered, invalid segment booleans, MC/DC capability
and duplicate object keys. Both original files are restored after each suite.
Unsigned package preparation requires these named checks in the pinned acceptance.

These checks validate the exercised JSON format, scalar domains and local summary
arithmetic. The aggregate checkpoint below reconciles file totals; the v2 owner
checkpoint additionally reconciles eligible file region summaries with owner counters.
This does not certify every LLVM layout or semantic relation. Nonempty branch/expansion
layouts have structural checks but no real producer acceptance in this matrix;
MC/DC is explicitly unsupported. Required line coverage and CRAP stay blocked; candidate code-region support is
limited to the exact owners documented below.

The exact root-owner validator additionally rejects seven real-export mutations:
missing LLVM function, duplicate symbol, multiple records for one source function,
cross-file owner, unknown source span, inconsistent entry count and executed
regions under a zero-count function. Two further checks omit the source function
inventory or alter excluded test spans after test-only manifest re-anchoring; both
fail because the verifier recomputes the complete source analysis. Together with
the positive owner description these bring the suite to 46 checks. Unexecuted
functions require their own actual zero-count record. No missing owner is filled
with zero, and no function borrows its parent's execution count. The rule is
limited to ASCII, unannotated, nongeneric root functions with exactly one LLVM
record and exact region envelope. Opaque/argument-position `impl Trait`, Unicode,
modules, attributes and unresolved expansion/activation remain unsupported.
The binary digest in Core normalization identity distinguishes this checkpoint
from earlier candidate evidence; no existing series is silently upgraded.

The [registry acceptance record](stable-rust-candidate-evidence/registry.json)
added 13 checks; compiler-input checks brought the suite to 74; aggregate reconciliation adds 11 for 85. The preceding binary rejected the real
locked `itoa` fixture before implementation. This binary compiles/tests it offline,
certifies its root owner, and authenticates 14 cached package files against the
explicit `.crate` checksum. Changed archives, modified/extra/symlinked cached files,
missing archives, missing proof packages and omitted metadata packages all fail.
The poisoned-cache case also fails before a successful collection manifest exists.
Test-only manifest re-anchoring exercises semantic proof validation; no signature
is forged. Tests mutate and restore only the isolated workspace cache.

That fixture cache occupies 134,385 bytes
(15,935 archive, 51,103
extracted files, 9,826 index); its capture occupies
64,718 bytes. These are user dependency/test inputs, separate
from the plugin's zero-byte persistent cache and four-file release payload. Test
setup copies the already provisioned crate locally; zero network bytes were fetched.
Observed external `include!`/path inputs now block capture, as described below.
Arbitrary build-script reads and environment closure remain uncertified. Registry build scripts/proc macros, inactive lock packages, Git and
custom registries are rejected; this is a bounded package provenance implementation.

The [compiler-input record](stable-rust-candidate-evidence/compiler-inputs.json)
retains a real preceding-binary reproduction: an external `include_bytes!` file
changed after capture, yet verification returned exit 0. The new collector checks
observed stable dep-info against authenticated workspace/registry files. Three
real captures using external `include_bytes!`, `include!` and `#[path]` fail before
emitting a manifest. A project-internal data file is captured and verified.
Re-anchored empty proofs, missing inputs, Make expressions, wrong compiler cwd
and unmatched producer sets fail. Generated source bytes from the boundary fixture
are retained by hash while their owners remain unsupported. These add 15 command
checks, bringing that checkpoint to 74 total. Compiler wrapper records include actual rustc arguments,
working directory and exit status; they do not prove arbitrary build-script reads,
environment completeness or transitive process tracing.

The first dep-info implementation rejected the real registry's relative
`src/lib.rs` as ambiguous (`acceptance-17.log`). The public Cargo wrapper supplies
the actual producer cwd, resolving that failure. `acceptance-18.log` records a
negative test expecting a producer-set error after deleting its only record; the
empty-proof check correctly fired first. The revised test retains an unmatched
record and exercises the intended check. Package preparation then correctly
rejected a binary/acceptance digest mismatch (`package-07.log`); the final
preparation artifact was revalidated in `acceptance-24`, all five Core cases,
historical comparison and lifecycle checks. No acceptance anchor was bypassed.

Rust unit tests separately exercise nonzero exit, timeout/process-group cleanup,
missing executable, source decisions/unsupported owners, symlinks, tampering,
malformed LLVM shape/version and strict JSON duplicate-key rejection.
The execution-audit scanner has synthetic regression tests; those are logic tests,
not cross-host acceptance.

## Actual Core and migration checks

The same collector binary was used by all five real Core acceptance cases. The
plain/boundary cases are at
`target/gh-259/core-acceptance-{plain,partial,boundaries,features}-17/`. Each invokes the
actual Core CLI with a signed v2 request, then runs Core's generic evidence
validation and requiredness evaluator. Plain produces two verified complexity
counts, 3 and 1, plus verified per-function execution ratios 1/1 and 0/1
and code-region ratios 6/6 and 0/3. Partial executes only one side of the same
production function: its function ratio is still 1/1, while its region ratio is 5/6. Boundary fixtures produce unavailable evidence with unsupported
capabilities and no metrics. Required CRAP blocks in every case. The registry fixture at
`target/gh-259/core-acceptance-registry-17/` adds CC 1 and execution coverage 1/1.
It runs with the same isolated `CARGO_HOME` used for capture. The first attempt
with the host Cargo home correctly failed configuration identity verification
(`core-acceptance-registry-10.log`); no identity check was bypassed.

All five cases reject invalid signatures, replay, expiry, altered project/series,
unknown owners, changed source identity, wrong context target, missing claims,
wrong capture anchor and a stale configuration binding. Core independently rejects
stale context and corrupted normalized artifacts. Test keys and synthetic fixture
context commits establish protocol behavior, not protected production signing or
user-project Git provenance. No Core production implementation was changed.

The [same-source historical comparison](stable-rust-collector-migration.md) also
ran with that binary. Original archive/source/report anchors remain intact; new
raw line/region/function totals differ. Its summary is retained separately from
Core acceptance and does not recertify the archived historical report.

## Actual offline lifecycle checks

The [lifecycle summary](stable-rust-candidate-evidence/lifecycle.json) records 38
executed checks. RSA-2048/SHA-256 signatures are real, generated by repository-only
OpenSSL automation using a test-only key. A retained [reproduction](stable-rust-candidate-evidence/lifecycle-reproduction.json)
shows the preceding binary accepting signed version `.2` while the program reports
`.1`. Authentication now precedes a bounded staged-program `--version` launch;
exact version matching and a repeated payload/trust check precede activation.

The [upgrade build](stable-rust-candidate-evidence/upgrade-build.json) independently
compiles the same implementation with test version `0.1.0-candidate.1.upgrade-test`
using stable Rust 1.97.1. Only the package and lockfile versions change. The two
program hashes differ: the installed first program performs the upgrade, and the
installed second program performs rollback. This validates executable switching,
not historical or production release compatibility. Its signed package occupies
3,133,145 bytes; the repository-only second build target occupies 178,718,592 bytes
and is excluded from runtime packages and downloads.

Checks cover unsigned-package rejection, ten re-signed invalid support documents,
both-signature requirements, malformed RSA, payload/inventory mixing,
extra assets, symlinks, duplicate JSON, missing/changed verifiers, nonzero verifier
exit, a real 60-second verifier timeout, wrong program versions, failed program
launches, program exit 23, a real 60-second program timeout, staged support-file
mutation, corrupt/nonexecutable rollback targets, concurrent
lock rejection and SIGKILL during verification followed by retry and rollback.
Each rejected upgrade/rollback checks that the prior selection and binary remain
unchanged. Sigstore is an explicitly mocked external command checking fixed
arguments and exit behavior; neither Sigstore cryptography nor production signing
is established. No mock flag exists in the candidate executable.

The [lifecycle contract](stable-rust-collector-lifecycle.md) documents the exact
flat package, externally pinned trust and atomic selection. Local test packages
contain the prepared program, real collected licenses and typed support metadata.
Generated private keys, mock verifier, logs and acceptance artifacts stay outside
those packages. Test versions and signature envelopes remain development inputs.
Unsigned preparation fails if an acceptance pin, binary identity, archived license
or unpacked build source does not match; six automated preparation tests cover
these boundaries. The first actual preparation rejected acceptance for an earlier
binary (`package-01.log`); the final binary was recaptured before successful
preparation (`package-12.log`).

## Commands and results

Commands ran from the assigned workspace. Existing Core code/check results remain
unchanged; candidate execution, packaging, Python suites and documentation were
validated again for this checkpoint. `CARGO_TARGET_DIR` was redirected into
this workspace because the default `/home/gem/cargo-target` is read-only.

| Command | Actual result and log under `target/gh-259/` |
| --- | --- |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/stable-target" RUSTFLAGS= RUSTDOCFLAGS= CARGO_ENCODED_RUSTFLAGS= RUSTC_WRAPPER= RUSTC_WORKSPACE_WRAPPER= cargo +1.97.1 build --manifest-path tools/quality/rust-stable-collector/Cargo.toml --release --locked --offline` | Passed final build with explicit empty compiler flags/wrappers; exact artifact revalidated below; `region-build-02.log` |
| Same target, `cargo +1.97.1 test --manifest-path tools/quality/rust-stable-collector/Cargo.toml --locked --offline` | 17 passed; `region-tests-01.log` |
| `cargo +1.97.1 fmt --manifest-path tools/quality/rust-stable-collector/Cargo.toml -- --check` | Passed; `region-fmt-01.log` |
| Same target, `cargo +1.97.1 clippy --manifest-path tools/quality/rust-stable-collector/Cargo.toml --all-targets --locked --offline -- -D warnings` | Passed; `region-clippy-01.log` |
| `python3 tools/quality/rust-stable-collector/validate_stable_candidate.py --binary target/gh-259/stable-target/release/harness-gate-rust-stable-collector --output target/gh-259/acceptance-24` | 92 passed; `acceptance-24.log` |
| `env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u NO_PROXY -u http_proxy -u https_proxy -u all_proxy -u no_proxy CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 397 passed; `region-core-nextest-03.log` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed; `region-core-fmt-01.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed; `region-core-clippy-01.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/quality-target" python3 -m unittest discover -s tools/quality/tests -v` | 444 tests OK, 36 skipped; `quality-tests-12.log` |
| `python3 -m unittest discover -s tools/release/tests -v` | 91 passed; `release-tests-13.log` |
| `python3 -m unittest discover -s tools/release/tests -p test_stable_collector_policy.py -v` | 2 passed after CI build environment alignment; `package-ci-policy-01.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/quality-target" python3 -m unittest discover -s tools/quality/tests -p test_ci_topology.py -v` | 7 passed after CI build environment alignment; `package-ci-topology-01.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-259/core-target" python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed; `docs-consistency-13.log` |
| Same Core target, `cargo build --manifest-path tools/harness-gate/Cargo.toml --example stable_collector_acceptance --locked --offline` | Passed; `region-core-build-01.log` |
| `target/gh-259/core-target/debug/examples/stable_collector_acceptance target/gh-259/stable-target/release/harness-gate-rust-stable-collector target/gh-259/core-target/debug/harness-gate target/gh-259/acceptance-24 target/gh-259/core-acceptance-${fixture}-17 $fixture` for `plain partial boundaries features` | All four passed; matching `core-acceptance-${fixture}-17.log` |
| `CARGO_HOME="$PWD/target/gh-259/acceptance-24/registry-cargo-home" target/gh-259/core-target/debug/examples/stable_collector_acceptance target/gh-259/stable-target/release/harness-gate-rust-stable-collector target/gh-259/core-target/debug/harness-gate target/gh-259/acceptance-24 target/gh-259/core-acceptance-registry-17 registry` | Passed; `core-acceptance-registry-17.log` |
| `python3 tools/quality/rust-stable-collector/compare_historical_fixture.py --binary target/gh-259/stable-target/release/harness-gate-rust-stable-collector --output target/gh-259/historical-12` | Passed; `historical-12.log` |
| `python3 tools/quality/rust-stable-collector/validate_lifecycle.py --binary target/gh-259/stable-target/release/harness-gate-rust-stable-collector --package target/gh-259/package-12/unsigned-package --output target/gh-259/lifecycle-12` | 38 passed; real RSA, mocked Sigstore; `lifecycle-12.log` |
| `python3 tools/quality/rust-stable-collector/prepare_release.py --output target/gh-259/package-12 --target-dir "$PWD/target/gh-259/stable-target" --toolchain 1.97.1 --acceptance target/gh-259/acceptance-24/summary.json 60848db3d1742b91dacdccfb18486318700967f42baa2bdba52829a71fb95d73` | Passed locked offline build and package preparation; `package-12.log` and `package-12/preparation.json` |
| `harness-gate config check`; `harness-gate verify --profile ci --all` | Not applicable: no `.harness-gate/flow.toml` declaring `ci`; neither was run |

The registry suite first hit an offline test-setup failure while resolving the
collector's full graph (`acceptance-15.log`); setup now resolves only the fixture
graph and retains its command output. The complete 59-check rerun passed.

The owner-checkpoint release suite initially rejected the preceding checked-in
acceptance, which lacked the newly required owner checks (`release-tests-06.log`).
After refreshing the real 46-check evidence, the complete release suite passed.

The initial Core nextest run hit a local webhook failure with inherited proxies;
removing proxy variables resolved it. Initial candidate probes exposed LLVM's
Rust-specific version suffix, cargo subcommand argument handling and export
schema 3.1.0; those were fixed before the recorded successful release runs.
The first checkpoint quality run found a missing draft documentation link and a
new development script in the production-module inventory directory. The link
was completed and the development driver moved into its crate directory.
The continuation quality run started before the new migration document existed
and reported a documentation link failure (`quality-tests-02.log`); the final
full rerun uses the completed documentation. Compiler-private test classes now
require an explicit manual-experiment opt-in;
required CI retains all other existing checks and does not bootstrap that backend.

## Acceptance still blocked or unimplemented

- Rust 1.98.1 is not installed. The [official release index](https://blog.rust-lang.org/releases/)
  was checked on 2026-09-13: 1.97.1 was released on July 16 and 1.98.1 on
  September 3. Fetching the installable stable manifest from this workspace failed
  `curl: (56) Proxy CONNECT aborted`. Only installed 1.97.1 was executed;
  the second candidate's interfaces and runtime remain unverified.
  Doctor's candidate allowlist is not evidence of support for both versions.
- A second runnable system was unavailable: Docker socket access was denied.
  The available unprivileged `bwrap` route also failed with exit 1 because the
  sandbox denied creating a user namespace. The [namespace probe record](stable-rust-candidate-evidence/namespace-environment.json)
  preserves its exact command and error; it is an availability check, not a run
  on another system. Only the environment above has been exercised; there is no
  released support platform yet and no claim covering all Linux systems.
- `strace` is installed but the sandbox rejects `PTRACE_TRACEME` and
  `PTRACE_SEIZE`. Local command records, build logs and ELF inspection passed;
  a complete transitive `execve` audit did not run. Required CI now requests real
  release-build and runtime traces and fails if tracing fails; CI is pending.
- The Core candidate certifies lexical complexity and bounded function execution
  and code-region coverage for the declared limited owners. It does not certify line
  coverage or CRAP and does not replace existing required measurements. Production request signing and project integration remain open.
- Complete compiler input provenance, broader certified source/coverage owners, comprehensive
  adversarial coverage formats, complete negative matrices and reviewed migration
  still need implementation/acceptance. No MIR backend was rerun and
  original historical anchors remain untouched.
- Rust offline verification/install/upgrade/rollback now pass real RSA and
  transaction tests. Unsigned package preparation and an authenticated notice inventory
  are implemented. Protected signing/publication, production license review, trusted
  bootstrap and full release acceptance remain open. The HTTPS checkpoint above
  adds explicit pinned transport and measured body/cache bytes; public distribution
  and production signature acceptance remain pending.
  The Sigstore verifier is unavailable locally (`cosign version`: no such file
  or directory). The [continuation probes](stable-rust-candidate-evidence/continuation-environment.json)
  retain this error and the installed toolchain list.
  No production signing was simulated
  or claimed; no user installation was changed. The release hold remains.

GH-259 must remain open after this checkpoint. T8 and production publication
cannot proceed on this evidence.

The aggregate-checkpoint lifecycle invocation first named a nonexistent package
subdirectory (`lifecycle-11-setup-failed.log`, FileNotFoundError). No lifecycle
operation ran. The corrected `unsigned-package` invocation passed all 38 checks.

The first docs-consistency invocation used the default Cargo target and failed.
A direct `cargo run --quiet --locked --manifest-path tools/harness-gate/Cargo.toml
-- --help` probe recorded `failed to open: /home/gem/cargo-target/debug/.cargo-build-lock`,
`Read-only file system (os error 30)` in `docs-default-target-probe.log`.
The documented workspace-local `CARGO_TARGET_DIR` invocation passed; the failed
report and log remain in `docs-consistency-12-default-target-failed.{json,log}`.


## Exact-owner code-region checkpoint

The [retained reproduction and Core results](stable-rust-candidate-evidence/region-coverage.json)
show the preceding `e020ad59…` binary returning function execution 1/1 for the
partially exercised `classify` function, while its actual LLVM record contains five
covered code regions out of six. That binary exposed no normalized region metric.
Original reproduction files remain at `target/gh-259/region-repro-01/`.

The new `rust-llvm-exact-root-owner/v2-candidate` rule counts unique CodeRegion
entries belonging to the existing exact source owner and counts nonzero execution
counts as covered. This follows LLVM 22.1.6's
[`sumRegions` and function summary implementation](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.6/llvm/tools/llvm-cov/CoverageSummaryInfo.cpp).
It rejects duplicate spans and requires the sum across all mapped owners,
including explicitly excluded test owners, to match that file's region summary.
Test regions participate in this reconciliation but never enter production ratios.
This does not prove arbitrary counters are truthful after every possible coordinated
rewrite; capture integrity, identities and the remaining LLVM checks still apply.

Real plain and partial fixtures produce region ratios 6/6 and 5/6 respectively;
the unexecuted function has its own actual 0/3 record in both. Their complexity
counts remain 3 and 1. The partial raw export has 13 regions including tests, while
its two production owners have only 9. Five authenticated Core cases now pass,
including direct assertions on these normalized ratios and on required CRAP
remaining blocked. No CRAP model or measurement migration has been accepted.
The support metadata and normalization binary identity identify this candidate;
no accepted historical series or required binding is upgraded automatically.

Seven additional real checks bring the capture suite from 85 to 92: partial
fixture preparation, capture, certified partial ratios, duplicate owner region,
owner counter disagreement, and two internally consistent file/export-summary
mutations (count and covered). The latter mutations pass local summary arithmetic
but fail reconciliation with actual owner records. Release preparation requires
the named checks. Rust tests, stable-only CI and the Core example retain the
existing unsupported owner boundaries.

The first region release build passed and `acceptance-23` recorded 92 successful
checks, but package preparation rebuilt with explicit empty compiler environment
variables and correctly rejected the different executable digest
(`package-11.log`: `acceptance does not identify this candidate binary`). Those
records are not final package acceptance. `region-build-02.log` uses the same
explicit environment as preparation; `acceptance-24`, all five Core cases at
`core-acceptance-*-17`, `historical-12`, `package-12` and `lifecycle-12` use the final
`e2e3296e…` executable. The unsigned payload is four files totaling 3,132,634 bytes.
Lifecycle checks use actual test RSA and mocked Sigstore, with 38 passed; the
separate upgrade executable is `ed82b2bc…`. No production artifact was signed.

The unmodified Core nextest command initially failed the existing
`webhook_accepts_success_response` test with `io: Connection refused` under host
proxy variables. The focused retry reproduced it, while a direct loopback socket
probe succeeded. Removing proxy variables only from the test process made the
focused test pass (`region-core-webhook-no-proxy-01.log`). The full no-proxy command
passed all 397 tests. The inherited-proxy full run had four webhook failures;
the initial fail-fast run stopped at the first. Results are recorded in the table
above; failures remain in
`region-core-nextest-01.log` and `region-core-nextest-02.log`, with the exact final
environment adjustment in `region-core-nextest-03.json`. No Core implementation
or network policy was changed.
