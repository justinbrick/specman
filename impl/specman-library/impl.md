---
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

- **Primary — `rust@1.91.0` (2024 edition):** The crate leverages the stable 1.91 toolchain to compose performant, strongly typed services. Rust traits organize entry-point catalogs, while enums and structs model deterministic function descriptors, `DependencyTree`, and `TemplateDescriptor`. Edition 2024 features (inline `let`, `if-let` chains, and `impl Trait` in return position) keep the ergonomics modern without compromising MSRV requirements. Supporting crates include:
  - `schemars` for deriving JSON Schema artifacts tied to the SpecMan Data Model entities.
  - `serde_json` for canonical serialization/deserialization of request and response payloads.
  - `markdown` for parsing templates and emitting deterministic Markdown outputs for generated specifications, implementations, and scratch pads.
- **Secondary languages:** None. All orchestration, validation, and I/O run in Rust; shell glue or scripting layers consume the library through binary wrappers built from the same crate.

## Concept: Workspace Discovery

This module fulfills [Concept: Workspace Discovery](../../spec/specman-core/spec.md#concept-workspace-discovery) by providing reusable locators that normalize the current directory, enforce `.specman` ancestry rules, and cache the result for callers across the crate and downstream tooling.

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspacePaths {
  root: PathBuf,
  dot_specman: PathBuf,
}

impl WorkspacePaths {
  pub fn new(root: PathBuf, dot_specman: PathBuf) -> Self {
    Self { root, dot_specman }
  }

  pub fn root(&self) -> &Path {
    &self.root
  }

  pub fn dot_specman(&self) -> &Path {
    &self.dot_specman
  }

  pub fn spec_dir(&self) -> PathBuf {
    self.root.join("spec")
  }

  pub fn impl_dir(&self) -> PathBuf {
    self.root.join("impl")
  }

  pub fn scratchpad_dir(&self) -> PathBuf {
    self.dot_specman.join("scratchpad")
  }
}

pub trait WorkspaceLocator: Send + Sync {
  fn workspace(&self) -> Result<WorkspacePaths, SpecmanError>;
}

pub struct FilesystemWorkspaceLocator {
  start: PathBuf,
  cache: Mutex<Option<WorkspacePaths>>,
}

impl FilesystemWorkspaceLocator {
  pub fn new(start: impl Into<PathBuf>) -> Self {
    Self {
      start: start.into(),
      cache: Mutex::new(None),
    }
  }

  pub fn from_current_dir() -> Result<Self, SpecmanError> {
    Ok(Self::new(env::current_dir()?))
  }
}

impl WorkspaceLocator for FilesystemWorkspaceLocator {
  fn workspace(&self) -> Result<WorkspacePaths, SpecmanError> {
    if let Some(paths) = self.cache.lock().unwrap().clone() {
      if paths.root().is_dir() && paths.dot_specman().is_dir() {
        return Ok(paths);
      }
    }

    let discovered = discover(&self.start)?;
    *self.cache.lock().unwrap() = Some(discovered.clone());
    Ok(discovered)
  }
}

pub fn discover(start: impl AsRef<Path>) -> Result<WorkspacePaths, SpecmanError> {
  let canonical_start = normalize_start(start.as_ref())?;

  for ancestor in canonical_start.ancestors() {
    let candidate = ancestor.join(".specman");
    if candidate.is_dir() {
      return Ok(WorkspacePaths::new(ancestor.to_path_buf(), candidate));
    }
  }

  Err(SpecmanError::Workspace(format!(
    "no .specman directory found from {}",
    canonical_start.display()
  )))
}
```

- All workspace APIs return canonicalized paths and emit `SpecmanError::Workspace` when `.specman` is missing, ensuring downstream services never operate outside the active workspace root.
- The locator cache is automatically invalidated when the `.specman` directory disappears, keeping long-running CLI sessions compliant with the spec’s revalidation requirement.
- `normalize_start` (not shown) guarantees that discovery begins from a real directory, even when callers hand the locator a not-yet-created file path.
- Workspace discovery output seeds every other concept (dependency mapping, metadata mutation, lifecycle automation) so branch tooling stays within the same root.

## Concept: Data Model Backing Implementation

Per [Concept: Data Model Backing Implementation](../../spec/specman-core/spec.md#concept-data-model-backing-implementation), persistence and schema contracts rely on adapters plus workspace persistence helpers. These APIs expose deterministic serialization while sourcing JSON Schema via `schemars` for each struct listed below.

```rust
pub trait DataModelAdapter: Send + Sync {
  fn save_dependency_tree(&self, tree: DependencyTree) -> Result<(), SpecmanError>;
  fn load_dependency_tree(
    &self,
    root: &ArtifactId,
  ) -> Result<Option<DependencyTree>, SpecmanError>;
  fn invalidate_dependency_tree(&self, root: &ArtifactId) -> Result<(), SpecmanError>;
}

#[derive(Default)]
pub struct InMemoryAdapter {
  dependency_trees: Mutex<BTreeMap<ArtifactId, DependencyTree>>,
}

pub struct WorkspacePersistence<L: WorkspaceLocator> {
  locator: L,
}

impl<L: WorkspaceLocator> WorkspacePersistence<L> {
  pub fn new(locator: L) -> Self { Self { locator } }
  pub fn persist(
    &self,
    artifact: &ArtifactId,
    rendered: &RenderedTemplate,
  ) -> Result<PersistedArtifact, SpecmanError>;
  pub fn remove(&self, artifact: &ArtifactId) -> Result<RemovedArtifact, SpecmanError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistedArtifact {
  pub artifact: ArtifactId,
  pub path: PathBuf,
  pub workspace: WorkspacePaths,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovedArtifact {
  pub artifact: ArtifactId,
  pub directory: PathBuf,
  pub workspace: WorkspacePaths,
}

pub trait ArtifactRemovalStore: Send + Sync {
  fn remove_artifact(&self, artifact: &ArtifactId) -> Result<RemovedArtifact, SpecmanError>;
}

pub enum EntityKind { Specification, Implementation, ScratchPad, Template, Other(String) }
pub struct SchemaRef { pub name: String, pub version: Option<SemVer>, pub schema: serde_json::Value }
```

- Persistence helpers live under this concept (per user guidance) because they ensure rendered Markdown conforms to the SpecMan Data Model and that deletions remove the canonical directories described by the spec.
- `SchemaRef` and `EntityKind` annotate API inputs/outputs with JSON schema metadata while `SemVer` (re-exported from `semver`) enforces semantic versioning for stored dependency trees.
- Adapter implementations (in-memory for tests, disk-backed for tooling) share the same trait contract so CLI and services can swap storage without violating SpecMan Core’s deterministic serialization rules.

## Concept: Dependency Mapping Services

This section satisfies [Concept: Dependency Mapping Services](../../spec/specman-core/spec.md#concept-dependency-mapping-services) by exposing traversal traits plus a filesystem-backed mapper that normalizes filesystem and HTTPS locators, builds aggregate dependency trees, and serializes them through serde/schemars when needed (for example, cycle detection errors).

```rust
pub trait ContentFetcher: Send + Sync {
  fn fetch(&self, url: &Url) -> Result<String, SpecmanError>;
}

pub trait DependencyMapping: Send + Sync {
  fn dependency_tree(&self, root: &ArtifactId) -> Result<DependencyTree, SpecmanError>;
  fn upstream(&self, root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError>;
  fn downstream(&self, root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError>;
}

pub struct FilesystemDependencyMapper<L: WorkspaceLocator> {
  workspace: L,
  fetcher: Arc<dyn ContentFetcher>,
}

impl<L: WorkspaceLocator> FilesystemDependencyMapper<L> {
  pub fn new(workspace: L) -> Self;
  pub fn with_fetcher(workspace: L, fetcher: Arc<dyn ContentFetcher>) -> Self;
  pub fn dependency_tree_from_path(
    &self,
    path: impl AsRef<Path>,
  ) -> Result<DependencyTree, SpecmanError>;
  pub fn dependency_tree_from_url(&self, url: &str) -> Result<DependencyTree, SpecmanError>;
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct DependencyTree {
  pub root: ArtifactSummary,
  pub upstream: Vec<DependencyEdge>,
  pub downstream: Vec<DependencyEdge>,
  pub aggregate: Vec<DependencyEdge>,
}
```

- Traversal stores the aggregate edge set so aggregate serialization (`serde_json::to_string(&tree)`) can be returned verbatim whenever a dependency cycle triggers a `SpecmanError::Dependency`. That serialized payload doubles as the JSON Schema example mandated by this concept.
- Upstream, downstream, and aggregate vectors follow the SpecMan Data Model entity definitions while `has_blocking_dependents()` enforces lifecycle guardrails for creation/deletion flows.
- Downstream scans rely on `WorkspaceInventory::build` to walk every artifact under `spec/`, `impl/`, and `.specman/scratchpad`, guaranteeing that optional edges remain visible while never blocking deletions unless the `optional` flag is false (or the artifact is a scratch pad referencing another scratch pad).

## Concept: Template Orchestration

Template orchestration honors [Concept: Template Orchestration](../../spec/specman-core/spec.md#concept-template-orchestration) and references [SpecMan Templates](../../spec/specman-templates/spec.md) for token governance. All template APIs strictly resolve filesystem paths; sample token maps remain centralized in the template specification to avoid drift.

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum TemplateScenario { Specification, Implementation, ScratchPad, WorkType(String) }

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum TemplateLocator { FilePath(PathBuf), Url(String) }

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct TemplateDescriptor {
  pub locator: TemplateLocator,
  pub scenario: TemplateScenario,
  pub required_tokens: Vec<String>,
}

pub type TokenMap = BTreeMap<String, serde_json::Value>;

pub trait TemplateEngine: Send + Sync {
  fn render(
    &self,
    descriptor: &TemplateDescriptor,
    tokens: &TokenMap,
  ) -> Result<RenderedTemplate, SpecmanError>;
}

#[derive(Default)]
pub struct MarkdownTemplateEngine;

impl TemplateEngine for MarkdownTemplateEngine {
  fn render(&self, descriptor: &TemplateDescriptor, tokens: &TokenMap) -> Result<RenderedTemplate, SpecmanError> { /* token substitution */ }
}
```

- The engine validates that every `required_tokens` entry is present and fails fast with `SpecmanError::Template` if unresolved `{{token}}` placeholders remain.
- File-based locators use repository-relative paths for portability; remote templates intentionally error until SpecMan Templates adds a governance workflow for network fetching.
- Rendering results feed directly into `WorkspacePersistence::persist`, ensuring lifecycle tooling always writes deterministic Markdown that already satisfies all template instructions.

## Concept: Deterministic Execution

This concept (see [Deterministic Execution](../../spec/specman-core/spec.md#concept-deterministic-execution)) documents the shared error surface and logging strategy. Every exported function either returns the requested struct or one of the `SpecmanError` variants below—listing every variant per user directive ensures downstream tooling plans for each failure mode.

```rust
#[derive(Debug, Error)]
pub enum SpecmanError {
  #[error("template error: {0}")]
  Template(String),
  #[error("dependency error: {0}")]
  Dependency(String),
  #[error("workspace error: {0}")]
  Workspace(String),
  #[error("serialization error: {0}")]
  Serialization(String),
  #[error("io error: {0}")]
  Io(#[from] std::io::Error),
}
```

- Each module wraps lower-level failures using `SpecmanError::context(...)` so logs capture both the origin (e.g., dependency traversal, metadata mutation) and the root cause.
- Determinism requirements translate into stable error messaging and major version bumps whenever new variants or behavior changes would affect observability guarantees.

## Concept: Lifecycle Automation

Lifecycle orchestration (per [Concept: Lifecycle Automation](../../spec/specman-core/spec.md#concept-lifecycle-automation)) coordinates dependency inspection, template rendering, persistence, and deletion guardrails. All deletion guard details live here, covering scratch pad exemptions and force overrides as requested.

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreationRequest { pub target: ArtifactId, pub template: TemplateDescriptor, pub tokens: TokenMap }

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreationPlan { pub rendered: RenderedTemplate, pub dependencies: DependencyTree }

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeletionPlan { pub dependencies: DependencyTree, pub blocked: bool }

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScratchPadPlan { pub rendered: RenderedTemplate, pub profile: ScratchPadProfile }

pub trait LifecycleController: Send + Sync {
  fn plan_creation(&self, request: CreationRequest) -> Result<CreationPlan, SpecmanError>;
  fn plan_deletion(&self, target: ArtifactId) -> Result<DeletionPlan, SpecmanError>;
  fn plan_scratchpad(&self, profile: ScratchPadProfile) -> Result<ScratchPadPlan, SpecmanError>;
  fn execute_deletion(
    &self,
    target: ArtifactId,
    existing_plan: Option<DeletionPlan>,
    persistence: &dyn ArtifactRemovalStore,
    force: bool,
  ) -> Result<RemovedArtifact, SpecmanError>;
}

pub struct DefaultLifecycleController<M, T, A> {
  mapping: M,
  templates: T,
  adapter: A,
}
```

- `plan_deletion` computes `blocked` by delegating to `DependencyTree::has_blocking_dependents()`. Scratch pads only block on other scratch pads, while specs and implementations block on any non-optional downstream edge.
- `execute_deletion` recomputes the plan when necessary, rejects mismatched targets, consults `force` overrides, invokes `ArtifactRemovalStore::remove_artifact`, and invalidates cached dependency trees via the adapter. This keeps lifecycle behavior symmetric with creation flows.
- Scratch pad creation flows hydrate `ScratchPadProfile::token_map()` before persisting the rendered Markdown inside `.specman/scratchpad/{slug}` consistently with the SpecMan Data Model naming rules.

## Concept: Metadata Mutation

Metadata mutation (see [Concept: Metadata Mutation](../../spec/specman-core/spec.md#concept-metadata-mutation)) ensures YAML front matter edits remain idempotent while respecting workspace boundaries.

```rust
pub struct MetadataMutator<L: WorkspaceLocator> {
  workspace: L,
  adapter: Option<Arc<dyn DataModelAdapter>>,
}

impl<L: WorkspaceLocator> MetadataMutator<L> {
  pub fn new(workspace: L) -> Self { Self { workspace, adapter: None } }
  pub fn with_adapter(workspace: L, adapter: Arc<dyn DataModelAdapter>) -> Self { Self { workspace, adapter: Some(adapter) } }
  pub fn mutate(&self, request: MetadataMutationRequest) -> Result<MetadataMutationResult, SpecmanError>;
}

#[derive(Debug, Default)]
pub struct MetadataMutationRequest {
  pub path: PathBuf,
  pub add_dependencies: Vec<String>,
  pub add_references: Vec<ReferenceAddition>,
  pub persist: bool,
}

#[derive(Debug, Clone)]
pub struct ReferenceAddition {
  pub locator: String,
  pub reference_type: Option<String>,
  pub optional: Option<bool>,
}

#[derive(Debug)]
pub struct MetadataMutationResult {
  pub artifact: ArtifactId,
  pub updated_document: String,
  pub persisted: Option<PersistedArtifact>,
}
```

- The mutator only accepts HTTPS or workspace-relative locators, aligning with dependency traversal rules and preventing workspace escapes.
- Persisted mutations invalidate cached dependency trees through the optional adapter hook; schemars output is reused instead of embedding raw JSON schemas in this document, per user instruction.
- No sample token maps are duplicated here—template-specific data remains governed by [spec/specman-templates/spec.md](../../spec/specman-templates/spec.md).

### Artifact-Specific Front Matter Schemas

- `src/crates/specman/src/front_matter.rs` now models specification, implementation, and scratch metadata via dedicated structs (`SpecificationFrontMatter`, `ImplementationFrontMatter`, `ScratchFrontMatter`) plus the `ArtifactFrontMatter` enum. Each struct derives `Serialize`, `Deserialize`, and `JsonSchema`, and their doc comments cite the relevant paragraphs in [SpecMan Data Model](../../spec/specman-data-model/spec.md) to satisfy the Stage 4 comment-alignment requirement.
- The new `ScratchWorkType` enum encodes the draft/revision/feat/ref/fix discriminators exactly as defined in the data model. A manual `JsonSchema` implementation ensures schema consumers see a single-key object that mirrors the YAML shape (`work_type: { feat: {} }`, etc.), preserving deterministic serialization for Stage 5.
- Downstream consumers (dependency traversal, metadata mutator, and CLI summaries) no longer deserialize ad-hoc `RawFrontMatter` maps. Instead they call `ArtifactFrontMatter::from_yaml_value`, match on the artifact variant, and rely on typed fields (`identity`, `dependencies`, `references`, `work_type`) for validation. This keeps lifecycle code focused on orchestration while the front-matter module owns schema fidelity.
- CLI summaries (`spec`, `implementation`, `scratch` commands) now use the typed structs to surface names, versions, branches, targets, and work types without re-parsing YAML manually, eliminating the duplicated `serde_yaml::Value` logic noted in Stage 5 planning.
- Dedicated unit tests in `front_matter.rs` exercise the new parser/serializer paths so regressions in schema alignment (especially work-type payloads) are caught before lifecycle planning or CLI summaries consume invalid structures.

## Entity: DataModelAdapter

[Entity: DataModelAdapter](../../spec/specman-core/spec.md#entity-datamodeladapter) formalizes persistence hooks for dependency trees and cache invalidation.

```rust
pub trait DataModelAdapter: Send + Sync {
  fn save_dependency_tree(&self, tree: DependencyTree) -> Result<(), SpecmanError>;
  fn load_dependency_tree(
    &self,
    root: &ArtifactId,
  ) -> Result<Option<DependencyTree>, SpecmanError>;
  fn invalidate_dependency_tree(&self, root: &ArtifactId) -> Result<(), SpecmanError>;
}

#[derive(Default)]
pub struct InMemoryAdapter {
  dependency_trees: Mutex<BTreeMap<ArtifactId, DependencyTree>>,
}
```

- The in-memory adapter underpins tests and local experiments, while production tooling can provide a disk-backed adapter by implementing the same trait. Both paths emit schemars-compatible dependency trees.
- Adapter hooks are invoked by lifecycle automation (on creation/deletion) and metadata mutation (on persist) to keep cached graphs consistent.

## Entity: DependencyTree

The [DependencyTree entity](../../spec/specman-core/spec.md#entity-dependencytree) captures traversal results for every artifact type.

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct ArtifactSummary {
  pub id: ArtifactId,
  pub version: Option<SemVer>,
  pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct DependencyEdge {
  pub from: ArtifactSummary,
  pub to: ArtifactSummary,
  pub relation: DependencyRelation,
  pub optional: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct DependencyTree {
  pub root: ArtifactSummary,
  pub upstream: Vec<DependencyEdge>,
  pub downstream: Vec<DependencyEdge>,
  pub aggregate: Vec<DependencyEdge>,
}
```

- JSON schemas for these structs are produced at build time via `schemars` so tools can consume the canonical shape without this document duplicating the output.
- `DependencyTree::has_blocking_dependents()` enforces deletion guard policies referenced under Lifecycle Automation.

## Entity: TemplateDescriptor

[Entity: TemplateDescriptor](../../spec/specman-core/spec.md#entity-templatedescriptor) documents the metadata necessary to render Markdown artifacts. `RenderedTemplate` is documented alongside it per user guidance.

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct TemplateDescriptor {
  pub locator: TemplateLocator,
  pub scenario: TemplateScenario,
  pub required_tokens: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct RenderedTemplate {
  pub body: String,
  pub metadata: TemplateDescriptor,
}
```

- The descriptor ensures template locators, scenarios, and required tokens travel together; rendering echoes that metadata back to callers alongside the fully substituted Markdown body.
- Sample token maps remain defined by the template specification to avoid drift between documentation and actual templates.

## Entity: LifecycleController

Lifecycle controller abstractions align with [Entity: LifecycleController](../../spec/specman-core/spec.md#entity-lifecyclecontroller).

```rust
pub trait LifecycleController: Send + Sync {
  fn plan_creation(&self, request: CreationRequest) -> Result<CreationPlan, SpecmanError>;
  fn plan_deletion(&self, target: ArtifactId) -> Result<DeletionPlan, SpecmanError>;
  fn plan_scratchpad(&self, profile: ScratchPadProfile) -> Result<ScratchPadPlan, SpecmanError>;
  fn execute_deletion(
    &self,
    target: ArtifactId,
    existing_plan: Option<DeletionPlan>,
    persistence: &dyn ArtifactRemovalStore,
    force: bool,
  ) -> Result<RemovedArtifact, SpecmanError>;
}

pub struct DefaultLifecycleController<M, T, A> {
  mapping: M,
  templates: T,
  adapter: A,
}
```

- Default controller implementations enforce the deletion guard behaviors described earlier, surface explicit errors on blocked deletions, and invalidate cached dependency trees after successful removals.
- Scratch pad plans reuse the template metadata defined under TemplateDescriptor and persist outputs under `.specman/scratchpad/{slug}`, matching the SpecMan Data Model storage rules.

## Entity: ScratchPadProfile

[Entity: ScratchPadProfile](../../spec/specman-core/spec.md#entity-scratchpadprofile) defines reusable scratch pad templates while linking to the actual Markdown sources under `templates/scratch/` (no inline catalog to prevent drift, per user instruction).

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct ScratchPadProfile {
  pub name: String,
  pub template: TemplateDescriptor,
  #[serde(default)]
  pub configuration: BTreeMap<String, serde_json::Value>,
}

impl ScratchPadProfile {
  pub fn token_map(&self) -> TokenMap {
    self.configuration
      .iter()
      .map(|(k, v)| (k.clone(), v.clone()))
      .collect()
  }
}
```

- Each profile pairs a template with an arbitrary configuration map, letting lifecycle automation hydrate the correct token map before rendering.
- Profiles link back to authored Markdown templates (for example, `templates/scratch/scratch.md`) rather than embedding template copies here, keeping documentation concise while still traceable.

## Operational Notes

- **Build & Testing:** Run `cargo build -p specman` and `cargo test -p specman` from the `src` directory to exercise the workspace crate with Rust 1.91. Clippy and fmt gates should run in CI to enforce style and catch regressions.
- **Configuration:** Template locators are read from environment variables (`SPECMAN_TEMPLATE_ROOT`) or CLI flags before falling back to repository-relative defaults, satisfying the extensibility guidance from [Template Orchestration](../../spec/specman-core/spec.md#concept-template-orchestration).
- **Lifecycle Automation:** Delete operations must call `dependency_tree` first and abort when downstream edges exist, returning the serialized tree to the caller as mandated by [Lifecycle Automation](../../spec/specman-core/spec.md#concept-lifecycle-automation).
- **Observability:** Each public function logs structured events (entity name, version, dependency counts) so operators can trace execution and audit compliance with the [SpecMan Data Model](../../spec/specman-data-model/spec.md#implementations).

Together, these notes ensure the implementation remains compliant with the SpecMan Data Model sections covering Implementations, Implementing Language, References, APIs, and Metadata while providing actionable guidance for practitioners running the crate.
