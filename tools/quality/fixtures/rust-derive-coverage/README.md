# Built-in derive coverage diagnostic

This unpublished stable Rust fixture isolates `Clone` derives with different
inputs (`One`, `Two`, `Unused`) and one feature-dependent input (`Configured`).
The `default` and `extra` configurations return 7 and 9 respectively. `Unused`
and its ordinary wrapper are never executed. Three tests in each configuration
verify the actual returned values, including two hand-written control impls.

`Manual` and `Annotated` use the same clone body. The latter deliberately carries
`#[automatically_derived]` to isolate the compiler's coverage eligibility rule.
This diagnostic does not remove attributes from user code, replace a user's
derive, or introduce an accepted collector mapping rule.

Run the repository diagnostic with existing LLVM tools matching the selected
stable compiler (no installation is performed):

```sh
python3 tools/quality/rust-stable-collector/diagnose_derive_coverage.py \
  --toolchain 1.97.1 --output target/derive-coverage \
  --llvm-cov /absolute/path/to/llvm-cov \
  --llvm-profdata /absolute/path/to/llvm-profdata
```

On the recorded 1.97.1 build, LLVM exports the four ordinary wrappers, the manual
clone and the three tests. It exports neither the four derived clone methods nor
the annotated control. The unexecuted wrapper has an actual zero record; missing
derived records do not have a zero record. Wrapper coverage is not clone coverage.

The driver records source/tool hashes, exact commands, output and raw exports in
a fresh directory. This is repository test automation, not plugin runtime code.
A changed owner inventory fails and requires review rather than silently widening
support. This covers built-in `Clone`, not arbitrary procedural derives, final
expansion provenance, authenticated Core integration, or function CRAP.
