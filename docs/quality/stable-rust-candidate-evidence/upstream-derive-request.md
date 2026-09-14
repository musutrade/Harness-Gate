# Coverage capability request: opt-in instrumentation and owner records for built-in derived methods

Draft for rust-lang/rust; **not submitted**. The macro request received HTTP 403
(`Resource not accessible by personal access token`) on 2026-09-14. The derive
request was not attempted after that repository permission failure. A maintainer
with issue-creation permission can copy the report below. Submission does not
constitute a compiler fix, merge or adoption.

---

Request: a supported opt-in way to instrument built-in derived methods and export their distinct owners and real execution counters with source-based coverage. This is a capability request, not a claim that excluding derives by default is a regression.

The opt-in need was already discussed in [#147434](https://github.com/rust-lang/rust/issues/147434#issuecomment-3376466001). #134749 tracks the broader coverage attribute and asks for specific questions in dedicated issues. This request adds a fixed built-in Clone reproduction with different inputs/configurations and executed/unexecuted methods. It does not request a change to default coverage policy or guaranteed identical numbers across Rust versions.

### Confirmed boundary on current stable

At rustc commit `48a229ceaefd4985c50990b14116b6d856af0985`, [coverage/query.rs](https://github.com/rust-lang/rust/blob/48a229ceaefd4985c50990b14116b6d856af0985/compiler/rustc_mir_transform/src/coverage/query.rs#L59) explicitly excludes automatically-derived definitions and their nested bodies. The built-in deriving implementation marks the generated impl accordingly. This source observation matches the runtime results below. It does not establish a viable stable opt-in override.

### Reproduction

The dependency-free crate is pinned [here](https://github.com/musutrade/Harness-Gate/tree/f553fa412df9e7ac119197af98c13cb169ee070c/tools/quality/fixtures/rust-derive-coverage), including the complete source, tests, Cargo.toml and lockfile. It derives Clone for:

```rust
#[derive(Clone)]
pub struct One(pub u8);
#[derive(Clone)]
pub struct Two(pub u8, pub u8);
#[derive(Clone)]
pub struct Unused(pub u8);
#[derive(Clone)]
pub struct Configured {
    pub value: u8,
    #[cfg(feature = "extra")]
    pub extra: u8,
}
```

Ordinary wrapper functions call the clone methods; tests check results 3 for One, 8 for Two, and 7/9 for Configured without/with `extra`. A wrapper for Unused is never called. Two additional diagnostic controls implement Clone manually, with and without `#[automatically_derived]`, and both are actually executed. They isolate the annotation effect; replacing business derives with handwritten implementations is not proposed as a workaround.

With Rust 1.98.1, matching LLVM tools and cargo-llvm-cov 0.9.0 already installed, run these in that fixture directory (unset inherited RUSTFLAGS/RUSTDOCFLAGS/CARGO_ENCODED_RUSTFLAGS and compiler wrappers):

```sh
export LLVM_COV="$(rustc +1.98.1 --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-cov"
export LLVM_PROFDATA="$(rustc +1.98.1 --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata"
cargo +1.98.1 llvm-cov --locked --json --target x86_64-unknown-linux-gnu --output-path default.json
cargo +1.98.1 llvm-cov --locked --json --target x86_64-unknown-linux-gnu --features extra --output-path extra.json
```

Each configuration passes three tests. Each LLVM JSON 3.1.0 export contains eight owners: four ordinary wrappers, the unannotated manual Clone method, and three test functions. The unused wrapper has count 0; other entries have count 1. The four derived Clone methods and the annotated manual control have no entries. A missing derived entry cannot tell the observer whether its method was executed, and must not inherit its wrapper's count.

### Requested capability

A supported instrumentation path should distinguish the derived method owners for different type inputs and selected configuration, export actual counts for executed methods, and define the treatment of emitted but unexecuted methods. The source observer can bind macro source, inputs, cfg and target, but that cannot recreate absent runtime counters. Stable structured generated-source/owner information may be useful too; recording a TokenStream alone does not supply execution evidence or final expansion ownership.

If there is an existing specific opt-in capability request, this can be linked to it. I have not implemented a compiler patch or claimed that coverage-attribute stabilization alone solves the missing owner mapping.

### Environment and evidence limits

```text
rustc 1.98.1 (48a229cea 2026-09-01)
commit-hash: 48a229ceaefd4985c50990b14116b6d856af0985
host: x86_64-unknown-linux-gnu
LLVM version: 22.1.8
cargo-llvm-cov 0.9.0
```

Source, tool, command and raw export identities are recorded in [the runtime-only continuation evidence](https://github.com/musutrade/Harness-Gate/blob/f553fa412df9e7ac119197af98c13cb169ee070c/docs/quality/stable-rust-candidate-evidence/recovery-198.json). Those existing real runs and pinned compiler source copies were rechecked by SHA-256 before submission. An earlier full repository diagnostic stopped at a missing rustfmt component; only the separate default/extra runtime runs are counted here. No beta/nightly compiler, compiler-private API, or patched compiler was tested; no repaired version is claimed.
