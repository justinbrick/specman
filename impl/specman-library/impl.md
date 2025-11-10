---
spec: ../../spec/specman-data-model/spec.md
name: specman-library
version: 1.0.0
library: specman-library@1.0.0
location: ../../src/crates/specman
primary_language:
  language: rust@1.91.0
  properties:
    edition: "2024"
  libraries:
    - name: markdown@0.3.0
    - name: schemars@0.8.21
    - name: serde_json@1.0.108
references:
  - ref: ../../spec/specman-data-model/spec.md
    type: specification
    optional: false
---

## Implementation Overview
The `specman-library` crate implements the [SpecMan Data Model](../../spec/specman-data-model/spec.md) so client tooling can interact with the core concepts and entities defined there. This initial iteration focuses on providing Rust data structures and helper APIs that align with the normative constraints.

## Target Specification Alignment
- maps each key entity from the specification to strongly-typed Rust structures
- enforces required constraints using derived validation schemas via `schemars`
- exposes helper functions to compose SpecMan workspaces, scratch pads, and specifications according to the rules defined in the specification

## Implementing Language Details
The library is authored in Rust (`rust@1.91.0`) targeting the 2024 edition.

- `markdown@0.3.0` renders specification-aligned documentation snippets directly from Markdown sources
- `schemars@0.8.21` generates JSON Schema definitions from Rust types to validate persisted metadata
- `serde_json@1.0.108` provides serialization and deserialization support for SpecMan metadata artifacts

## Library Structure
The implementation will be published as a Rust crate named `specman-library@1.0.0`. Source code lives under `src/crates/specman`. The planned module layout is:

- `src/model.rs` containing entity definitions mirroring the [SpecMan Data Model](../../spec/specman-data-model/spec.md)
- `src/workspace/` encapsulating workspace discovery and validation utilities
- `src/scratchpad/` offering helpers for scratch pad metadata management

## Concept-Oriented Services
These service traits map 1:1 with the concepts defined in the [SpecMan Data Model](../../spec/specman-data-model/spec.md). Each trait focuses on enforcing the specification's constraints while providing implementation-oriented hooks.

### [SpecMan Workspace](../../spec/specman-data-model/spec.md#specman-workspace)
Workspace discovery ensures the library always anchors tooling at the nearest ancestor containing `.specman`, satisfying the MUST-level requirement for workspace detection.

```rust
use std::path::{Path, PathBuf};

pub trait WorkspaceService {
  async fn resolve(&self, options: ResolveWorkspaceConfig) -> Result<SpecmanWorkspace, WorkspaceError>;

  fn is_root(&self, path: &Path) -> bool;

  fn dot_dir(&self, workspace: &SpecmanWorkspace) -> PathBuf;
}
```

### [SpecMan Dot Folder](../../spec/specman-data-model/spec.md#specman-dot-folder)
Dot folder management keeps the `.specman` directory canonical and ensures the `scratchpad` root always exists before higher-level operations run.

```rust
use std::path::{Path, PathBuf};

pub trait DotFolderService {
  fn ensure_structure(&self, workspace: &SpecmanWorkspace) -> Result<(), WorkspaceError>;

  fn scratchpad_root(&self, workspace: &SpecmanWorkspace) -> PathBuf;

  fn list_tooling_state(&self, workspace: &SpecmanWorkspace) -> Result<Vec<PathBuf>, WorkspaceError>;
}
```

### [Scratch Pads](../../spec/specman-data-model/spec.md#scratch-pads)
Scratch pad coordination validates folder naming, metadata parsing, and lifecycle operations so pads always satisfy structural and naming constraints.

```rust
use std::path::Path;

pub trait ScratchpadService {
  fn validate_name(&self, name: &str) -> Result<(), ScratchpadError>;

  fn parse_metadata(&self, source: &str) -> Result<ScratchpadMetadata, ScratchpadError>;

  fn plan_layout(&self, plan: &ScratchpadPlan) -> Result<ScratchpadPlanResult, ScratchpadError>;

  fn retire(&self, directory: &ScratchpadDirectory) -> Result<(), ScratchpadError>;
}
```

### [Scratch Pad Metadata](../../spec/specman-data-model/spec.md#scratch-pad-metadata)
Metadata parsing extends the scratch pad service by guaranteeing work type and target combinations match the normative matrix.

```rust
pub trait ScratchpadMetadataService {
  fn validate_target(&self, metadata: &ScratchpadMetadata) -> Result<(), ScratchpadError>;

  fn normalize_branch(&self, metadata: &mut ScratchpadMetadata) -> Result<(), ScratchpadError>;
}
```

### [Specifications](../../spec/specman-data-model/spec.md#specifications)
Specification support focuses on metadata loading, dependency resolution, and folder validations to keep authored specifications compliant.

```rust
use std::path::Path;

pub trait SpecificationService {
  fn load_metadata(&self, path: &Path) -> Result<SpecificationMetadata, SpecificationError>;

  fn resolve_dependencies(
    &self,
    metadata: &SpecificationMetadata,
  ) -> Result<Vec<SpecificationDependency>, SpecificationError>;

  fn validate_structure(&self, spec_root: &Path) -> Result<(), SpecificationError>;
}
```

### [Implementations](../../spec/specman-data-model/spec.md#implementations)
Implementation services verify language declarations, references, and SemVer metadata before tooling publishes or consumes implementation artifacts.

```rust
use std::path::Path;

pub trait ImplementationService {
  fn load_metadata(&self, path: &Path) -> Result<ImplementationMetadata, ImplementationError>;

  fn validate_languages(&self, metadata: &ImplementationMetadata) -> Result<(), ImplementationError>;

  fn enumerate_references(
    &self,
    metadata: &ImplementationMetadata,
  ) -> Result<Vec<ImplementationReference>, ImplementationError>;
}
```

### [APIs](../../spec/specman-data-model/spec.md#apis)
API documentation tooling keeps the crate's Rust-facing surface aligned with specification entities and concepts via symbol registration.

```rust
pub trait ApiDocumentationService {
  fn register_signature(&self, symbol: &str, code: &str) -> Result<(), ApiError>;

  fn link_concept(&self, symbol: &str, concept: &str) -> Result<(), ApiError>;

  fn declare_entity_structure(
    &self,
    entity: &str,
    fields: &[(&str, &str)],
  ) -> Result<(), ApiError>;
}
```

### Workspace Artifact Discovery
Discovery helpers extend the workspace concept so tools can list specifications or implementations from any child directory while still respecting the nearest `.specman` root.

```rust
use std::path::{Path, PathBuf};

pub trait ArtifactDiscoveryService {
  async fn list_specifications_from(
    &self,
    start: &Path,
  ) -> Result<Vec<PathBuf>, WorkspaceError>;

  async fn list_implementations_from(
    &self,
    start: &Path,
  ) -> Result<Vec<PathBuf>, WorkspaceError>;

  async fn list_implementations_for_spec(
    &self,
    spec: &Path,
  ) -> Result<Vec<ImplementationMetadata>, ImplementationError>;

  async fn list_implementations_for_spec_and_language(
    &self,
    spec: &Path,
    language: &str,
  ) -> Result<Vec<ImplementationMetadata>, ImplementationError>;

  fn extract_heading_content(
    &self,
    artifact: &Path,
    heading: &str,
  ) -> Result<String, ArtifactError>;
}
```

### Dependency Graphs
Dependency graph traversal captures specification and implementation relationships, enforces depth limits, and detects cycles per the specification's dependency guarantees.

```rust
use std::path::Path;

pub trait DependencyGraphService {
  fn specification_graph(
    &self,
    root: &Path,
    mode: DependencyGraphMode,
    config: DependencyGraphConfig,
  ) -> Result<SpecificationDependencyGraph, DependencyGraphError>;

  fn implementation_graph(
    &self,
    root: &Path,
    mode: DependencyGraphMode,
    config: DependencyGraphConfig,
  ) -> Result<ImplementationDependencyGraph, DependencyGraphError>;
}
```

## Entity Data Models
Concrete data structures map the specification's key entities and provide serialization plus schema derivation through `serde` and `schemars`.

```rust
use std::path::PathBuf;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpecmanWorkspace {
  /// Absolute path to the workspace root; MUST contain a `.specman` child directory.
  pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpecmanDotFolder {
  /// Canonical `.specman` directory located at the workspace root.
  pub path: PathBuf,
  /// Scratch pad root at `<dot-folder>/scratchpad`.
  pub scratchpad_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScratchpadDirectory {
  /// Lowercase, hyphenated, ≤4 word scratch pad folder name.
  pub name: String,
  /// Absolute folder path inside the scratch pad root.
  pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScratchpadMetadata {
  /// Target artifact constrained by work type rules.
  pub target: TargetArtifact,
  /// Work classification among draft/revision/feat/ref.
  pub work_type: WorkType,
  /// Optional Git branch formatted as `{target_name}/{work_type}/{scratch_pad_name}`.
  pub branch: Option<String>,
  /// Additional tool-specific metadata respecting normative constraints.
  #[serde(default)]
  pub front_matter: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactLocator {
  /// Relative filesystem path or absolute URL pointing at the artifact.
  pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "locator")]
pub enum TargetArtifact {
  Specification(ArtifactLocator),
  Implementation(ArtifactLocator),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkType {
  Draft,
  Revision,
  Feat,
  Ref,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImplementationMetadata {
  pub spec: String,
  pub name: Option<String>,
  pub version: String,
  pub location: Option<String>,
  pub library: Option<LibraryReference>,
  pub primary_language: ImplementingLanguage,
  #[serde(default)]
  pub secondary_languages: Vec<ImplementingLanguage>,
  #[serde(default)]
  pub references: Vec<ImplementationReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImplementingLanguage {
  pub language: String,
  #[serde(default)]
  pub properties: serde_json::Value,
  #[serde(default)]
  pub libraries: Vec<LibraryReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LibraryReference {
  pub name: String,
  #[serde(default)]
  pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImplementationReference {
  #[serde(rename = "ref")]
  pub locator: String,
  #[serde(rename = "type")]
  pub kind: String,
  #[serde(default)]
  pub optional: bool,
}
```

Schema validation will reject values that break constraints, with additional runtime checks ensuring branch naming, locator formats, and work type/target combinations remain valid.

## Operational Use Cases
- `list_specifications_from` surfaces all known specifications, locating the nearest workspace automatically.
- `list_implementations_from` mirrors the specification traversal for implementation folders under `impl/`.
- `list_implementations_for_spec` inspects implementation metadata to associate specs with libraries.
- `list_implementations_for_spec_and_language` filters implementations by primary language identifier.
- `extract_heading_content` loads Markdown content beneath targeted headings for downstream rendering.
- `specification_graph` / `implementation_graph` generate directional dependency graphs with depth limits and cycle detection.

## Module Plan
- Workspace module exposes `resolve_workspace` and ancestor search utilities honoring the nearest `.specman` rule.
- Scratch pad module provides `parse_scratchpad_metadata` and layout planning for `.specman/scratchpad/{name}`.
- Dependency analysis module builds specification and implementation graphs with configurable traversal depth.
- Domain types encapsulate metadata (`TargetArtifact`, `WorkType`, `ScratchpadMetadata`, etc.) with validation hooks.
- Future modules cover specification utilities, implementation metadata helpers, and Markdown support via `markdown` crate.

## Additional References
- [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) for normative keyword interpretation in documentation and validation messaging.
