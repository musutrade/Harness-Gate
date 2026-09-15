# HTTP JSON contract collector — 0.1.0-rc.3 candidate

Independent Harness-Gate adapter-v2 plugin. Core owns policy and signature checks.
It measures `contract.breaking_changes`, `contract.client_drift`, and
`contract.compatible` from a separately trusted initial baseline, current declared
contract, actual Angular TypeScript AST, and host-recorded live HTTP responses.
All source/capture inputs and baseline bytes are pinned in the signed binding.

Supported scope is deliberately explicit: OpenAPI 3.0.3, parameter-free GET paths,
closed required string-enum JSON object responses and one canonical Angular
`httpResource<ResponseType>(() => '/literal/path')` consumer. Every declared HTTP
status must have a live response observation. Unknown schema/client features,
missing variants or invalid provider responses fail collection, never become zero.
The host must independently inventory approved consumer source files and protect
HTTP capture provenance from repository tests; the receipt alone is not isolation.

Existing operation changes are conservatively counted as incompatible. This is
not a general OpenAPI compatibility engine; a compatible evolution outside this
subset requires a reviewed new series. Additional operations require complete
observations. Initial baseline establishment is distinct from claiming a prior
released API. Baseline updates require host policy approval.

Artifacts use an exclusive component subdirectory under Core's shared root.
No signing keys, policy overrides or Core source imports are included.

The `generate SPEC TYPE_NAME OUTPUT` command emits a response interface stamped
with the exact contract SHA-256. Collection retains both the checked-in generated
type and the current expected generated type. It rejects forged stamps or types
edited without regeneration; a recognized older baseline stamp is reported as
client drift. This satisfies Core's generated-client provenance contract without
changing Core. Formatting differences are normalized through the TypeScript AST.
