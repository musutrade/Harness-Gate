# Shared macro templates: source observations and execution gap

This T4 checkpoint implements source observations in the candidate Rust executable.
It does not complete T4, enable macro metrics in Core, certify generated-function
coverage/CRAP, or remove the release hold. It adopts the shared-generator prototype
from [PR 260](https://github.com/musutrade/Harness-Gate/pull/260), reviewed at commit
`c2f7c14fbee0245e3a48176f7a768f6d3fd1a041`, as an actual collector dependency.
The process macro and collector call the same `expand` parser/generator. There is
no separately maintained expansion algorithm.

## Contract

```sh
harness-gate-rust-stable-collector observe-macro-template PROJECT default NEW_OUTPUT.json
harness-gate-rust-stable-collector observe-macro-template PROJECT branching NEW_OUTPUT.json
harness-gate-rust-stable-collector verify-macro-template PROJECT OBSERVATION.json EXPECTED_SHA256
```

`PROJECT` is a copy of `tools/quality/fixtures/rust-macro-observation`. Write output
outside the project. This bounded model recognizes our version 0.1.0 identity and
single-if templates. It checks compiled-in identities for the generator and macro
source, workspace/package manifests, consumer manifest and Cargo.lock. It binds
every project file and external Cargo configuration, each invocation's source span
and input, selected feature variant, requested Linux x86_64 GNU target, generated
token digest and owner. Unknown additional inputs or changed model/dependency
identities fail. Reverification requires an external digest and recomputes the
complete result from current sources.

`rust-shared-template-cyclomatic/v1-candidate` counts 1 plus `if` decisions in one
generated function of these two known templates: values 1 and 2. It does not measure
cognitive complexity, expansion size, arbitrary macros, final compiler expansion
or execution. The JSON calls the configuration *requested*: this command does not
invoke a compiler or attest that a build used it. It cannot substitute for collection
or authenticated Core evidence. No new installed executable or tracing tool is needed.

## Reproduction and diagnosis

The fixture pins syn 2.0.119, quote 1.0.45 and proc-macro2 1.0.106. It generates
`plain`, `branch`, `unexecuted` and `configured`. The same source/dependency identity
is tested with default and `branching` features on Rust 1.97.1 / LLVM 22.1.6 Linux.
Tests call the actual generated functions: `plain(-3) == -3`, `branch(2) == 1`,
`branch(-2) == 0`, and `configured(7)` changes from 7 to 1 with the feature.
`unexecuted` is never called.

| Layer | Reproduced fact | Disposition |
| --- | --- | --- |
| Collector model | The earlier collector did not consume the shared generator and gave macro invocations a generic unsupported reason. | The bounded source observer now uses that exact library and checks identities/owners. General source normalization remains unsupported. |
| Collector input verification | Authenticated collection rejects registry dependencies with build scripts/proc macros before collecting this fixture. | Still blocked. Authenticate actual input/configuration behavior before expanding the contract. No rejection rule was removed. |
| Shared parser/generator | Two fixed templates produce different functions that execute correctly for tested inputs/configurations. | Reused directly. No third-party parser or macro-library defect established. |
| Stable compiler/LLVM coverage export | The consumer export contains two executed test functions, but no generated business-function record, including `unexecuted`. | Missing generated owner/counters at this interface. Do not infer zero or borrow macro implementation compilation-time counters. |

Initial diagnostic tests/exports are retained in
`target/gh-259/macro-observation-initial/`. An isolated `quote_spanned!` experiment
using the invocation identifier's span also produced no business-function export.
It is not adopted as a library patch. This narrows the observed interface gap; it
does not identify a particular erroneous rustc branch or justify guessing at a
third-party patch. See the public
[rustc coverage contract](https://doc.rust-lang.org/rustc/instrument-coverage.html).

### Stable span diagnostic

`diagnose_macro_spans.py` now reproduces that gap from a fresh fixture copy. It
runs the original and diagnostic macro under both configurations, for four real
test/coverage executions. The diagnostic wrapper calls the same generator and
returns its original tokens; it only traverses a clone to log stable `proc_macro`
line/column ranges. The generator, consumer and dependency files remain unchanged.
The driver verifies identical consumer coverage exports before/after logging and
different generated `configured` tokens between configurations. It never uses
compiler-private APIs, a compiler build, MIR dumps or a replacement macro expander.

```sh
python3 tools/quality/rust-stable-collector/diagnose_macro_spans.py \
  --output target/macro-span-diagnostic --toolchain 1.97.1 \
  --llvm-cov /absolute/path/to/matching/llvm-cov \
  --llvm-profdata /absolute/path/to/matching/llvm-profdata
```

Both LLVM paths are required, checked for existence and matched to the compiler's
LLVM version before collection. These are preinstalled external dependencies.
The span API used here is stable since Rust 1.88; this diagnostic was actually run
only on 1.97.1. See [the stable Span API](https://doc.rust-lang.org/proc_macro/struct.Span.html).

Observed ranges are 1-based, with an exclusive end column:

| Invocation | Configuration | Body range | Interior token/group count | Interior ranges equal body |
| --- | --- | --- | --- | --- |
| `plain` | Both | 3:1–3:33 | 1 | Yes |
| `branch` | Both | 4:1–4:33 | 9 | Yes |
| `unexecuted` | Both | 5:1–5:37 | 9 | Yes |
| `configured` | Default | 9:1–9:38 | 1 | Yes |
| `configured` | Branching | 7:1–7:37 | 9 | Yes |

Source inspection of the pinned Rust 1.97.1 narrows the suspected failure layer:

- [Coverage span filtering](https://github.com/rust-lang/rust/blob/1.97.1/compiler/rustc_mir_transform/src/coverage/spans.rs#L48-L72)
  removes spans that occupy the entire body and stops if no usable spans remain.
- [Mapping extraction](https://github.com/rust-lang/rust/blob/1.97.1/compiler/rustc_mir_transform/src/coverage/mappings.rs)
  reports an empty mapping set; [instrumentation](https://github.com/rust-lang/rust/blob/1.97.1/compiler/rustc_mir_transform/src/coverage/mod.rs)
  then returns without adding counters.
- A separate [eligibility rule](https://github.com/rust-lang/rust/blob/1.97.1/compiler/rustc_mir_transform/src/coverage/query.rs)
  disables coverage on `automatically_derived` implementations. Our function macro
  emits no such attribute, so that rule does not explain this reproducer.

The observed token ranges are consistent with the body-span filter discarding
generated statements. This is an inference, not a trace of internal compiler
execution: the stable diagnostic does not observe final lowered spans or hygiene
contexts. Therefore it neither proves a compiler defect nor supplies a fix. Assigning
invented distinct spans to make counters appear would not certify their ownership
or coverage. No such workaround is adopted.

The compact record is [macro-span-diagnostic.json](stable-rust-candidate-evidence/macro-span-diagnostic.json);
full exports/logs are anchored under `target/gh-259/macro-span-diagnostic03/`.
The initial run without explicit LLVM paths attempted cargo-llvm-cov's rustup
component setup and failed on the read-only toolchain/download location before
coverage ran. The revised driver requires existing tools and records their hashes.
That failed attempt remains recorded; it is not acceptance evidence.

The regression driver runs both real test/coverage configurations and the Rust
observer. It rejects altered source/model/version identities, wrong anchors, mixed
configurations, forged coverage zero, altered targets and duplicate owner claims.
Duplicate active invocation owners are `measurement_error`. Nested calls, derive
items and unreviewed cfg return `unsupported` with an explicit boundary and no
partial successful function inventory. Known templates are `supported` only for
their stated source complexity. All coverage/CRAP fields contain no numeric fallback.

## Built-in derive: a separate, reproduced eligibility boundary

The [Clone fixture](../../tools/quality/fixtures/rust-derive-coverage/README.md)
pins four derive inputs, the target, lockfile and the `default`/`extra` feature
configurations. Both configurations execute three tests. Different inputs return
3 and 8; the configured input returns 7 or 9. The `Unused` type and its wrapper
remain unexecuted. This uses the same built-in derive implementation in the
recorded compiler, not a second implementation of its generation logic.

On rustc commit `8bab26f4f68e0e26f0bb7960be334d5b520ea452`, matching LLVM 22.1.6
exports eight owners in each configuration: four ordinary wrappers, a manual
`Clone::clone` control and three tests. It exports none of the four derived clone
methods. The unexecuted wrapper has a real zero record; the unexecuted derived
method has no record. These are different evidence states.

The source-level cause is distinct from the function-like macro's span issue:
the pinned [derive generator](https://github.com/rust-lang/rust/blob/8bab26f4f68e0e26f0bb7960be334d5b520ea452/compiler/rustc_builtin_macros/src/deriving/generic/mod.rs#L800)
adds `automatically_derived`, and the pinned
[coverage eligibility rule](https://github.com/rust-lang/rust/blob/8bab26f4f68e0e26f0bb7960be334d5b520ea452/compiler/rustc_mir_transform/src/coverage/query.rs#L59-L66)
excludes those implementations. A hand-written control with that attribute also
has no exported owner; the same method body without the attribute has count 1.
This reproducible difference is consistent with the explicit compiler rule. No
compiler internals were executed or traced by the diagnostic, and no parser or
third-party macro defect is claimed.

The collector does not strip attributes, replace business derives, inherit wrapper
coverage or turn missing methods into zero. Such changes would not authenticate
the original derived method's coverage. `Clone` coverage/CRAP remains unsupported;
this experiment makes that refusal specific, rather than treating all macros as
one unexplained limitation. Source complexity and final generated owner mapping
for arbitrary derives still require separate work.

[derive-coverage-diagnostic.json](stable-rust-candidate-evidence/derive-coverage-diagnostic.json)
records source/tool identities, original export paths/hashes, results and the
initial missing-tool attempt. The repository-only driver runs in stable CI and
requires existing matching LLVM tools. No dependency is installed. This is one
compiler on one Linux environment, not the missing second-toolchain/system
acceptance, process trace or authenticated Core evidence.

The capability request remains pending: provide a supported stable way to obtain
original derived-method coverage with verifiable owner identity. No upstream
request or fix has been submitted, merged or adopted for this fixture. A future
solution must preserve original program behavior and pass different-input,
configuration and unused-owner cases. The related attribute-macro PR below has
not been shown to change this separate eligibility rule.

## Remaining source-level work

This first-party integration is in draft PR 261; it is not an upstream fix.

| Work | Submitted | Merged | Adopted fixed version |
| --- | --- | --- | --- |
| Third-party parser/macro defect | No concrete defect established; no speculative PR | No | No |
| Related upstream proc-macro coverage work | Existing rust-lang/rust issue 131119 and PR 158276; not submitted by this project | PR open at inspected head; not merged | None; applicability to this fixture unverified |
| This fixture's missing generated-function coverage capability | Reproducer/exports recorded here; no exact-match capability request submitted | No | No |
| Built-in Clone derived-method coverage | Pinned eligibility rule and real annotation control reproduced; capability request pending | No | No |
| First-party dependency build-input certification | Refusal reproduced; implementation pending | No | No |

### Related upstream work and applicability

[rust-lang/rust issue 131119](https://github.com/rust-lang/rust/issues/131119)
reports missing body coverage with `tracing::instrument`. The related
[PR 158276](https://github.com/rust-lang/rust/pull/158276) was open and unmerged
when inspected at head `401941acaed24c6477203c87533d08f3c548d953`.
Its proposed change selects the root expansion node for an attribute-macro body
whose selected node has no child contexts and only empty/dummy spans.

This is related work, not an established fix for our reproduction. Our function-like
macro emits complete functions, has equal nonempty body/interior source ranges,
and does not use an attribute-macro closure. No patched compiler was built or run.
The applicability distinction follows from comparing the proposed source condition
with our existing diagnostic; it is not a trace of the compiler's internal state.
The inspected upstream status and source anchors are retained in
[macro-upstream-tracking.json](stable-rust-candidate-evidence/macro-upstream-tracking.json).

The outstanding capability is a stable export that distinguishes generated function
owners, including an unexecuted owner, and binds real counters to each fixed
invocation/configuration. A future upstream merge must still reach a supported
stable release and pass both configurations of this fixture, unused-owner and
ambiguity regressions with matching LLVM tools before adoption. Submission, merge,
and adoption are separate events; no supported metric or toolchain range changes
based on this related PR. The exact function-like reproduction still needs an
upstream applicability determination or a separately tracked capability request.

Before promotion, establish an authenticated stable mapping from generated owners
to counters, cover nested/repeated expansion and derive cases, and exercise compiler
and configuration drift. An upstream submission alone will not satisfy that work.
No temporary third-party patch is installed. No business behavior, Core thresholds,
requiredness, baseline or historical anchor changes in this checkpoint.
