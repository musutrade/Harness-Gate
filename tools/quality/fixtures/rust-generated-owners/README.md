# Generated-owner fixture: shared generator emitting a real source file

This bounded fixture tests whether generated functions can obtain **distinct,
real coverage owners** on stable Rust without any compiler-private interface.
It is the real-file counterpart of `rust-macro-observation`, whose proc-macro
token stream produces no generated owners at the LLVM export.

`generator` is one ordinary Rust library shared by the build-time writer and any
observer. `consumer/build.rs` calls it and writes the generated Rust source to
`OUT_DIR/generated_owners.rs`; `consumer/src/lib.rs` pulls that file in with
`include!`. The generated file is normal crate source, so `-C instrument-coverage`
records each generated function under that file.

Two templates are supported, identical to the shared-generator prototype:
`pub fn NAME(x: i32) -> i32 { x }` and
`pub fn NAME(x: i32) -> i32 { if x > 0 { 1 } else { 0 } }`.

Tests execute `plain`, `branch` and `configured` with real inputs. `unexecuted`
is never called, so its coverage must be a real zero record rather than a
missing entry. This is not general macro support and does not certify the
token-stream path; see `docs/quality/stable-rust-macro-observation.md`.
