# Source coverage omits functions emitted by a function-like proc macro on Rust 1.98.1

Draft for rust-lang/rust; **not submitted**. The macro request received HTTP 403
(`Resource not accessible by personal access token`) on 2026-09-14. The derive
request was not attempted after that repository permission failure. A maintainer
with issue-creation permission can copy the report below. Submission does not
constitute a compiler fix, merge or adoption.

---

Functions emitted by a function-like procedural macro execute successfully on Rust 1.98.1, but have no function entries in the LLVM source-coverage export. I am reporting the fixed reproduction and asking whether a supported mapping/instrumentation path exists. I have not established which internal compiler rule causes this, or that a parser/proc-macro dependency is defective.

### Reproduction

The complete three-crate fixture, including Cargo.lock, is pinned at [this commit](https://github.com/musutrade/Harness-Gate/tree/f553fa412df9e7ac119197af98c13cb169ee070c/tools/quality/fixtures/rust-macro-observation). The ordinary generator library is shared by the proc-macro entry and a source observer; no second expansion implementation is involved. Its two templates are:

```rust
let generated = if branching {
    quote! { pub fn #name(x: i32) -> i32 { if x > 0 { 1 } else { 0 } } }
} else {
    quote! { pub fn #name(x: i32) -> i32 { x } }
};
```

Dependencies are proc-macro2 1.0.106, quote 1.0.45, syn 2.0.119 and unicode-ident 1.0.24, with exact checksums in that lockfile. The macro parses `(Ident, LitBool)` and returns the generated stream. The consumer uses the same macro source for:

```rust
observed_function!(plain, false);
observed_function!(branch, true);
observed_function!(unexecuted, true);
#[cfg(feature = "branching")]
observed_function!(configured, true);
#[cfg(not(feature = "branching"))]
observed_function!(configured, false);
```

The two consumer tests assert `plain(-3) == -3`, `branch(2) == 1`, `branch(-2) == 0`, and `configured(7) == 7` without the feature or `1` with it. `unexecuted` is deliberately never called.

With Rust 1.98.1, its matching LLVM tools and cargo-llvm-cov 0.9.0 already installed, run these in the pinned fixture directory (unset inherited RUSTFLAGS/RUSTDOCFLAGS/CARGO_ENCODED_RUSTFLAGS and compiler wrappers):

```sh
export LLVM_COV="$(rustc +1.98.1 --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-cov"
export LLVM_PROFDATA="$(rustc +1.98.1 --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata"
cargo +1.98.1 llvm-cov --locked --package gate-observation-consumer --json --target x86_64-unknown-linux-gnu --output-path default.json
cargo +1.98.1 llvm-cov --locked --package gate-observation-consumer --json --target x86_64-unknown-linux-gnu --features branching --output-path branching.json
```

Both runs pass their tests. In JSON format 3.1.0, `data[].functions[]` entries referring to `consumer/src/lib.rs` contain only the two test functions, each with count 1. None of `plain`, `branch`, `unexecuted`, or `configured` has an exported function entry. Missing entries are not interpreted as zero executions.

A separate diagnostic repeats both configurations with stable proc_macro span logging around the unchanged returned tokens. Original and diagnostic exports have identical consumer function entries. For each generated body, the body and interior tokens report the same nonempty invocation source range. This does not reveal the compiler's internal expansion context, or prove the span rule responsible. The [diagnostic driver](https://github.com/musutrade/Harness-Gate/blob/f553fa412df9e7ac119197af98c13cb169ee070c/tools/quality/rust-stable-collector/diagnose_macro_spans.py) and [recorded evidence identities](https://github.com/musutrade/Harness-Gate/blob/f553fa412df9e7ac119197af98c13cb169ee070c/docs/quality/stable-rust-candidate-evidence/recovery-198.json) are available; that repository-only Python driver is optional and is not involved in the direct Cargo reproduction above.

### Requested capability / scope

Is there a supported way for this macro to obtain real, distinct generated-function coverage owners/counters using stable instrumentation, without rewriting the consumer behavior or substituting calling-function coverage? If not, please track the instrumentation/mapping capability needed, including whether emitted but unexecuted owners can have explicit zero-count records. An explicit unsupported case is preferable to an invented mapping. This does not request identical coverage numbers across compiler releases.

Related: #131119 and PR #158276 concern an attribute macro around existing bodies. That PR is still unmerged at the time of this report; I have not built it or established applicability to whole functions emitted by this function-like macro. Historical #84561 concerns regions within invoked macros, rather than this missing whole-function record. This report is separate from the intentional built-in derive exclusion.

### Environment and limits

```text
rustc 1.98.1 (48a229cea 2026-09-01)
commit-hash: 48a229ceaefd4985c50990b14116b6d856af0985
host: x86_64-unknown-linux-gnu
LLVM version: 22.1.8
cargo-llvm-cov 0.9.0
```

These are existing actual Linux runs with the pinned sources/exports rechecked by SHA-256 before submission; not new runs performed for this report. Beta/nightly and patched compilers were not tested. No compiler-private API, unstable expansion tool, or unstable instrumentation option was used. General macro coverage support is not claimed.
