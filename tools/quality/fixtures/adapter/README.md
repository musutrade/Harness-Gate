# Adapter protocol fixture

`adapter_fixture.py` is a deterministic no-network process used by the Rust
protocol tests. The test signs the declaration with a fixed Ed25519 key and
derives the interpreter digest on the active platform, so the fixture does not
embed host-specific paths or trust material.

Supported `input.mode` values are `pass`, `crash`, `sleep`, `malformed`, and
`escape`. The last four are failure-injection cases and must remain available
for cross-platform contract tests.
