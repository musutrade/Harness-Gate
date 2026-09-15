# TypeScript risk collector plugin (candidate 0.1.0-rc.1)

An independently installed Node plugin. It speaks `harness-collector-request/v1`
on stdin and returns exactly one `harness-collector-response/v1` on stdout.
The response contains standard `harness-evidence/v1` facts and retained raw
artifacts. Core owns thresholds, ratchets, requiredness and final decisions.
The existing TypeScript reference adapter, preset series and Rust release policy
are unchanged. This candidate does not turn the old unsupported capability into
supported evidence by changing a label.

## Install and test

```sh
npm ci --ignore-scripts
npm test
HARNESS_GATE_BINARY=/absolute/core-0.4.5 npm run test:core
npm pack
npm install --prefix /private/plugin-root ./harness-gate-typescript-collector-0.1.0-rc.1.tgz
/private/plugin-root/node_modules/.bin/harness-gate-typescript-collector --version
```

Node 24 and TypeScript 6.0.2 are explicit runtime dependencies. The npm shrinkwrap
pins the independently installed dependency closure. No parent repository Python
modules, development workspace or Core source checkout is required at runtime.
The plugin and all dependencies must be installed in a trusted, immutable runtime;
host configuration binds that runtime's digest and executable path. npm metadata
is not a release signature or a grant of collector trust. This package is private
until independent publication review and release packaging are complete.

## Original-source measurement contract

The candidate accepts **original TypeScript Istanbul instrumentation before
TypeScript transpilation**, using istanbul-lib-instrument 6.0.3 and TypeScript
6.0.2. Real-tool fixtures retain original coordinates in native counters.
Ordinary Angular CLI mapped coverage, V8, templates, generated code and TSX have
not been certified by these fixtures. They may not be relabelled as this series.
A trustworthy source-map integration and real Angular acceptance are still
required before using this plugin for all of CodexSymphony's frontend.

Every `.ts` file under the host-selected source root is inventoried, including
files whose names end in `.test.ts` or `.spec.ts`. Only ambient `.d.ts` files are
excluded automatically. The trusted host can pass an explicit `exclude` list of
exact file paths; it is part of receipt scope. Names do not authorize production
omissions. Symlinks, path escapes, parse errors, missing/extra native file entries,
missing or duplicated functions, stale source bytes and damaged counters fail.

Per-function identities include complete original path, source digest, AST kind,
UTF-16 start/end span and host project/component/target/boundary. Constructor,
method and accessor declarations use the original node start; named function
expressions/declarations use the name position. Native coordinates must join
exactly one AST function and cover the complete body inventory. Transformed
positions cannot be matched by basename, name similarity or line-only fallback.

Complexity is a versioned **AST decision-count** series: initial 1, plus each
if, conditional expression, loop, catch, non-default switch case, short-circuit
operator, logical assignment, optional-chain operation and parameter/binding
default initializer. Nested callable bodies have their own owner. This is not
claimed to be Rust MIR complexity, a control-flow graph analysis or cognitive
complexity. Source-level semantics can differ from another tool's complexity.

Line coverage uses original Istanbul statement-start lines with maximum hits on
a shared line. Nested function statements belong to their innermost function;
a parent/file percentage never substitutes for a child. This is line coverage,
not branch coverage: an executed condition and unexecuted return on the same
line can share a covered line. Empty denominators are `not_applicable`, without
numeric coverage or CRAP. Runtime code outside callable bodies is outside this
function-risk series and requires separate file/boundary coverage measurement.

CRAP = CC² × (1 − covered/total)³ + CC. The plugin emits an exact reduced rational;
unsafe JSON integers block rather than round. It has no threshold option. Native
fixtures exercise Core 0.4.5 with a caller-owned required `risk.crap <= 10` policy:
10 passes; 11 and partially covered CC=10 fail; retained artifact mutation fails.
These are test fixtures, not production acceptance or a trusted project baseline.

## Host protocol

The normal request supplies project/component/collector/context, capabilities,
canonical workspace and fresh output roots. Collector identity is
`{"name":"typescript-risk","version":"0.1.0-rc.1"}`. Parameters:

- `source_root`: canonical repository-relative production root.
- `boundary`: caller-owned source boundary ID.
- `exclude`: optional exact test/nonproduction path list, approved by host policy.
- `subjects`: full host-owned function inventory for this scope.
- `coverage`: repository-relative original-source coverage JSON file.
- `receipt`: authenticated caller expectations described below.

`inventory` accepts a request with source selection/context and emits discovery
candidates for host review; it does not authorize those subjects. The host must
validate complete ownership against project configuration before signing requests.

The receipt has schema `typescript-original-coverage-receipt/v1`, `request`
(the `binding()` fields including context and scope), exact sorted `sources`
(path/SHA-256), `coverage_sha256`, original absolute `coverage_root`, and
`toolchain` with TypeScript, instrumenter, instrumentation rule and Node version.
See the native test drivers for complete examples. The receipt must be produced
by a trusted capture process and bound by Core's signed collector request. A
receipt constructed by the measured program does not establish provenance.
Current source bytes, counters and receipt are verified before any evidence is
published. Core/host must additionally authenticate the request and verify
returned source, subject, tool, measurement-series and artifact identities.
The plugin does not self-sign, create trusted keys or populate production state.

Use the generic Core collector host for production; invoking the executable
alone does not establish trust. `plugin.json` is package metadata, not a new
Core lifecycle or a replacement for its generic signing/adapter manifest.
