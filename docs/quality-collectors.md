# Trusted project collectors

GH-181 implements OpenSpec tasks 3.1–3.4 over the existing signed, out-of-process
adapter v2 host. `quality collect` is an advanced project orchestration interface:

```sh
harness-gate quality collect --repository-root . --state trusted-state.json --trusted-keys trusted-keys.json --output collection.json
```

The state has the [trusted compilation contract](quality-compilation.md).
Its artifact inventory and existing artifact root must be empty. The output
belongs outside that root. Trusted keys are a host-owned
JSON array of `{ "key_id": "…", "public_key": "base64 Ed25519 key" }` objects.
The CLI uses the adapter host's default capability, timeout, request age, output
and artifact budgets and durable nonce storage in `.harness-gate/collector-nonces`.
The internal orchestration entry point accepts a host-owned `HostPolicy` for
explicit limits. Collectors cannot enlarge those limits or grant themselves trust.

## Binding and execution

The validated quality profile selects collector bindings. Each binding's request
path points to a complete signed adapter request. Resolved pack state supplies
collector/package identity and canonical series metadata; configuration supplies
subject/component/relationship targets and expected capabilities. The generic
orchestrator iterates those bindings without inspecting ecosystem metadata.
All selected producers fail closed on execution or validation errors.

The quality configuration's `harness-collector-request/v1` binding selects the
project collector contract below. The signed adapter request retains protocol
version `2` and result schema `1`.
Its `step_id` is the configured binding ID, `invocation_id` is the trusted run,
package name/version match the trusted series collector, and `artifact_root`
matches the compiled canonical root. The host verifies the signature, executable
digest, freshness, nonce, requested capabilities and enforceable resource limits
before launch. Request and response JSON reject duplicate keys at every depth.

The signed `input` is exactly:

```json
{
  "schema": "harness-project-collector-request/v1",
  "project": "trusted project ID",
  "collector": {"name": "configured package", "version": "pinned version"},
  "context": {"commit": "…", "base_commit": null, "target": "…", "run": "…"},
  "workspace_root": "/canonical/source/root",
  "output_root": "/canonical/artifact/root",
  "selection": null,
  "bindings": [{"subject": "canonical subject ID", "capability": "bundle.size", "series": "canonical series ID"}]
}
```

`selection` is the compiled selection contract or null. Bindings are sorted by
subject, capability and series; they exactly resolve the configured targets.
Overlapping authoritative producer claims are rejected before launch.
The outer adapter `artifact_root` must equal the compiled canonical artifact
root as well; sign this resolved path when the workspace uses a symlink alias
(including macOS temporary directories).

`config_digest` is SHA-256 of canonical generic JSON containing `schema` equal to
`quality-collector-binding/v1`, `config_files`, compiled `project`, `policy`,
`expected`, `selection`, `mappings`, `exceptions`, and trusted `profile`, `series`.
Here `config_files` excludes configured request paths to avoid a self-referential
signature. The trusted state separately pins the complete signed request bytes.
The existing adapter signing domain covers the complete input, arguments, roots,
limits and executable identity. A trusted caller prepares and signs these
requests; collectors never generate their own trusted state.

## Measurement response and output

The adapter response contains exactly `schema_version: "1"`, `status: "PASS"`,
the matching `invocation_id`, `artifacts`, and `collection`. Here PASS means the
measurement transport completed. `collection` contains `schema` equal to
`harness-project-collector-response/v1`, an `evidence` array of existing
`harness-evidence/v1` records, and optional `error` (absent or null on success).
This project envelope is distinct from the frozen Python collector v1 protocol.

Each configured subject/capability/series must be present exactly once. The
Rust generic core validates normalized values, capability honesty, canonical
subject/series identity, collector/tool/runtime metadata, source and context,
artifact hashes and links. The outer artifact inventory must exactly match the
records' artifact descriptors and every file on disk must be declared. Symlinks,
missing files, extra files and mixed provenance fail closed. Source/config pins
are rechecked after execution. No partial collection output is published on
failure; an earlier output is removed after protecting input aliases.

Successful output has schema `quality-collection/v1`, compiled `inputs` bound to
the collected artifact inventory, and `evidence`. It is an inspectable measurement
bundle, not an authenticated cache or delivery decision. The existing Rust policy
evaluator owns requiredness, thresholds and approval. Unavailable capability
states remain distinct; no numeric values or CRAP series are synthesized.

The signed executable fixture in `tools/quality/fixtures/workflow/collectors/`
launches an arbitrary ecosystem with a custom binding, package, tool and series
through the same code as the reference fixture. The configured `bundle.size`
contract succeeds; an arbitrary `quasar.payload_bytes` capability is resolved and
launched, then fails closed at the released core's unsupported-metric validation.
This preserves the GH-180 boundary without certifying a new metric or ecosystem. Runtime tests
and a source guard reject introducing closed language dispatch into this path.

`adapter run` remains available for advanced/debug use.
[Verification composition](quality-verification.md) consumes host-prepared signed
requests and combines the resulting quality decision with execution gates.
Automatic request construction remains a host/pack responsibility.
