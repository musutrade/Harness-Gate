# Capability: Language-Agnostic Project Model

## ADDED Requirements

### Requirement: Model heterogeneous components without language-specific core branches
Harness-Gate SHALL represent a project as one or more components, each with stable identity, path, ecosystem metadata, source boundaries, and relationships, without requiring language-specific branching in the core domain model.

#### Scenario: Represent a polyglot repository
- **GIVEN** a repository containing Angular/TypeScript, Rust, Python, and Java components
- **WHEN** the project model is loaded
- **THEN** each component is represented by the same generic component model
- **AND** ecosystem-specific details remain metadata or adapter configuration rather than core type branches.

### Requirement: Model analyzable subjects generically
Harness-Gate SHALL represent analyzable subjects such as files, functions, methods, routes, endpoints, contracts, components, and critical paths through a versioned generic subject model.

#### Scenario: Represent function and method subjects
- **GIVEN** a Rust function and a Java method
- **WHEN** both are normalized as subjects
- **THEN** both records use the same subject envelope
- **AND** retain language/ecosystem metadata needed for traceability.

### Requirement: Preserve stable subject identity
Harness-Gate SHALL derive subject identity from component identity, normalized source location, qualified symbol or equivalent discriminator, source span where available, and source integrity metadata; short symbol names alone MUST NOT be authoritative identity.

#### Scenario: Reject ambiguous short-name identity
- **GIVEN** two methods named `run` in different types or components
- **WHEN** subject identities are created
- **THEN** the identities are distinct
- **AND** no policy comparison joins them solely because their short names match.

### Requirement: Model cross-component relationships
Harness-Gate SHALL support explicit relationships between components and subjects so that cross-component gates can bind producers, consumers, and shared contracts.

#### Scenario: Bind frontend to backend contract
- **GIVEN** an Angular frontend consuming an OpenAPI contract produced by a Rust API
- **WHEN** the relationship graph is loaded
- **THEN** the frontend, API, and contract are linked explicitly
- **AND** a contract gate can evaluate that relationship without inferring it from filenames.
