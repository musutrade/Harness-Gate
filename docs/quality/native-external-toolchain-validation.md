# Native standalone local acceptance — 2026-09-14

The native delivery branch is based directly on main `04344d6`, independently of
the stable candidate in PR #261. The old driver, normalizer, classifier, policy
bridge and projection sources are byte-identical to that main revision.

The driver was rebuilt from source with Rust 1.97.1 and then packaged with a
std-only launcher, existing Python modules/schemas and authenticated license
notices. Local artifact: **1,338,392 bytes**, SHA-256
`56d80adf5fd326d54504f9dc8346b4766f89bf8c190a2bfd1a6673b0ca6dc856`.
This is an unsigned development binary, not a signed release.

- **16 real native tests passed, zero skipped**, including independent macro,
  derive, constructor, closure and async counts; two generic instances; selected
  Cargo features and test exclusion; exact old-engine report equivalence;
  independent LLVM re-export; damaged, missing and ambiguous evidence rejection.
- The copied standalone executable ran outside the repository, using external
  Rust and isolated Python. Missing dependencies, compiler overrides and changed,
  extra or symlinked cache payloads failed. A separate real Rust 1.98.1 probe was
  rejected as an incompatible toolchain. No stable fallback was attempted.
- Actual released Core **0.4.2** authenticated the executable and full signed
  protocol-v2 request, accepted native collection, and blocked tampered arguments
  and replay. The existing policy fixture retained unchanged debt at CRAP 56,
  blocked changed debt, and blocked regression from CRAP 7 back to 56. The packaged
  `evaluate` command also returned Core's expected failing aggregate.
- **88 release tests passed**, including the shared inventory/signature-set
  invariants, exact version/tag/main-CI eligibility, and native publication job
  dependencies. YAML parsing and syntax checking of all 16 shell steps passed.
- Documentation/schema consistency, Rust formatting, strict OpenSpec validation
  and `git diff --check` passed. The actual packaged binary/SBOM/inventory checksums
  passed; the unsigned package was rejected for missing Sigstore products.

Machine results and retained logs are in
[validation.json](native-external-evidence/validation.json),
[native acceptance](native-external-evidence/native-acceptance.log),
[release tests](native-external-evidence/release-tests.log) and
[build inventory](native-external-evidence/build.json). The machine record names
the complete local evidence directory and its independently retained fixture
anchor. Raw local evidence remains available there; it is not a production trust
receipt or a newly accepted baseline.

Local host: Ubuntu 26.04, Python 3.14.4, matching external Rust/LLVM 1.97.1/22.1.6.
The initial precompiled-driver smoke test ran before adding `rustc-dev`; the
subsequent source build used that build-only component. Default Rust remained
`stable-x86_64-unknown-linux-gnu`. The executable embeds no toolchain/interpreter.

Hosted Ubuntu 24.04/Python 3.12 acceptance, protected-main CI, Sigstore signing,
provenance and immutable tag publication are separate workflow results and are
not claimed by these local tests. Existing signed RC3 assets are unchanged.
