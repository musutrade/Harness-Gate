# Design and migration requirements

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
