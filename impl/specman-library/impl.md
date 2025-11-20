spec: ../../spec/specman-core/spec.md
name: specman-library
version: "0.1.0"
location: ../../src/crates/specman
library:
  name: specman-library@0.1.0
primary_language:
  language: rust@1.91.0
  properties:
    edition: "2024"
  libraries:
    - name: schemars
    - name: serde_json
    - name: markdown
secondary_languages: []
---

# Implementation — SpecMan Library Rust Crate

The `specman-library` implementation delivers the reusable library surface defined in [SpecMan Core](../../spec/specman-core/spec.md) by packaging deterministic helpers inside a dedicated Rust crate rooted at `src/crates/specman`. Each module honors the constraints from the [SpecMan Data Model](../../spec/specman-data-model/spec.md) so downstream tools can consume uniform structures regardless of host environment.

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
- **Primary — `rust@1.91.0` (2024 edition):** The crate leverages the stable 1.91 toolchain to compose performant, strongly typed services. Rust traits organize entry-point catalogs, while enums and structs model deterministic function descriptors, `DependencyTree`, and `TemplateDescriptor`. Edition 2024 features (inline `let`, `if-let` chains, and `impl Trait` in return position) keep the ergonomics modern without compromising MSRV requirements. Supporting crates include:
  - `schemars` for deriving JSON Schema artifacts tied to the SpecMan Data Model entities.
  - `serde_json` for canonical serialization/deserialization of request and response payloads.
  - `markdown` for parsing templates and emitting deterministic Markdown outputs for generated specifications, implementations, and scratch pads.
- **Secondary languages:** None. All orchestration, validation, and I/O run in Rust; shell glue or scripting layers consume the library through binary wrappers built from the same crate.
```

## Implementation Details

### Code Location

Source code lives at `src/crates/specman`, a standalone crate listed inside the repository-level Cargo workspace (`src/Cargo.toml`). The crate exposes a library target (`lib.rs`) plus optional binaries for CLI entry points. Consumers import the crate via the workspace path, and CI builds run `cargo fmt && cargo clippy && cargo test -p specman` from the `src` directory to keep outputs verified against Rust 1.91.

### Libraries

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]

  pub name: String,
  pub template: TemplateDescriptor,
  pub configuration: BTreeMap<String, serde_json::Value>,
}
The implementation adds three non-standard dependencies to satisfy specification requirements:

2. **serde_json** — Performs canonical serialization/deserialization of API payloads, matching the deterministic behavior demanded by the [Data Model Backing Implementation](../../spec/specman-core/spec.md#concept-data-model-backing-implementation).
3. **markdown** — Streams Markdown template parsing and rendering, enabling runtime resolution of template locators per the [Template Orchestration](../../spec/specman-core/spec.md#concept-template-orchestration) concept.
- `ScratchPadProfile` mirrors the structure described under [ScratchPadProfile](../../spec/specman-core/spec.md#entity-scratchpadprofile), pairing template metadata with configurable token payloads.

## Implementation TODOs

- [x] Define `SemVer`, `EntityKind`, and `SchemaRef` to capture reusable logic metadata.
- [x] Model dependency traversal via `ArtifactId`, `ArtifactSummary`, `DependencyEdge`, and `DependencyTree`, alongside the `DependencyMapping` trait.
- [x] Describe templating primitives with `TemplateLocator`, `TemplateScenario`, `TemplateDescriptor`, `RenderedTemplate`, and the `TemplateEngine` trait.
- [x] Capture scratch pad orchestration data with `ScratchPadProfile`.
- [x] Establish lifecycle abstractions through `LifecycleController`, `CreationRequest`, `CreationPlan`, `DeletionPlan`, and `ScratchPadPlan`.
- [x] Provide persistence adapters using the `DataModelAdapter` trait plus an in-memory implementation for testing.
- [x] Implement `SpecmanError` for consistent error reporting across modules.

## API Surface

Primary entry points focus on dependency traversal and template rendering so downstream tooling can plan lifecycle actions deterministically.

```rust
pub trait DependencyMapping {
    fn dependency_tree(&self, root: ArtifactId) -> DependencyTree;
    fn upstream(&self, root: &ArtifactId) -> Vec<DependencyEdge>;
    fn downstream(&self, root: &ArtifactId) -> Vec<DependencyEdge>;
}
```

- `dependency_tree` returns the aggregate `DependencyTree` described in the specification, including transitive nodes for use in lifecycle enforcement.
- `upstream` and `downstream` provide filtered projections needed for targeted impact analysis per [Dependency Mapping Services](../../spec/specman-core/spec.md#concept-dependency-mapping-services).

```rust
pub fn render_template(locator: &TemplateLocator, tokens: &TokenMap) -> Result<RenderedTemplate, SpecmanError> {
    // Resolves file or HTTPS sources, checks that all {{token}} are provided,
    // and returns Markdown plus structured metadata.
}
```

- `render_template` satisfies the [Template Orchestration](../../spec/specman-core/spec.md#concept-template-orchestration) requirements by reading templates lazily, ensuring HTML directives remain intact until satisfied, and injecting substitution tokens supplied by the caller.

Lifecycle automation reuses these primitives by pairing dependency lookups with template rendering prior to creation/deletion commands, guaranteeing the guardrails mandated by [Lifecycle Automation](../../spec/specman-core/spec.md#concept-lifecycle-automation).

## Data Models

The crate mirrors the entities defined in the specification and derives serde/schemars traits for each record. Representative structures include:

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DependencyTree {
  pub root: ArtifactSummary,
  pub upstream: Vec<DependencyEdge>,
  pub downstream: Vec<DependencyEdge>,
  pub aggregate: Vec<DependencyEdge>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TemplateDescriptor {
  pub locator: TemplateLocator,
  pub scenario: TemplateScenario,
  pub required_tokens: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScratchPadProfile {
  pub name: String,
  pub template: TemplateDescriptor,
  pub configuration: BTreeMap<String, serde_json::Value>,
}
```

- `DependencyTree` encodes the traversal helpers outlined in [DependencyTree](../../spec/specman-core/spec.md#entity-dependencytree), maintaining upstream/downstream/aggregate slices for quick querying.
- `TemplateDescriptor` captures metadata from [TemplateDescriptor](../../spec/specman-core/spec.md#entity-templatedescriptor), recording locator data and required tokens so orchestration logic can validate input completeness.
- `ScratchPadProfile` mirrors the structure described under [ScratchPadProfile](../../spec/specman-core/spec.md#entity-scratchpadprofile), pairing template metadata with configurable token payloads.

All records include serde derives for JSON interchange and schemars derives so clients can request the authoritative schema that mirrors the SpecMan Data Model specification.

## Operational Notes

- **Build & Testing:** Run `cargo build -p specman` and `cargo test -p specman` from the `src` directory to exercise the workspace crate with Rust 1.91. Clippy and fmt gates should run in CI to enforce style and catch regressions.
- **Configuration:** Template locators are read from environment variables (`SPECMAN_TEMPLATE_ROOT`) or CLI flags before falling back to repository-relative defaults, satisfying the extensibility guidance from [Template Orchestration](../../spec/specman-core/spec.md#concept-template-orchestration).
- **Lifecycle Automation:** Delete operations must call `dependency_tree` first and abort when downstream edges exist, returning the serialized tree to the caller as mandated by [Lifecycle Automation](../../spec/specman-core/spec.md#concept-lifecycle-automation).
- **Observability:** Each public function logs structured events (entity name, version, dependency counts) so operators can trace execution and audit compliance with the [SpecMan Data Model](../../spec/specman-data-model/spec.md#implementations).

Together, these notes ensure the implementation remains compliant with the SpecMan Data Model sections covering Implementations, Implementing Language, References, APIs, and Metadata while providing actionable guidance for practitioners running the crate.
