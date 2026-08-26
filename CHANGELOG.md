# Changelog

All notable changes to Harness-Gate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
