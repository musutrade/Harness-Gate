# Tasks

## Current continuation: line coverage and explicit CRAP review

The v4 candidate adds authenticated same-owner line coverage. Core owns an exact
line-CRAP migration preview, with required CRAP still blocked and no baseline
adoption. PR #262 CI optimizations are synchronized from main. Complete Linux
suite orchestration and current-stable historical comparison retain raw paths and
binary identities. Protected candidate dual signatures and broad generated-owner
capability remain independent acceptance requirements; T3–T8 stay open.
See `docs/quality/stable-rust-line-crap-migration.md`.

## Stable generated-owner alternative (2026-09-14)

The generated-function coverage gap has a stable-interface alternative that does
not wait on the unmerged upstream changes. When the generated source is a real
file pulled in with `include!` (directly, or written by a build script into
`OUT_DIR`), each generated function gets a distinct owner with real counters, and
an unexecuted generated function gets an explicit zero record. Fixture
`tools/quality/fixtures/rust-generated-owners` and regression
`validate_generated_owners.py` (7 real checks, both configurations, plus
doctor/prepare/collect) fix this on Rust 1.98.1; CI wires it into Quality Script
Tests. The proc-macro token-stream path stays `unsupported`, and promoting this
to a supported metric needs Core review plus an accepted measurement migration.
See `docs/quality/stable-rust-generated-owners.md`.

## Generated-owner re-check on 1.98.1 (2026-09-14)

The pinned token-stream fixture reproduces the generated-owner gap on the required
compiler, Rust 1.98.1 / LLVM 22.1.8: both configurations pass and still omit
`plain`, `branch`, `unexecuted` and `configured`. Emitting generated source into a
real file and pulling it in with `include!` (directly and from a build script's
`OUT_DIR`) yields distinct owners with real counters, including an explicit zero
record for the unexecuted function. This is a bounded two-template observation, not
adopted support: proc-macro generated coverage and CRAP stay unsupported, and Core
review plus an accepted metric migration are still required. Evidence is
`generated-source-owner-repro.json` in the candidate evidence directory. Required CI
run 34796325281 at `e68a0da` passes all required jobs. No baseline, threshold or
release hold changes.

## Latest continuation (2026-09-14; T3–T8 remain open)

Operator CI run 34792991105 at `9dec7e7` passes all required jobs, including
10 exec-observer regressions, traced build, 125 capture checks, 26 macro checks,
six Core fixtures, package preparation, 38 lifecycle checks and 29 HTTPS checks.
The repository-only Rust observer supersedes the strace race; incomplete evidence
still fails. Rust 1.98.1 is the sole required compiler, including upgrade builds.
This supersedes older pending trace/rustfmt/dual-version actions in this history.

T5's umask 0002 failure was reproduced with the real installer, then fixed by
setting the creation mode of new versions directories to 0755. Nine actual
install/reinstall/unsafe-directory checks pass across 0002/0022/0077 with the
rebuilt 1.98.1 binary; existing unsafe directory permissions and the runnable
selected program are preserved. RSA is real test signing; Sigstore is mocked.
See `install-permissions.json` in the candidate evidence directory. This does not
transfer earlier runtime acceptance to the new binary or finish T5.

The critical path remains stable generated owner/counter export (including a
supported built-in derive instrumentation path) and reviewed required CRAP/
measurement migration. See the recovery document for exact capability requests
and separate submission/merge/adoption states. Linux and production-signature
acceptance remain independent; no release hold removal or completion is claimed.

The 2026-09-14 upstream submission attempt for the exact function-like fixture
received HTTP 403 (`Resource not accessible by personal access token`); no issue
was created, and the derive submission was not attempted after that failure.
Complete reports are preserved as `upstream-macro-request.md` and
`upstream-derive-request.md` in the candidate evidence directory for an authorized
maintainer to submit. Thirteen public pinned fixture blobs match the existing
inputs. The related derive/default-policy and coverage-attribute discussions do
not provide an adopted stable fix. No runtime suite was repeated for this
documentation step; T4 and T6 remain unchecked.

T4's dep-info follow-up binds complete raw compiler output to the producing
invocation's observed digest and byte length. Added/changed/omitted environment
comments and missing/inconsistent producer identities have real-capture regression
checks. This authenticates observed output bytes only; environment completeness,
build-script isolation and full T3–T8 acceptance remain open. See `dep-info.json`
and `acceptance-dep-info.json` in the candidate evidence directory.

The built-in `Clone` derive follow-up fixes four invocation inputs and two feature
configurations, reproduces absent derived owners with an annotated/manual control,
and pins the compiler's explicit eligibility rule. See `derive-coverage-diagnostic.json`
in the candidate evidence directory. Six real tests pass, but this is a diagnosed
capability gap, not a derive coverage fix or T4 completion; upstream request,
merge and adoption are all pending.

A separate current-stable runtime-only continuation now confirms the built-in
Clone boundary on 1.98.1: two configurations, six passing tests, eight ordinary/
manual owners per export, and absent derived/annotated owners. The unchanged
fixture and tool identities are recorded in `recovery-198.json`. The complete
diagnostic still lacks local rustfmt; no formatting or process-audit pass is
claimed. This confirms the capability blocker, not T4 completion.

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
- [ ] T4: Implement protocol-compatible validation/normalization, preserved Core authority and explicit capability/identity failures; investigate macro/derive gaps with minimal reproductions and focused source-level PRs, validating per-invocation generated code and real coverage before claiming accuracy.
- [ ] T5: Implement lightweight verified publication/installation/upgrade/rollback with user-installed dependencies and measured artifact sizes.
- [ ] T6: Run real current-stable-toolchain, cross-host, negative, source-boundary and old/new measurement comparisons; review the new series transition without baseline reset.
- [ ] T7: Update CI with stable-only acceptance, publish support/limitations documentation and complete all required checks.
- [ ] T8: Remove the legacy publication hold only with reviewed replacement acceptance; production publication remains separately protected.

Documentation or a prototype alone does not complete T2–T8. The policy PR closes
only T1; the implementation issue remains open until the runnable replacement,
release preparation and actual acceptance are complete.

## GH-259 candidate checkpoint

T2 inventory and implementation design: `docs/quality/stable-rust-collector.md`.
Actual candidate execution and remaining acceptance:
`docs/quality/stable-rust-collector-validation.md`. T3 has a runnable Rust capture
and AST implementation. Operator acceptance covers target Rust 1.98.1;
full semantic and cross-system acceptance remains open. T4 has
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
current-stable/full two-system acceptance remain open. T5 and T8 stay unchecked.

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
That historical checkpoint ran Rust 1.97.1 only; the operator supplement below
adds Rust 1.98.1 and bounded Ubuntu 24.04 results. T3–T8 remain unchecked;
complete two-system, real signing, complete owner/input boundaries
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

## Duplicate identity follow-up

T4's strict decoder now also covers typed request/manifest/doctor/archive inputs
and capture-time Cargo metadata. Fresh captured data reproduced last-key-wins
acceptance for conflicting file identities; the same re-anchored input now fails.
Eight CLI checks cover both orders and early collection failure, and package
preparation requires all eight. See `json-identities.json` and `acceptance-json.json`
in the candidate evidence directory for actual commands, identities and scope.
This closes the concrete decoder defect only; T3–T8 and release/migration blockers
remain open. No threshold, baseline, existing required binding or old evidence is
changed.


## Output isolation follow-up

T4's runner now validates the canonical output parent before directory creation.
The previous program rejected aliased/parent-component collection output but
left an empty directory in the project; doctor also reproduced this for direct
paths. Regression acceptance compares directory entries and file hashes for both
entry points and retains a valid relative-output case. Package preparation requires
these seven cases. Validation is recorded in `output-isolation.json` and
`acceptance-output-isolation.json` in the candidate evidence directory. This fixes
the reproduced failure side effect; it does not certify concurrent filesystem
mutation isolation or complete T3–T8 and the retained release/migration blockers.

## Operator recovery and current-stable scope (2026-09-13)

Only Rust 1.98.1 is required for this candidate's build, plugin-specific CI and
runtime acceptance. The operator's exact existing binary passed 125 traced
capture checks and six traced authenticated Core fixtures on 1.98.1. Its Ubuntu
24.04 plain-fixture checks establish a second userspace, not a full second-system
matrix. Historical 1.97.1 records are retained without a dual-version gate.

The inspected operator-patch Quality Script Tests job was cancelled after its six
Core fixture successes. Diagnostic defaults and explicit CI arguments now agree
on 1.98.1. New package preparation requires that compiler and same-binary
current-stable acceptance. No original failing trace is ignored.

See `docs/quality/stable-rust-collector-recovery.md` and `recovery-198.json` in the
candidate evidence directory for the new build, focused stable macro diagnostic,
validation limits and exact next actions. Stable generated-owner/counter evidence
and reviewed required CRAP migration remain missing; T3–T8 stay unchecked, GH-259
stays open, and the legacy release hold is retained.

## Retained T4 acceptance requirements from policy PR #260

Full T4 acceptance still requires source/configuration-bound structured observations,
unambiguous generated owners, actual stable coverage for executed and unexecuted
owners, nested expansions, and a deliberately faulty mapping regression. Focused
upstream patches require a reproduced library defect; submission and deployed fixes
remain separate. The shared-generation prototype alone does not complete T4 or
authorize changing the accepted measurement series.

## Generated source to Core integration (2026-09-14)

An explicit `export-core-source` command creates a verified source workspace outside
the original project and capture. `describe --source-workspace` and authenticated
adapter conversion preserve the original capture binding while presenting actual
relative source paths to unchanged Core validation. The workspace uses a distinct
candidate source identity; it does not adopt a baseline or migrate required metrics.
Generated inputs at distinct paths keep distinct owners even when bytes match, and
all consuming compiler records are retained. Three actual configurations (default,
branching, duplicate source) must reach Core with real complexity/function/region
values; mutated sources and cross-configuration snapshots must fail.

See `docs/quality/stable-rust-generated-owners.md` for the source-view contract and
validation. Policy semantics are unchanged. T4 remains incomplete for broad macro/
derive ownership, line coverage and CRAP; T6 migration and remaining release/system
acceptance are unchanged. This closes the concrete generated-file Core path gap,
not the complete GH-259 acceptance checklist.

## Same-binary Linux acceptance and actual verifier checkpoint (2026-09-14)

Ubuntu 26.04 and 24.04 now run the same Rust 1.98.1 candidate through 135 capture,
64 generated-source, 26 macro, nine authenticated Core, 47 lifecycle and 29 HTTPS
checks per userspace, plus the current-stable historical comparison. Both also
pass five actual cosign cryptographic/integration checks; the positive is the
upstream cosign release, and the collector rejects an unrelated signed payload.
See `docs/quality/stable-rust-candidate-evidence/line-migration-linux.json` for
original paths, hashes, sizes, prior failures and exact limits. The userspaces
share a kernel. Candidate production dual signatures, macro/derive capability,
required-CRAP migration approval and release-hold removal are still incomplete;
T3–T8 are not closed by this bounded matrix.
