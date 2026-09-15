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

## Business JSON contracts (candidate rc.4)

The bounded OpenAPI 3.0.3 adapter supports GET/POST/PUT/PATCH/DELETE, declared
path/query/header parameters, JSON request bodies, closed nested objects, arrays,
string/boolean/number/integer fields, enums, nullable fields and local acyclic
component schema references. Unsupported schema features fail closed. Every
response status for every declared operation requires a real observation.
No-content responses are represented by an absent OpenAPI content member and
an observation with a null body and empty content type.

`generate` now emits all operation response types in one generated type file;
`getHealth` retains `HealthResponse`, other operations use PascalCase operationId
plus `Response` (and `Request` for JSON request bodies). The complete source digest
stamp is retained. The checker rejects edited/widened types, `any`, TypeScript
suppression comments, missing consumers and undeclared operations.

Supported Angular forms are canonical `httpResource<T>(() => '/literal')` and
`HttpClient` held in an explicitly typed/injected variable or class property,
using typed get/post/put/patch/delete calls with literal or template paths.
Dynamic/aliased HTTP APIs require an additional certified adapter; they are not
silently ignored. Parameter templates must resolve to one declared route shape.
Request examples are checked against request schemas for successful responses;
response types are checked structurally against generated operation types.

This is a new measurement series. Installation requires a freshly validated host
approval; it must not replace rc.3 in place or reuse an old series identity.
