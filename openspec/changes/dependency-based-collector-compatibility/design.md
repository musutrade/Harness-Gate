# Design

See [ADR 0052](../../../docs/adr/0052-dependency-based-collector-compatibility.md)
and the [delivery contract](../../../docs/quality/rust-collector-delivery-contract.md#dependency-based-installation-v3-unreleased).

Version requirements in delivery v3/runtime 2/user installer v3. Inventory required ELF
symbol versions, excluding SDK link inputs. Validate those inputs through C/OpenSSL and
native Rust compile/run/coverage self-tests. Keep the verifier inside the independently
pinned bootstrap and verify its dependencies fit the signed runtime bounds. Resolve
missing compiler/runtime content using the existing component store and atomic activation.
