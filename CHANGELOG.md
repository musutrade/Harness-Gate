# Changelog

## Unreleased

### Added

- `config check --format json` now emits versioned, field-path diagnostics for
  configuration parse, interpolation, validation, and resource conflicts.
- Configuration preflight rejects unordered shared services, unordered service
  injection-name collisions, and duplicate step logs before execution.
- Optional `[report_templates]` paths are validated as repository-contained,
  disjoint read-only template input; optional HTML and JUnit report output is
  now available without changing the legacy JSON/Markdown files.
- HTML report templates are rendered with Tera, including repository-contained
  `include` and `extends`/`block` composition.
- Verification plans now use dependency-aware scheduling with serial
  compatibility by default and opt-in bounded parallel execution through
  `[execution]`.
- Docker-backed services can select the Docker-compatible Podman runtime with
  `runtime = "podman"`; Docker remains the default.
- Optional `[[notifications.webhooks]]` entries deliver the serialized report
  over HTTP(S) after report writing.

All notable changes to Harness-Gate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Verification steps may declare `depends_on`; invalid dependency graphs fail during configuration validation and selected steps run in stable topological order.

### Added
- Added `harness-gate config schema` to export `schema/flow.schema.json`.
- Added `${NAME}` and `${NAME:-default}` configuration environment interpolation.

## [0.2.0] - 2026-08-27

### Added
- Terminal-aware colored output and interactive verification progress bars
- Configurable `--color auto|always|never` output mode with `NO_COLOR` support
- ADR-0022 and OpenSpec documentation for terminal feedback

### Added
- Stable CLI error codes for audit, secret scan, scope, and verification failures

### Changed
- Refactored audit, secret scan, scope, and verification boundaries to use typed errors while preserving actionable error context
- Consolidated report output and shared Git snapshot/path handling in internal utilities
- Updated all Rust dependencies to latest semver-compatible versions
  - serde: 1.0 → 1.0.229
  - serde_json: 1.0 → 1.0.151
  - toml: 1.1 → 1.1.4
  - ignore: 0.4 → 0.4.33
  - regex: 1.13 → 1.13.1
  - globset: 0.4 → 0.4.20
  - tempfile: 3.8 → 3.27
- Security patches and performance improvements from dependency updates

## [0.1.0] - 2026-08-26

### Added
- Initial public release of Harness-Gate (extracted from arc-flow)
- Renamed from `arc-flow` to `harness-gate`
- Configuration directory changed from `.arc-flow/` to `.harness-gate/`
- Environment variables prefix changed from `ARC_FLOW_` to `HARNESS_GATE_`
- Standalone project structure with independent development workflow
- Complete documentation for independent usage (English + Chinese)
- MIT License
- CI/CD workflows with GitHub Actions
- Installation script for easy setup
- Badge support for CI, crates.io, and docs

### Changed
- Binary name: `arc-flow` → `harness-gate`
- Project structure: Independent repository instead of subdirectory
- Version set to 0.1.0 for initial public release

### Maintained
- All original functionality from arc-flow
- Schema v2 configuration format
- Secret scanning capabilities
- Architecture audit system
- Multi-component workflow management
- Docker service integration
- Git hook integration
