---
spec: ../../spec/specman-core/spec.md
name: specman-library
version: "2.1.1"
location: ../../src/crates/specman
library:
  name: specman-library@2.1.1
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

## Concept: Reference Validation

The `reference_validation` module provides deterministic validation of Markdown link destinations within a workspace. This is used to catch broken intra-document fragments, missing filesystem targets, and invalid/unsupported destination forms.

Key behaviors:

- **CommonMark link discovery:** Extracts inline links (`[text](dest)`) and defined reference links (`[text][id]` + `[id]: dest`). Image destinations are ignored.
- **Destination classification:** Distinguishes workspace filesystem targets, `https://` URLs, fragment-only links (`#slug`), and unsupported destinations.
- **Workspace boundary enforcement:** Resolves filesystem destinations relative to the current document directory and rejects any path that escapes the workspace root.
- **Fragment validation:** Validates fragments against heading slugs computed using the SpecMan slug algorithm defined in [Concept: Markdown Slugs](../../spec/specman-data-model/spec.md#concept-markdown-slugs).
- **Transitive validation:** When enabled, recursively validates linked Markdown documents up to `max_documents`, and validates cross-document fragments (`other.md#slug`) against the target document's headings.
- **Optional HTTPS reachability:** When configured, can perform network checks for `https://` destinations.

Note: The slug implementation follows the SpecMan steps (NFKD normalization, filtering, hyphenation, cleanup, deduplication). Unicode case folding is approximated via Unicode lowercasing to avoid introducing additional dependencies.

Public entry point:

```rust
pub fn validate_references(
  locator: &str,
  workspace: &WorkspacePaths,
  options: ReferenceValidationOptions,
) -> Result<ReferenceValidationReport, SpecmanError>;
```

## Concept: Workspace Discovery

This module fulfills [Concept: Workspace Discovery](../../spec/specman-core/spec.md#concept-workspace-discovery) with a dedicated initializer, typed errors, and cached locator resolution that keeps callers anchored to the nearest `.specman` ancestor.

```rust
#[derive(Debug, Error)]
pub enum WorkspaceError {
  NotFound { searched_from: PathBuf },
  DotSpecmanMissing { workspace_root: PathBuf },
  InvalidStart { start: PathBuf, message: String },
  InvalidHandle { locator: String, message: String },
  OutsideWorkspace { candidate: PathBuf, workspace_root: PathBuf },
  Io(#[from] std::io::Error),
}

pub struct WorkspaceDiscovery;
impl WorkspaceDiscovery {
  pub fn initialize(start_path: impl Into<PathBuf>) -> Result<WorkspaceContext, WorkspaceError>;
  pub fn from_explicit(workspace_root: impl Into<PathBuf>) -> Result<WorkspaceContext, WorkspaceError>;
  pub fn create(workspace_root: impl Into<PathBuf>) -> Result<WorkspaceContext, WorkspaceError>;
}

#[derive(Clone, Debug)]
pub struct WorkspaceContext { /* caches resolved locators */ }
impl WorkspaceContext {
  pub fn paths(&self) -> &WorkspacePaths;
  pub fn resolve_locator(&self, locator: impl AsRef<str>) -> Result<PathBuf, WorkspaceError>;
}
```

- `initialize` walks ancestors from any starting path (file or directory) to find the nearest `.specman`, preserving symlinked segments via lexical normalization instead of `realpath`. It errors with `WorkspaceError::NotFound` when no workspace exists.
- `from_explicit` validates an explicit workspace root and requires an on-disk `.specman`, returning `WorkspaceError::DotSpecmanMissing` when absent and `WorkspaceError::InvalidStart` when the root is not a directory.
- `create` provisions `.specman` plus required subdirectories (`scratchpad/`, `cache/`) at an explicit root, rejects nested workspace creation (`NestedWorkspace`), and returns a ready `WorkspaceContext` for resolver reuse.
- `WorkspaceContext::resolve_locator` accepts SpecMan handles (`spec://`, `impl://`, `scratch://`) and workspace-relative paths, rejects `http(s)://` inputs, and enforces workspace boundaries with `OutsideWorkspace` before memoizing the result.
- `FilesystemWorkspaceLocator` wraps discovery with lightweight caching and revalidation so CLI runs never reuse a stale root; the compatibility shim `discover()` still returns raw `WorkspacePaths` for legacy callers.
- `WorkspacePaths` continues to expose `spec_dir`, `impl_dir`, and `scratchpad_dir` helpers to centralize canonical layout derived from the discovered root.

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
  pub fn dependency_tree_from_locator(
    &self,
    reference: &str,
  ) -> Result<DependencyTree, SpecmanError>;
  pub fn dependency_tree_from_locator_best_effort(
    &self,
    reference: &str,
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

Resource handles are now parsed centrally via `ArtifactLocator::from_reference`, which consumes
workspace paths, HTTPS URLs, and the SpecMan-specific `spec://`, `impl://`, and `scratch://`
schemes. Handles normalize identifiers (lowercase, single-segment slugs) before resolving to
workspace files, ensuring every downstream consumer works with canonical filesystem paths.

```rust
#[derive(Clone, Debug)]
enum ArtifactLocator { File(PathBuf), Url(Url) }

impl ArtifactLocator {
  pub fn from_reference(reference: &str, workspace: &WorkspacePaths) -> Result<Self, SpecmanError> {
    if reference.starts_with("https://") {
      return Self::from_url(reference);
    }

    if let Some(handle) = ResourceHandle::parse(reference)? {
      return handle.into_locator(workspace);
    }

    Self::from_path(reference, workspace, Some(workspace.root()))
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResourceHandle { kind: ArtifactKind, slug: String }
```

- Traversal stores the aggregate edge set so aggregate serialization (`serde_json::to_string(&tree)`) can be returned verbatim whenever a dependency cycle triggers a `SpecmanError::Dependency`. That serialized payload doubles as the JSON Schema example mandated by this concept.
- `dependency_tree_from_locator` is the public entry point for every CLI/adapter call and delegates to the shared parser, so resource handles, workspace paths, and HTTPS URLs share identical validation + error messaging. The legacy URL helper remains as a thin shim for backwards compatibility.
- `dependency_tree_from_locator_best_effort` is reserved for context/prompt emission; it annotates `ArtifactSummary.resolution` (strict vs. best-match file/url) and `ArtifactSummary.resolved_path` (workspace path or HTTPS URL) so callers can favor resolvable paths when strict handles are missing.
- Upstream, downstream, and aggregate vectors follow the SpecMan Data Model entity definitions while `has_blocking_dependents()` enforces lifecycle guardrails for creation/deletion flows.
- Downstream scans rely on `WorkspaceInventory::build` to walk every artifact under `spec/`, `impl/`, and `.specman/scratchpad`, guaranteeing that optional edges remain visible while never blocking deletions unless the `optional` flag is false (or the artifact is a scratch pad referencing another scratch pad).

## Concept: SpecMan Structure

This module fulfills [Concept: SpecMan Structure](../../spec/specman-core/spec.md#concept-specman-structure) by providing **parsing-only** structure indexing and discovery utilities over the local workspace.

- **One-shot indexing:** the indexer walks canonical artifact documents (`spec/*/spec.md`, `impl/*/impl.md`, `.specman/scratchpad/*/scratch.md`), parses headings + heading content, extracts constraint identifier lines, and records relationships derived from inline links.
- **Deterministic side effects:** indexing is parsing-only and deterministic; `build_once` is read-only and returns a fully in-memory `WorkspaceIndex`. A separate cached path may persist a cache under `.specman/cache/index` (safe to delete) to avoid re-parsing unchanged workspaces.
- **Workspace boundaries:** all indexed files and all workspace-local link targets are validated to remain inside the discovered workspace root.
- **Duplicate heading slugs:** indexing fails fast when two headings within the same document resolve to the same slug (this is a deliberate stricter behavior than the disambiguation rule described in the data model’s slug concept).

## Feature: Structure Index Persistence

The library includes an **optional, disk-backed cache** for the workspace structure index to speed up repeated operations without changing indexing semantics.

- **Cache root (hard requirement):** `.specman/cache/index/`
- **Workspace identity binding:** `.specman/root_fingerprint` stores a stable per-workspace identifier used to prevent accidental cross-workspace cache reuse.
- **Persisted scope:** only canonical `spec/*/spec.md` and `impl/*/impl.md` artifacts are persisted. Scratch pads are still indexed in-memory for the current invocation but are excluded from the persisted cache.

On-disk layout:

- `.specman/cache/index/manifest.json`
  - Records `schema_version`, `workspace_root_fingerprint`, `generated_at_unix_ms`, and a deterministic list of cached artifacts with `workspace_path`, `kind`, `mtime_unix_ms`, and `size`.
- `.specman/cache/index/index.v{schema_version}.json`
  - Serialized representation of the cached (spec+impl) portion of `WorkspaceIndex`.
- `.specman/cache/index/.lock`
  - Fail-fast lock file used to prevent concurrent writers from corrupting the cache.

Freshness + invalidation:

- Any mismatch in schema version, workspace fingerprint, artifact existence, artifact `mtime/size`, or artifact set membership (added/removed spec/impl documents) invalidates the cache.
- Missing/corrupt/partially written JSON is treated as a cache miss and triggers a full rebuild followed by cache overwrite.
- If a lock file is present, index cache operations fail fast with a descriptive `SpecmanError::Workspace` rather than attempting a read.

Public entry points:

- `FilesystemStructureIndexer::build_cached()` / `build_cached_with_workspace(...)`
  - Loads a fresh cache when available; otherwise rebuilds and refreshes the cache.
- `FilesystemStructureIndexer::purge_index_cache()`
  - Deletes `.specman/cache/index/` (cache-only; safe to remove).

API surface (parsing + in-memory queries):

```rust
pub struct WorkspaceIndex {
  pub schema_version: u32,
  pub workspace_root: PathBuf,
  pub artifacts: BTreeMap<ArtifactKey, ArtifactRecord>,
  pub headings: BTreeMap<HeadingIdentifier, HeadingRecord>,
  pub constraints: BTreeMap<ConstraintIdentifier, ConstraintRecord>,
  pub relationships: Vec<RelationshipEdge>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactKey {
  pub kind: ArtifactKind,
  pub workspace_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeadingIdentifier {
  pub artifact: ArtifactKey,
  pub slug: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstraintIdentifier {
  pub artifact: ArtifactKey,
  pub group: String,
}

pub trait StructureIndexing: Send + Sync {
  fn build_once(&self) -> Result<WorkspaceIndex, SpecmanError>;
}

pub struct FilesystemStructureIndexer<L: WorkspaceLocator> { /* workspace locator */ }

impl<L: WorkspaceLocator> FilesystemStructureIndexer<L> {
  pub fn build_cached(&self) -> Result<WorkspaceIndex, SpecmanError>;
  pub fn purge_index_cache(&self) -> Result<(), SpecmanError>;
}

pub trait StructureQuery {
  fn list_heading_slugs(&self) -> Vec<HeadingIdentifier>;
  fn list_constraint_groups(&self) -> Vec<ConstraintIdentifier>;
  fn render_heading(&self, heading: &HeadingIdentifier) -> Result<String, SpecmanError>;
  fn render_heading_by_slug(&self, slug: &str) -> Result<String, SpecmanError>;
  fn render_constraint_group(&self, group: &ConstraintIdentifier) -> Result<String, SpecmanError>;
}
```

Rendering follows the Structure Discovery requirements: `render_heading` returns the requested section and then appends content for headings referenced via inline links in the order they are referenced, deduplicating repeated references so each heading section appears at most once.

## Concept: Template Orchestration

Template orchestration honors [Concept: Template Orchestration](../../spec/specman-core/spec.md#concept-template-orchestration), which now owns the token contract, HTML-instruction rules, and pointer-governance requirements. All template APIs strictly resolve filesystem paths; sample token maps remain centralized in the template specification to avoid drift.

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
- File-based locators use repository-relative paths for portability; remote templates intentionally error until SpecMan Core authorizes a governance workflow for network fetching.
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

## Concept: Lifecycle Service Façade

To make lifecycle flows easier to consume correctly from hosts (CLI/MCP), the library now provides a high-level façade (`specman::Specman`) that owns lifecycle orchestration + template resolution + persistence. This removes the need for callers to wire multiple low-level services (controller, catalog, persistence) for common operations.

Key exported façade types:

```rust
pub struct Specman<M, T, L> { /* controller + catalog + persistence */ }

pub enum CreateRequest { /* Specification | Implementation | ScratchPad | Custom */ }
pub struct CreatePlan { pub artifact: ArtifactId, pub rendered: RenderedTemplate }

pub struct DeleteRequest { pub target: ArtifactId, pub plan: Option<DeletePlan>, pub policy: DeletePolicy }
pub struct DeletePlan { pub target: ArtifactId, pub dependencies: DependencyTree, pub blocked: bool }

pub enum SpecmanError { /* ... */ Lifecycle(LifecycleError), /* ... */ }
pub enum LifecycleError { DeletionBlocked { target: ArtifactId }, PlanTargetMismatch { requested: ArtifactId, planned: ArtifactId }, /* ... */ }
```

- The façade returns structured lifecycle failures (`SpecmanError::Lifecycle(...)`) so callers can branch on “blocked deletion” vs “plan mismatch” without string matching.
- The façade keeps lower-level traits (`DependencyMapping`, `TemplateEngine`, `WorkspaceLocator`) reusable for tests and alternate hosts.

### Create: Front Matter Inputs

Create requests can optionally provide fully-typed front matter at creation time so the first persisted write already includes caller-supplied metadata (rather than requiring a follow-up mutation step).

```rust
pub enum CreateRequest {
  Specification {
    context: SpecificationContext,
    front_matter: Option<SpecificationFrontMatter>,
  },
  Implementation {
    context: ImplementationContext,
    front_matter: Option<ImplementationFrontMatter>,
  },
  ScratchPad {
    context: ScratchPadContext,
    front_matter: Option<ScratchFrontMatter>,
  },
  Custom { /* ... */ },
}
```

- The façade renders templates first, then merges/synthesizes YAML front matter before persisting.
- When the template output contains front matter, the create request’s supplied front matter overrides the corresponding keys.
- Locator fields in front matter (dependencies, references, `spec`, scratch targets) accept `spec://` / `impl://` / `scratch://` inputs but are normalized before writing so persisted documents store only workspace-relative paths (forward slashes) or `https://` URLs. Plain `http://` is rejected.
- Scratch pad targets are normalized relative to the workspace root (not the scratch file directory) to match dependency resolution semantics.

### Update: Front Matter Mutation

In addition to the existing path-based `MetadataMutator`, the library provides an artifact-oriented API that updates only YAML front matter via a tagged enum, preserving the Markdown body exactly.

```rust
pub struct FrontMatterUpdateRequest {
  pub persist: bool,
  pub ops: Vec<FrontMatterUpdateOp>,
}

pub enum FrontMatterUpdateOp {
  // Tagged enum (serde) with artifact-specific operations.
  // Examples: set_name, set_title, add_dependency, remove_reference, set_spec, clear_target, ...
}

impl<M, T, L> Specman<M, T, L> {
  pub fn update(
    &self,
    target: ArtifactId,
    update: FrontMatterUpdateRequest,
  ) -> Result<FrontMatterUpdateResult, SpecmanError>;
}
```

- Update reads the artifact’s existing Markdown, splits front matter vs body, applies ops to typed front matter, then re-serializes YAML + reattaches the original body unchanged.
- Updates enforce kind compatibility (spec ops cannot be applied to an impl file, etc.) and apply the same locator normalization rules as create.
- If an artifact has no front matter, update synthesizes a front matter mapping before applying ops.

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
- `plan_creation` tolerates brand-new artifacts (missing target files) by planning against an empty `DependencyTree`, enabling “plan → render → persist” flows without pre-creating files.
- `execute_deletion` recomputes the plan when necessary, rejects mismatched targets, consults `force` overrides, invokes `ArtifactRemovalStore::remove_artifact`, and surfaces structured lifecycle errors (`SpecmanError::Lifecycle(...)`) for blocked deletions and plan/target mismatches.
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

- The mutator accepts HTTPS URLs, workspace-relative paths, and SpecMan resource handles, aligning with dependency traversal rules while still preventing workspace escapes.
- Resource-handle validation routes through the same parser as dependency traversal, so `metadata add-dependency` / `add-reference` commands can accept `spec://`, `impl://`, and `scratch://` identifiers without bespoke normalization while still preventing workspace escapes.
- Persisted mutations invalidate cached dependency trees through the optional adapter hook; schemars output is reused instead of embedding raw JSON schemas in this document, per user instruction.
- No sample token maps are duplicated here—template-specific data remains governed by [SpecMan Core Template Orchestration](../../spec/specman-core/spec.md#concept-template-orchestration).

### Planning: Declarative Metadata Mutation APIs (Tagged Enums)

This repository currently exposes two overlapping mutation surfaces:

- A path-oriented helper (`MetadataMutator` + `MetadataMutationRequest`).
- A façade-oriented tagged-enum request (`Specman::update` + `FrontMatterUpdateRequest/Op`).

This plan refactors the tagged-enum request semantics to be _declarative_ (express desired end-state) while preserving externally observable behavior. This is a planning-only refactor note: no behavior changes are asserted by this document alone.

#### Non-Negotiable Spec Invariants

Per [Concept: Metadata Mutation](../../spec/specman-core/spec.md#concept-metadata-mutation) and the handle rules in the data model, any implementation must preserve:

- Only YAML front matter changes; the Markdown body remains byte-for-byte unchanged.
- Callers can choose persist-to-disk vs return updated full document.
- Locator normalization + supported scheme validation + workspace boundary enforcement.
- Persisted artifacts MUST NOT contain `spec://` / `impl://` / `scratch://` handles.
- List semantics:
  - list provided → replace exactly
  - list omitted → unchanged
- Scratch pad `target` is immutable; attempts to change it MUST fail with a descriptive error.

#### Declarative Semantics (Proposed)

To prevent the op-list from becoming order-dependent, treat `FrontMatterUpdateRequest.ops` as an _unordered set of declarations_:

- **Order MUST NOT matter**: applying the same declarations in any order yields the same result.
- **Conflicts are rejected**: if two declarations attempt to set incompatible end-states (or duplicate the same field in different ways), return a deterministic validation error.
- **Replace semantics for lists**: “replace dependencies/references/secondary_languages” MUST replace the persisted list exactly when present.
- **Idempotent ensures only where required by spec**: provide “ensure dependency/reference by locator” only for the idempotent add-by-locator behaviors mandated by SpecMan Core.
- **Artifact-kind gating**: ops must be validated against the artifact kind (spec vs impl vs scratch) before any parse/serialize.
- **Scratch `target` guard**: no `SetTarget`/`ClearTarget`-like ops for scratch pads; any attempt to mutate `target` fails.

Recommended conflict policy (default): **strict reject** (no implicit “last op wins”). This aligns best with deterministic execution requirements.

If a deterministic precedence policy is ever adopted, it MUST be explicitly documented and covered by tests.

#### Canonical Entry Point (Recommended)

- **Canonical for new callers:** `Specman::update(target, FrontMatterUpdateRequest)`.
  - Rationale: artifact-oriented, already kind-aware, consistent with other lifecycle façade operations.
- **Backward-compatibility layer:** keep `MetadataMutator::mutate(request)` as a thin adapter translating to the declarative tagged-enum (or delegating to the same internal engine).

#### Staged Refactor Checklist

The staged checklist + current status is tracked in the scratch pad at [../../.specman/scratchpad/metadata-api-declarative-refactor/scratch.md](../../.specman/scratchpad/metadata-api-declarative-refactor/scratch.md).

#### Open Questions

- If both “replace list” and “ensure item” appear in one request, do we hard-error, or define precedence?
- Are YAML formatting details (key ordering, quoting, trailing newlines) considered externally observable and therefore stability-critical?
- Should list ordering be preserved exactly as provided or normalized deterministically?
- Do we preserve unknown YAML keys during typed front matter mutation, or enforce a strict schema?

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
  pub resolved_path: Option<String>,
  pub resolution: Option<ResolutionProvenance>,
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
- `ResolutionProvenance` captures whether traversal used strict handle resolution, best-match docs, or best-match HTTPS; `resolved_path` carries the workspace path or URL chosen so context flows can still render actionable links.
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
