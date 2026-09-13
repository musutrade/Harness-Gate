# Tasks

T4 also includes the PR 260 source-level macro/derive investigation and shared
generation contract: bind source/version, invocation/config/target and ownership;
distinguish generated code from macro implementation; track upstream submission,
merge and adopted version separately. The bounded Rust observation implementation
is recorded in `docs/quality/stable-rust-macro-observation.md`. General authenticated
collection and generated-function coverage remain blocked. T4 stays unchecked.
The follow-up stable span diagnostic has four actual original/diagnostic coverage
runs and pins the equal body/interior source ranges. It narrows the compiler
mapping investigation without claiming internal trace evidence or an upstream fix.
Related upstream issue 131119 / PR 158276 are tracked with their inspected source
and submission/merge/adoption states in `macro-upstream-tracking.json` under the
candidate evidence directory. The proposed attribute-macro fix is not certified
for our function-like fixture; no patched compiler or fixed version was adopted.

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
lexical complexity and bounded function/code-region coverage while required line
coverage and CRAP remain blocked. The same-source
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

The bounded T5 activation checkpoint reproduces acceptance of a signed version
that differs from the executable's actual version, then adds authenticated,
bounded launch/version validation and post-launch payload/trust checks before
selection. The activation checkpoint binary passed 38 lifecycle checks, including an independently
compiled test-version upgrade and rollback executed by the installed programs.
Wrong versions, nonzero exit, timeout, invalid ELF and staged-file mutation preserve
the prior installation. Test RSA is real; Sigstore remains mocked. The second build
uses the same implementation and does not establish historical/production upgrade
compatibility. All 74 collection checks, four Core cases, 16 Rust tests and quality
checks were rerun; exact results are in the validation record. T5 and T8 remain
unchecked pending the full release acceptance and protected publication contract.

The bounded T4 aggregate checkpoint reproduces acceptance of inconsistent but
individually valid LLVM totals, then reconciles total count/covered values with
checked sums of exported file summaries. Eleven real-export negative cases are
required by package preparation. Full segment/region counter semantics remain
incomplete, with no owner/CRAP expansion or migration. Validation details and
remaining environment/release blockers are recorded in the validation document;
T3–T8 remain unchecked.

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

Actual code-region validation is recorded in
`docs/quality/stable-rust-collector-validation.md` and compact
`stable-rust-candidate-evidence/region-coverage.json`: 92 real capture checks,
five signed-request Core fixture cases, 17 candidate Rust tests and 38 lifecycle
checks (test RSA, mocked Sigstore). Final release, capture and unsigned package
share SHA-256 `e2e3296e90b8da095bfe98f504589faca27ade64565262697747567183cd3e6c`.
Only Rust 1.97.1 on the recorded Ubuntu environment ran. T3–T8 remain unchecked;
two-toolchain/system, real signing/process trace, complete owner/input boundaries
and reviewed migration remain required before removing the release hold.

## Bounded HTTPS lifecycle checkpoint (T5 remains incomplete)

The Rust executable now downloads an explicit digest-pinned request's five assets
with verified TLS, bounded redirects/body sizes/deadlines and exact length/hash
checks. Existing host-pinned RSA/cosign, support, executable-version and atomic
activation checks follow transport while retaining the installation lock. Failed
or killed updates preserve the prior version. No trust bootstrap, release discovery,
unsigned fallback or toolchain download is added; the release hold stays active.

The final stable binary passes 29 real loopback HTTPS checks, including distinct
compiled upgrade/rollback and request/TLS-root mutation during transfer. Initial
and upgrade signed-test downloads are 4,790,111 and 4,790,201 body bytes. TLS and RSA
are real test fixtures; Sigstore is mocked. The same binary passes 92 capture,
26 macro, five authenticated Core and 38 local lifecycle checks; Core 397, quality
444 (36 manual legacy skips), release 91, formatting and clippy pass. Exact logs,
prior failed attempts and byte measurements are recorded in
`docs/quality/stable-rust-candidate-evidence/https-download.json` and the validation
document. Second toolchain/system, actual process tracing, public distribution,
real dual-signature, protected trust bootstrap and release review remain pending.
These results do not complete T5 or T8, authorize publication or adopt a baseline.

## Bounded execution audit checkpoint (T7 remains incomplete)

Synthetic regression records confirmed that the previous execve audit skipped
resumed arguments and accepted truncated/unfinished calls. The repository-only
parser now joins calls by PID, requires complete decoded arguments, and rejects
incomplete/ambiguous records. Six focused tests pass; before/after evidence is in
`docs/quality/stable-rust-candidate-evidence/execution-audit.json`. This is parser
validation, not actual tracing evidence. The Rust binary remains unchanged, and
T3–T8 remain unchecked with the same release acceptance blockers.

The T4 inline-module checkpoint implements `rust-llvm-exact-free-owner/v3-candidate`.
An identical-source before/after fixture changes three ordinary module owners
from unsupported to independently verified results: lexical complexities 2/1/2,
execution counts 1/0/2 and code-region ratios 4/5, 0/3, 5/5. Missing, duplicate
and inherited-count mutations fail; real annotated/cfg variants remain unsupported.
Candidate Rust tests and real generic capture checks cover the new scope; Core
acceptance and package preparation require the module cases. Exact commands and
final results are in the candidate validation record. This does not certify
generated owners or complete T3–T8. Metric definitions are unchanged; the new
binary-bound Core series identity does not authorize baseline compatibility.
