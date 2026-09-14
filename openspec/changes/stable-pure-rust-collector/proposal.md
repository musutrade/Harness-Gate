# Stable, pure-Rust collector delivery

User-approved normative policy delta: remove unstable compiler interfaces from
first-party plugin releases and required CI, implement the Rust collector entirely
in Rust, and publish our code rather than a private development environment.
External dependencies are user-installed and checked through documented capabilities.

This supersedes the delivery architecture of the legacy compiler-private/Python
collector. Host compatibility PR #258 alone does not implement this replacement.
Retain history and immutable releases. Do not continue publishing the old architecture
as the answer to this requirement. Repository Core's existing stable CI remains required.

See Engineering Policy section 10 for durable requirements. All Core decision,
threshold, requiredness, trust and baseline rules remain unchanged except for the
explicitly reviewed measurement-series transition described by this change.


The user-approved release delta (2026-09-14) removes the extra RSA signature and
collector-specific duplicate signing/approval rounds. The stable candidate follows
Core with SHA-256, one Sigstore signature bound to its exact immutable version tag,
verified installation and failure rollback. See the lifecycle contract for the
versioned trust/envelope migration. Quality gates, CRAP, real measurement, failure
blocking and Core authority are preserved; PR #261 remains draft. Legacy published
assets and historical evidence are not rewritten or republished.
