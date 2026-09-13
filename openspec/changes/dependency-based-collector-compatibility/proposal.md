# Dependency-based collector compatibility

Replace publisher-host equality checks for newly versioned Rust collector manifests
with explicit signed dependency requirements. Users install missing private components
once and reuse them across upgrades. Preserve signatures, exact compiler/tool identities,
reviewed Core/protocol receipts and existing measurement/policy boundaries.

Legacy signed manifests retain their original contract. New installers validate actual
runtime behavior and retain the previous selection on failure. This change does not
publish a release or modify historical captures and baselines.
