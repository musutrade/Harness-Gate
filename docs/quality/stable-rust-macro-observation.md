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

The regression driver runs both real test/coverage configurations and the Rust
observer. It rejects altered source/model/version identities, wrong anchors, mixed
configurations, forged coverage zero, altered targets and duplicate owner claims.
Duplicate active invocation owners are `measurement_error`. Nested calls, derive
items and unreviewed cfg return `unsupported` with an explicit boundary and no
partial successful function inventory. Known templates are `supported` only for
their stated source complexity. All coverage/CRAP fields contain no numeric fallback.

## Remaining source-level work

This first-party integration is in draft PR 261; it is not an upstream fix.

| Work | Submitted | Merged | Adopted fixed version |
| --- | --- | --- | --- |
| Third-party parser/macro defect | No concrete defect established; no speculative PR | No | No |
| Missing generated-function coverage capability | Reproducer/exports recorded here; capability request not yet submitted | No | No |
| First-party dependency build-input certification | Refusal reproduced; implementation pending | No | No |

Before promotion, establish an authenticated stable mapping from generated owners
to counters, cover nested/repeated expansion and derive cases, and exercise compiler
and configuration drift. An upstream submission alone will not satisfy that work.
No temporary third-party patch is installed. No business behavior, Core thresholds,
requiredness, baseline or historical anchor changes in this checkpoint.
