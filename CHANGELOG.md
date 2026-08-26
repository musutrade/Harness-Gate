# Changelog

All notable changes to Harness-Gate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-08-26

### Added
- Initial release of Harness-Gate (forked from arc-flow)
- Renamed from `arc-flow` to `harness-gate`
- Configuration directory changed from `.arc-flow/` to `.harness-gate/`
- Environment variables prefix changed from `ARC_FLOW_` to `HARNESS_GATE_`
- Standalone project structure with independent development workflow
- Complete documentation for independent usage
- MIT License

### Changed
- Binary name: `arc-flow` → `harness-gate`
- Project structure: Independent repository instead of subdirectory
- Version reset to 1.0.0 for new project identity

### Maintained
- All original functionality from arc-flow 3.0.0
- Schema v2 configuration format
- Secret scanning capabilities
- Architecture audit system
- Multi-component workflow management
- Docker service integration
- Git hook integration
