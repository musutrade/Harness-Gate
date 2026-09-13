# Dependency compatibility validation, 2026-09-13

The development runtime built on glibc 2.43 / kernel 7.0 ran on a separate Ubuntu
24.04.1 host with glibc 2.39 / kernel 6.8.0-139-generic. Its actual executable
requirements derive a glibc minimum of 2.38. No system toolchain or package was
installed or changed for this validation.

The final runtime passed payload verification and dependency loading, private
C/OpenSSL compilation and execution, and private Rust compilation, execution and
coverage export. The independently pinned bootstrap OpenSSL and private Python
also started on that host. After retaining actual loaded-library fingerprints for
capture identity, the final rebuilt runtime repeated all four native host checks.
Only changed development runtime files were transferred for that repeat; doctor
verified the entire resulting payload inventory.

Local checks: 424 quality-script tests passed (3 optional artifact suites skipped),
87 release-contract tests passed, and all 12 native runtime integration tests passed
with private runtime, offline fixture dependencies and local Core configured.
Documentation/schema consistency and Python compilation passed. The initial runs
identified the missing module inventory entry and missing native test vendor
configuration; both were corrected before these final runs.

[Machine-readable summary](dependency-compatibility-20260913.json) records actual
requirements, host identity, runtime inventory digest and exit codes. Detailed
commands and outputs remain in the worktree's `target/dependency-compatibility/`.
They are developer validation material, not user installation downloads.

This validates development runtime portability and regression behavior. It is not
an online installation of a newly published signed v3 release, a protected release
approval, a reviewed production Core compatibility receipt or a baseline transition.
Existing published installer and collector tags remain unchanged.
