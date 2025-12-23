use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::SpecmanError;

use crate::front_matter::{self, ArtifactFrontMatter, DependencyEntry, FrontMatterKind};
use crate::shared_function::SemVer;
use crate::workspace::{WorkspaceLocator, WorkspacePaths};
use std::fmt;

/// Fetches remote artifact content (e.g., HTTPS markdown documents).
pub trait ContentFetcher: Send + Sync {
    fn fetch(&self, url: &Url) -> Result<String, SpecmanError>;
}

/// Default HTTPS fetcher backed by `ureq`.
#[derive(Default)]
struct HttpFetcher;

impl ContentFetcher for HttpFetcher {
    fn fetch(&self, url: &Url) -> Result<String, SpecmanError> {
        let response = ureq::get(url.as_str())
            .call()
            .map_err(|err| SpecmanError::Dependency(format!("failed to fetch {url}: {err}")))?;
        let status = response.status();
        if !(200..300).contains(&status) {
            return Err(SpecmanError::Dependency(format!(
                "received {status} from {url}"
            )));
        }
        response.into_string().map_err(|err| {
            SpecmanError::Dependency(format!("failed reading body from {url}: {err}"))
        })
    }
}

/// Identity for any SpecMan artifact (specification, implementation, or scratch pad).
#[derive(
    Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
pub struct ArtifactId {
    pub kind: ArtifactKind,
    pub name: String,
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}://{}", self.kind, self.name)
    }
}

/// Artifact kind segmentation.
#[derive(
    Clone,
    Copy,
    Debug,
    Serialize,
    Deserialize,
    JsonSchema,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
)]
pub enum ArtifactKind {
    #[default]
    Specification,
    Implementation,
    ScratchPad,
}

/// Tracks how a locator was resolved so callers can distinguish strict paths from
/// best-effort fallbacks that preserve context without guaranteeing mutability.
#[derive(
    Clone,
    Copy,
    Debug,
    Serialize,
    Deserialize,
    JsonSchema,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
)]
pub enum ResolutionProvenance {
    #[default]
    Strict,
    BestMatchFile,
    BestMatchUrl,
}

/// Lightweight summary that includes version data for dependency planning.
#[derive(
    Clone, Debug, Serialize, Deserialize, JsonSchema, Default, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct ArtifactSummary {
    pub id: ArtifactId,
    pub version: Option<SemVer>,
    pub metadata: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<ResolutionProvenance>,
}

/// Directed edge between two artifacts.
#[derive(
    Clone, Debug, Serialize, Deserialize, JsonSchema, Default, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct DependencyEdge {
    pub from: ArtifactSummary,
    pub to: ArtifactSummary,
    pub relation: DependencyRelation,
    pub optional: bool,
}

/// Relationship classification for dependency edges.
#[derive(
    Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
pub enum DependencyRelation {
    #[default]
    Upstream,
    Downstream,
}

/// Aggregated dependency data across upstream, downstream, and combined views.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct DependencyTree {
    pub root: ArtifactSummary,
    pub upstream: Vec<DependencyEdge>,
    pub downstream: Vec<DependencyEdge>,
    pub aggregate: Vec<DependencyEdge>,
}

impl DependencyTree {
    pub fn empty(root: ArtifactSummary) -> Self {
        Self {
            root,
            upstream: Vec::new(),
            downstream: Vec::new(),
            aggregate: Vec::new(),
        }
    }

    /// Returns true when the dependency tree contains downstream artifacts that should block
    /// deletion of the root artifact.
    pub fn has_blocking_dependents(&self) -> bool {
        match self.root.id.kind {
            ArtifactKind::ScratchPad => self
                .downstream
                .iter()
                .any(|edge| !edge.optional && edge.from.id.kind == ArtifactKind::ScratchPad),
            _ => self.downstream.iter().any(|edge| !edge.optional),
        }
    }
}

/// Contract for dependency traversal services.
pub trait DependencyMapping: Send + Sync {
    fn dependency_tree(&self, root: &ArtifactId) -> Result<DependencyTree, SpecmanError>;
    fn upstream(&self, root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError>;
    fn downstream(&self, root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError>;
}

/// Filesystem-backed implementation of the `DependencyMapping` trait.
pub struct FilesystemDependencyMapper<L: WorkspaceLocator> {
    graph: Arc<DependencyGraphServices<L>>,
}

impl<L: WorkspaceLocator> FilesystemDependencyMapper<L> {
    pub fn new(workspace: L) -> Self {
        Self {
            graph: Arc::new(DependencyGraphServices::new(workspace)),
        }
    }

    pub fn with_fetcher(workspace: L, fetcher: Arc<dyn ContentFetcher>) -> Self {
        Self {
            graph: Arc::new(DependencyGraphServices::with_fetcher(workspace, fetcher)),
        }
    }

    /// Returns the shared dependency graph services so callers can opt into
    /// read-only snapshots without rebuilding traversal state.
    pub fn dependency_graph(&self) -> &DependencyGraphServices<L> {
        self.graph.as_ref()
    }

    /// Provides an `Arc` handle to the underlying graph services so other
    /// components (e.g., persistence) can observe or invalidate inventories.
    pub fn graph_handle(&self) -> Arc<DependencyGraphServices<L>> {
        self.graph.clone()
    }

    /// Exposes a trait-object handle suitable for dependency inventory invalidation.
    pub fn inventory_handle(&self) -> Arc<dyn DependencyInventory>
    where
        L: 'static,
    {
        self.graph.clone() as Arc<dyn DependencyInventory>
    }

    /// Builds a dependency tree by reading the artifact located at the provided filesystem path.
    pub fn dependency_tree_from_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<DependencyTree, SpecmanError> {
        self.graph.dependency_tree_from_path(path)
    }

    /// Builds a dependency tree by resolving any supported locator string (workspace-relative
    /// paths, HTTPS URLs, or resource handles).
    pub fn dependency_tree_from_locator(
        &self,
        reference: &str,
    ) -> Result<DependencyTree, SpecmanError> {
        self.graph.dependency_tree_from_locator(reference)
    }

    /// Tolerant variant used for context emission; falls back to best-match locators when strict
    /// handles are missing. Do not use for mutation flows.
    pub fn dependency_tree_from_locator_best_effort(
        &self,
        reference: &str,
    ) -> Result<DependencyTree, SpecmanError> {
        self.graph
            .dependency_tree_from_locator_best_effort(reference)
    }

    /// Builds a dependency tree by fetching the artifact from the provided HTTPS URL.
    /// Prefer [`dependency_tree_from_locator`], which also supports resource handles and
    /// workspace-relative paths.
    pub fn dependency_tree_from_url(&self, url: &str) -> Result<DependencyTree, SpecmanError> {
        self.graph.dependency_tree_from_locator(url)
    }
}

/// Trait representing cache or inventory layers that need invalidation whenever
/// workspace artifacts mutate.
pub trait DependencyInventory: Send + Sync {
    fn invalidate(&self);
}

/// Shared dependency graph + workspace inventory services that other modules
/// can use without depending on filesystem-specific mapper wiring.
pub struct DependencyGraphServices<L: WorkspaceLocator> {
    workspace: L,
    fetcher: Arc<dyn ContentFetcher>,
    inventory_cache: Mutex<Option<WorkspaceInventorySnapshot>>,
}

impl<L: WorkspaceLocator> DependencyGraphServices<L> {
    pub fn new(workspace: L) -> Self {
        Self {
            workspace,
            fetcher: Arc::new(HttpFetcher),
            inventory_cache: Mutex::new(None),
        }
    }

    pub fn with_fetcher(workspace: L, fetcher: Arc<dyn ContentFetcher>) -> Self {
        Self {
            workspace,
            fetcher,
            inventory_cache: Mutex::new(None),
        }
    }

    pub fn dependency_tree_from_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<DependencyTree, SpecmanError> {
        let workspace = self.workspace_paths()?;
        let locator = ArtifactLocator::from_path(path.as_ref(), &workspace, None)?;
        self.build_tree_with_workspace(
            locator,
            ResolutionProvenance::Strict,
            workspace,
            DependencyResolutionMode::Strict,
        )
    }

    pub fn dependency_tree_from_locator(
        &self,
        reference: &str,
    ) -> Result<DependencyTree, SpecmanError> {
        let workspace = self.workspace_paths()?;
        let locator = ArtifactLocator::from_reference(reference, &workspace)?;
        self.build_tree_with_workspace(
            locator,
            ResolutionProvenance::Strict,
            workspace,
            DependencyResolutionMode::Strict,
        )
    }

    /// Best-effort variant that tolerates missing strict handles by falling back to workspace
    /// docs or HTTPS URLs. Intended for context emission only; mutations should prefer the
    /// strict variants above.
    pub fn dependency_tree_from_locator_best_effort(
        &self,
        reference: &str,
    ) -> Result<DependencyTree, SpecmanError> {
        let workspace = self.workspace_paths()?;
        let (locator, resolution) = match ArtifactLocator::from_reference(reference, &workspace) {
            Ok(locator) => (locator, ResolutionProvenance::Strict),
            Err(err) => best_effort_locator(reference, &workspace).ok_or(err)?,
        };

        self.build_tree_with_workspace(
            locator,
            resolution,
            workspace,
            DependencyResolutionMode::BestEffort,
        )
    }

    pub fn dependency_tree_from_url(&self, url: &str) -> Result<DependencyTree, SpecmanError> {
        self.dependency_tree_from_locator(url)
    }

    pub fn dependency_tree_for_artifact(
        &self,
        root: &ArtifactId,
    ) -> Result<DependencyTree, SpecmanError> {
        let workspace = self.workspace_paths()?;
        let locator = self.locator_for_artifact(root, &workspace)?;
        self.build_tree_with_workspace(
            locator,
            ResolutionProvenance::Strict,
            workspace,
            DependencyResolutionMode::Strict,
        )
    }

    pub fn inventory_snapshot(&self) -> Result<WorkspaceInventorySnapshot, SpecmanError> {
        let workspace = self.workspace_paths()?;
        self.inventory_with_workspace(&workspace)
    }

    pub fn invalidate_inventory(&self) {
        self.inventory_cache.lock().unwrap().take();
    }

    fn workspace_paths(&self) -> Result<WorkspacePaths, SpecmanError> {
        self.workspace.workspace()
    }

    fn build_tree_with_workspace(
        &self,
        root_locator: ArtifactLocator,
        root_resolution: ResolutionProvenance,
        workspace: WorkspacePaths,
        mode: DependencyResolutionMode,
    ) -> Result<DependencyTree, SpecmanError> {
        let mut traversal = Traversal::new(workspace.clone(), self.fetcher.clone(), mode);
        let root = traversal.visit(&root_locator, root_resolution)?;
        let mut aggregate: BTreeSet<_> = traversal.edges.clone();

        if let Some(root_path) = root_locator.workspace_path().map(Path::to_path_buf) {
            let inventory = self.inventory_with_workspace(&workspace)?;
            for dependent in inventory.dependents_of(&root_path) {
                let edge = DependencyEdge {
                    from: dependent.summary,
                    to: root.clone(),
                    relation: DependencyRelation::Downstream,
                    optional: dependent.optional,
                };
                aggregate.insert(edge);
            }
        }

        let upstream: Vec<DependencyEdge> = aggregate
            .iter()
            .filter(|edge| matches!(edge.relation, DependencyRelation::Upstream))
            .cloned()
            .collect();
        let downstream: Vec<DependencyEdge> = aggregate
            .iter()
            .filter(|edge| matches!(edge.relation, DependencyRelation::Downstream))
            .cloned()
            .collect();
        let aggregate: Vec<DependencyEdge> = aggregate.into_iter().collect();

        Ok(DependencyTree {
            root,
            upstream,
            downstream,
            aggregate,
        })
    }

    fn locator_for_artifact(
        &self,
        root: &ArtifactId,
        workspace: &WorkspacePaths,
    ) -> Result<ArtifactLocator, SpecmanError> {
        let base = match root.kind {
            ArtifactKind::Specification => workspace.spec_dir().join(&root.name).join("spec.md"),
            ArtifactKind::Implementation => workspace.impl_dir().join(&root.name).join("impl.md"),
            ArtifactKind::ScratchPad => workspace
                .scratchpad_dir()
                .join(&root.name)
                .join("scratch.md"),
        };

        ArtifactLocator::from_path(base, workspace, None)
    }

    fn inventory_with_workspace(
        &self,
        workspace: &WorkspacePaths,
    ) -> Result<WorkspaceInventorySnapshot, SpecmanError> {
        if let Some(snapshot) = self.inventory_cache.lock().unwrap().clone() {
            return Ok(snapshot);
        }

        let built = WorkspaceInventorySnapshot::build(workspace, self.fetcher.clone())?;
        *self.inventory_cache.lock().unwrap() = Some(built.clone());
        Ok(built)
    }
}

impl<L: WorkspaceLocator> DependencyInventory for DependencyGraphServices<L> {
    fn invalidate(&self) {
        self.invalidate_inventory();
    }
}

impl<L: WorkspaceLocator> DependencyMapping for FilesystemDependencyMapper<L> {
    fn dependency_tree(&self, root: &ArtifactId) -> Result<DependencyTree, SpecmanError> {
        self.graph.dependency_tree_for_artifact(root)
    }

    fn upstream(&self, root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError> {
        let tree = self.dependency_tree(root)?;
        Ok(tree.upstream)
    }

    fn downstream(&self, root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError> {
        let tree = self.dependency_tree(root)?;
        Ok(tree.downstream)
    }
}

impl<L: WorkspaceLocator> DependencyMapping for DependencyGraphServices<L> {
    fn dependency_tree(&self, root: &ArtifactId) -> Result<DependencyTree, SpecmanError> {
        self.dependency_tree_for_artifact(root)
    }

    fn upstream(&self, root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError> {
        let tree = self.dependency_tree(root)?;
        Ok(tree.upstream)
    }

    fn downstream(&self, root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError> {
        let tree = self.dependency_tree(root)?;
        Ok(tree.downstream)
    }
}

impl<M> DependencyMapping for Arc<M>
where
    M: DependencyMapping,
{
    fn dependency_tree(&self, root: &ArtifactId) -> Result<DependencyTree, SpecmanError> {
        (**self).dependency_tree(root)
    }

    fn upstream(&self, root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError> {
        (**self).upstream(root)
    }

    fn downstream(&self, root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError> {
        (**self).downstream(root)
    }
}

#[derive(Clone, Debug)]
struct ArtifactDocument {
    summary: ArtifactSummary,
    dependencies: Vec<ArtifactDependency>,
}

#[derive(Clone, Debug)]
struct ArtifactDependency {
    locator: ArtifactLocator,
    optional: bool,
    resolution: ResolutionProvenance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DependencyResolutionMode {
    Strict,
    BestEffort,
}

impl DependencyResolutionMode {
    fn is_strict(self) -> bool {
        matches!(self, Self::Strict)
    }
}

/// Canonicalized reference to either a workspace file or remote HTTPS document. Resource handles
/// are lowered into filesystem paths before becoming `ArtifactLocator::File` variants.
#[derive(Clone, Debug)]
enum ArtifactLocator {
    File(PathBuf),
    Url(Url),
}

/// Canonical parser for `spec://`, `impl://`, and `scratch://` resource handles as defined by
/// SpecMan Core's Dependency Mapping Services concept.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ResourceHandle {
    kind: ArtifactKind,
    slug: String,
}

impl ResourceHandle {
    fn parse(reference: &str) -> Result<Option<Self>, SpecmanError> {
        if let Some(rest) = reference.strip_prefix("spec://") {
            return Self::new(ArtifactKind::Specification, rest).map(Some);
        }

        if let Some(rest) = reference.strip_prefix("impl://") {
            return Self::new(ArtifactKind::Implementation, rest).map(Some);
        }

        if let Some(rest) = reference.strip_prefix("scratch://") {
            return Self::new(ArtifactKind::ScratchPad, rest).map(Some);
        }

        if reference.contains("://")
            && !reference.starts_with("http://")
            && !reference.starts_with("https://")
        {
            let scheme = reference
                .split_once("://")
                .map(|(scheme, _)| scheme)
                .unwrap_or(reference);
            return Err(SpecmanError::Dependency(format!(
                "unsupported locator scheme {scheme}:// (expected https://, spec://, impl://, scratch://, or workspace-relative path)"
            )));
        }

        Ok(None)
    }

    fn new(kind: ArtifactKind, raw_slug: &str) -> Result<Self, SpecmanError> {
        let slug = Self::canonical_slug(raw_slug)?;
        Ok(Self { kind, slug })
    }

    fn canonical_slug(raw: &str) -> Result<String, SpecmanError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(SpecmanError::Dependency(
                "resource handle must include a non-empty identifier".into(),
            ));
        }

        if trimmed.contains('/') || trimmed.contains('\\') {
            return Err(SpecmanError::Dependency(
                "resource handle identifiers cannot contain path separators".into(),
            ));
        }

        let canonical = trimmed.to_ascii_lowercase();
        if !canonical
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
        {
            return Err(SpecmanError::Dependency(
                "resource handle identifiers may only contain letters, numbers, '-' or '_'".into(),
            ));
        }

        Ok(canonical)
    }

    fn to_path(&self, workspace: &WorkspacePaths) -> PathBuf {
        match self.kind {
            ArtifactKind::Specification => workspace.spec_dir().join(&self.slug).join("spec.md"),
            ArtifactKind::Implementation => workspace.impl_dir().join(&self.slug).join("impl.md"),
            ArtifactKind::ScratchPad => workspace
                .scratchpad_dir()
                .join(&self.slug)
                .join("scratch.md"),
        }
    }

    fn into_locator(self, workspace: &WorkspacePaths) -> Result<ArtifactLocator, SpecmanError> {
        ArtifactLocator::from_path(self.to_path(workspace), workspace, Some(workspace.root()))
    }
}

impl ArtifactLocator {
    fn from_path(
        path: impl AsRef<Path>,
        workspace: &WorkspacePaths,
        base: Option<&Path>,
    ) -> Result<Self, SpecmanError> {
        let resolved = resolve_workspace_path(path.as_ref(), base, workspace)?;
        Ok(Self::File(resolved))
    }

    /// Parses any supported dependency reference, including workspace paths, HTTPS URLs, and
    /// SpecMan resource handles (`spec://`, `impl://`, `scratch://`). Relative paths are
    /// interpreted from the workspace root.
    fn from_reference(reference: &str, workspace: &WorkspacePaths) -> Result<Self, SpecmanError> {
        if reference.starts_with("http://") {
            return Err(SpecmanError::Dependency(format!(
                "unsupported url scheme in {reference}; use https"
            )));
        }

        if reference.starts_with("https://") {
            return ArtifactLocator::from_url(reference);
        }

        if let Some(handle) = ResourceHandle::parse(reference)? {
            return handle.into_locator(workspace);
        }

        ArtifactLocator::from_path(reference, workspace, Some(workspace.root()))
    }

    fn from_url(url: &str) -> Result<Self, SpecmanError> {
        let parsed = Url::parse(url)
            .map_err(|err| SpecmanError::Dependency(format!("invalid url {url}: {err}")))?;
        if parsed.scheme() != "https" {
            return Err(SpecmanError::Dependency(format!(
                "unsupported url scheme {} (expected https)",
                parsed.scheme()
            )));
        }
        Ok(Self::Url(parsed))
    }

    fn describe(&self) -> String {
        match self {
            ArtifactLocator::File(path) => path.display().to_string(),
            ArtifactLocator::Url(url) => url.as_str().to_string(),
        }
    }

    fn key(&self) -> String {
        match self {
            ArtifactLocator::File(path) => format!("file://{}", path.display()),
            ArtifactLocator::Url(url) => url.as_str().to_string(),
        }
    }

    fn base_dir(&self) -> Option<PathBuf> {
        match self {
            ArtifactLocator::File(path) => path.parent().map(Path::to_path_buf),
            _ => None,
        }
    }

    fn workspace_path(&self) -> Option<&Path> {
        match self {
            ArtifactLocator::File(path) => Some(path.as_path()),
            ArtifactLocator::Url(_) => None,
        }
    }

    fn load(&self, fetcher: &dyn ContentFetcher) -> Result<String, SpecmanError> {
        match self {
            ArtifactLocator::File(path) => Ok(fs::read_to_string(path)?),
            ArtifactLocator::Url(url) => fetcher.fetch(url),
        }
    }
}

struct Traversal {
    workspace: WorkspacePaths,
    edges: BTreeSet<DependencyEdge>,
    visited: HashMap<String, ArtifactSummary>,
    stack: Vec<String>,
    fetcher: Arc<dyn ContentFetcher>,
    mode: DependencyResolutionMode,
}

impl Traversal {
    fn new(
        workspace: WorkspacePaths,
        fetcher: Arc<dyn ContentFetcher>,
        mode: DependencyResolutionMode,
    ) -> Self {
        Self {
            workspace,
            edges: BTreeSet::new(),
            visited: HashMap::new(),
            stack: Vec::new(),
            fetcher,
            mode,
        }
    }

    fn visit(
        &mut self,
        locator: &ArtifactLocator,
        resolution: ResolutionProvenance,
    ) -> Result<ArtifactSummary, SpecmanError> {
        let key = locator.key();
        if self.stack.contains(&key) {
            let cycle = self
                .stack
                .iter()
                .chain(std::iter::once(&key))
                .cloned()
                .collect::<Vec<_>>()
                .join(" -> ");

            let root_summary = self
                .stack
                .first()
                .and_then(|first| self.visited.get(first).cloned())
                .unwrap_or_else(|| ArtifactSummary {
                    id: ArtifactId {
                        kind: ArtifactKind::Specification,
                        name: key.clone(),
                    },
                    ..Default::default()
                });

            let partial_tree = DependencyTree {
                root: root_summary,
                upstream: self
                    .edges
                    .iter()
                    .filter(|edge| matches!(edge.relation, DependencyRelation::Upstream))
                    .cloned()
                    .collect(),
                downstream: self
                    .edges
                    .iter()
                    .filter(|edge| matches!(edge.relation, DependencyRelation::Downstream))
                    .cloned()
                    .collect(),
                aggregate: self.edges.iter().cloned().collect(),
            };

            let serialized = serde_json::to_string(&partial_tree).unwrap_or_else(|_| "{}".into());
            return Err(SpecmanError::Dependency(format!(
                "dependency cycle detected: {cycle}; partial_tree={serialized}"
            )));
        }

        if let Some(summary) = self.visited.get(&key) {
            return Ok(summary.clone());
        }

        self.stack.push(key.clone());
        let document = ArtifactDocument::load(
            locator,
            &self.workspace,
            self.fetcher.as_ref(),
            self.mode,
            resolution,
        )?;
        let summary = document.summary.clone();

        for dependency in document.dependencies {
            let child = self.visit(&dependency.locator, dependency.resolution)?;
            self.record_edge(summary.clone(), child, dependency.optional);
        }

        self.stack.pop();
        self.visited.insert(key, summary.clone());

        Ok(summary)
    }

    fn record_edge(&mut self, parent: ArtifactSummary, child: ArtifactSummary, optional: bool) {
        let upstream = DependencyEdge {
            from: parent,
            to: child,
            relation: DependencyRelation::Upstream,
            optional,
        };
        self.edges.insert(upstream);
    }
}

#[derive(Clone)]
pub struct WorkspaceInventorySnapshot {
    entries: Arc<Vec<InventoryEntry>>,
}

impl WorkspaceInventorySnapshot {
    fn build(
        workspace: &WorkspacePaths,
        fetcher: Arc<dyn ContentFetcher>,
    ) -> Result<Self, SpecmanError> {
        let mut files = gather_workspace_artifacts(workspace)?;
        files.sort();
        files.dedup();

        let mut entries = Vec::new();
        for file in files {
            let locator = ArtifactLocator::from_path(&file, workspace, None)?;
            let document = ArtifactDocument::load(
                &locator,
                workspace,
                fetcher.as_ref(),
                DependencyResolutionMode::BestEffort,
                ResolutionProvenance::Strict,
            )?;
            entries.push(InventoryEntry {
                summary: document.summary,
                dependencies: document.dependencies,
            });
        }

        Ok(Self {
            entries: Arc::new(entries),
        })
    }

    pub fn dependents_of(&self, target: &Path) -> Vec<InventoryDependent> {
        let mut dependents = Vec::new();
        for entry in self.entries.iter() {
            let mut match_optional = None;
            for dependency in &entry.dependencies {
                if let ArtifactLocator::File(path) = &dependency.locator {
                    if path == target {
                        let current = match_optional.unwrap_or(true);
                        match_optional = Some(current && dependency.optional);
                    }
                }
            }

            if let Some(optional) = match_optional {
                dependents.push(InventoryDependent {
                    summary: entry.summary.clone(),
                    optional,
                });
            }
        }

        dependents
    }
}

#[derive(Clone)]
struct InventoryEntry {
    summary: ArtifactSummary,
    dependencies: Vec<ArtifactDependency>,
}

#[derive(Clone, Debug)]
pub struct InventoryDependent {
    pub summary: ArtifactSummary,
    pub optional: bool,
}

fn gather_workspace_artifacts(workspace: &WorkspacePaths) -> Result<Vec<PathBuf>, SpecmanError> {
    let mut files = Vec::new();
    collect_named_files(&workspace.spec_dir(), "spec.md", &mut files)?;
    collect_named_files(&workspace.impl_dir(), "impl.md", &mut files)?;
    collect_named_files(&workspace.scratchpad_dir(), "scratch.md", &mut files)?;
    Ok(files)
}

fn collect_named_files(
    root: &Path,
    file_name: &str,
    out: &mut Vec<PathBuf>,
) -> Result<(), SpecmanError> {
    if !root.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            collect_named_files(&entry.path(), file_name, out)?;
        } else if ty.is_file() && entry.file_name() == file_name {
            out.push(entry.path());
        }
    }

    Ok(())
}

impl ArtifactDocument {
    fn load(
        locator: &ArtifactLocator,
        workspace: &WorkspacePaths,
        fetcher: &dyn ContentFetcher,
        mode: DependencyResolutionMode,
        resolution: ResolutionProvenance,
    ) -> Result<Self, SpecmanError> {
        let raw = locator.load(fetcher)?;
        let mut metadata = BTreeMap::new();
        metadata.insert("locator".into(), locator.describe());
        metadata.insert("resolution".into(), format!("{resolution:?}"));

        let (frontmatter, status) = front_matter::optional_front_matter(&raw);
        if let Some(status) = status {
            metadata.insert("metadata_status".into(), status);
        }

        let parsed = frontmatter.and_then(|fm| match ArtifactFrontMatter::from_yaml_str(fm) {
            Ok(value) => Some(value),
            Err(err) => {
                metadata.insert(
                    "metadata_status".into(),
                    format!("invalid-front-matter: {err}"),
                );
                None
            }
        });

        let (name, version, kind, dependencies) = if let Some(front) = parsed.as_ref() {
            let kind = artifact_kind_from_front(front);
            let version = parse_version(front.version(), &mut metadata);
            let name = front
                .name()
                .map(|value| value.to_string())
                .unwrap_or_else(|| infer_name(locator));
            let deps = resolve_dependencies(front, locator, workspace, &mut metadata, mode)?;
            (name, version, kind, deps)
        } else {
            (
                infer_name(locator),
                None,
                infer_kind_from_locator(locator),
                Vec::new(),
            )
        };

        let summary = ArtifactSummary {
            id: ArtifactId { kind, name },
            version,
            metadata,
            resolved_path: Some(locator.describe()),
            resolution: Some(resolution),
        };

        Ok(Self {
            summary,
            dependencies,
        })
    }
}

fn resolve_dependencies(
    front: &ArtifactFrontMatter,
    locator: &ArtifactLocator,
    workspace: &WorkspacePaths,
    metadata: &mut BTreeMap<String, String>,
    mode: DependencyResolutionMode,
) -> Result<Vec<ArtifactDependency>, SpecmanError> {
    let mut deps = Vec::new();
    match front {
        ArtifactFrontMatter::Specification(spec) => {
            for entry in &spec.dependencies {
                let (reference, optional) = match entry {
                    DependencyEntry::Simple(value) => (value.as_str(), false),
                    DependencyEntry::Detailed(obj) => {
                        (obj.reference.as_str(), obj.optional.unwrap_or(false))
                    }
                };
                let locator = match resolve_dependency_locator(reference, locator, workspace) {
                    Ok(locator) => Some((locator, ResolutionProvenance::Strict)),
                    Err(err) => {
                        if mode.is_strict() {
                            return Err(err);
                        }
                        if let Some(best) = best_effort_locator(reference, workspace) {
                            Some(best)
                        } else {
                            record_dependency_error(metadata, reference, &err);
                            None
                        }
                    }
                };
                let Some((locator, resolution)) = locator else {
                    continue;
                };
                deps.push(ArtifactDependency {
                    locator,
                    optional,
                    resolution,
                });
            }
        }
        ArtifactFrontMatter::Implementation(implementation) => {
            if let Some(spec_ref) = implementation.spec.as_deref() {
                let locator = match resolve_dependency_locator(spec_ref, locator, workspace) {
                    Ok(locator) => Some((locator, ResolutionProvenance::Strict)),
                    Err(err) => {
                        if mode.is_strict() {
                            return Err(err);
                        }
                        if let Some(best) = best_effort_locator(spec_ref, workspace) {
                            Some(best)
                        } else {
                            record_dependency_error(metadata, spec_ref, &err);
                            None
                        }
                    }
                };
                if let Some((locator, resolution)) = locator {
                    deps.push(ArtifactDependency {
                        locator,
                        optional: false,
                        resolution,
                    });
                }
            }
            for reference in &implementation.references {
                let locator =
                    match resolve_dependency_locator(&reference.reference, locator, workspace) {
                        Ok(locator) => Some((locator, ResolutionProvenance::Strict)),
                        Err(err) => {
                            if mode.is_strict() {
                                return Err(err);
                            }
                            if let Some(best) = best_effort_locator(&reference.reference, workspace)
                            {
                                Some(best)
                            } else {
                                record_dependency_error(metadata, &reference.reference, &err);
                                None
                            }
                        }
                    };
                let Some((locator, resolution)) = locator else {
                    continue;
                };
                deps.push(ArtifactDependency {
                    locator,
                    optional: reference.optional.unwrap_or(false),
                    resolution,
                });
            }
        }
        ArtifactFrontMatter::Scratch(scratch) => {
            if let Some(target) = scratch.target.as_deref() {
                let locator = match resolve_scratch_target_locator(target, locator, workspace) {
                    Ok(locator) => Some((locator, ResolutionProvenance::Strict)),
                    Err(err) => {
                        if mode.is_strict() {
                            return Err(err);
                        }
                        if let Some(best) = best_effort_locator(target, workspace) {
                            Some(best)
                        } else {
                            record_dependency_error(metadata, target, &err);
                            None
                        }
                    }
                };
                if let Some((locator, resolution)) = locator {
                    deps.push(ArtifactDependency {
                        locator,
                        optional: false,
                        resolution,
                    });
                }
            }
            for entry in &scratch.dependencies {
                let (reference, optional) = match entry {
                    DependencyEntry::Simple(value) => (value.as_str(), false),
                    DependencyEntry::Detailed(obj) => {
                        (obj.reference.as_str(), obj.optional.unwrap_or(false))
                    }
                };
                let locator = match resolve_scratch_dependency_locator(reference, workspace) {
                    Ok(locator) => Some((locator, ResolutionProvenance::Strict)),
                    Err(err) => {
                        if mode.is_strict() {
                            return Err(err);
                        }
                        if let Some(best) = best_effort_locator(reference, workspace) {
                            Some(best)
                        } else {
                            record_dependency_error(metadata, reference, &err);
                            None
                        }
                    }
                };
                let Some((locator, resolution)) = locator else {
                    continue;
                };
                deps.push(ArtifactDependency {
                    locator,
                    optional,
                    resolution,
                });
            }
        }
    }
    Ok(deps)
}

fn artifact_kind_from_front(front: &ArtifactFrontMatter) -> ArtifactKind {
    match front.kind() {
        FrontMatterKind::Specification => ArtifactKind::Specification,
        FrontMatterKind::Implementation => ArtifactKind::Implementation,
        FrontMatterKind::ScratchPad => ArtifactKind::ScratchPad,
    }
}

fn infer_kind_from_locator(locator: &ArtifactLocator) -> ArtifactKind {
    if let ArtifactLocator::File(path) = locator {
        if path_contains_segment(path, "impl") {
            return ArtifactKind::Implementation;
        }
        if path_contains_segment(path, ".specman") || path_contains_segment(path, "scratchpad") {
            return ArtifactKind::ScratchPad;
        }
    }
    ArtifactKind::Specification
}

fn resolve_workspace_path(
    candidate: &Path,
    base: Option<&Path>,
    workspace: &WorkspacePaths,
) -> Result<PathBuf, SpecmanError> {
    fn lexical_normalize(path: &Path) -> PathBuf {
        use std::path::Component;

        let mut normalized = PathBuf::new();
        let mut pending_parents: usize = 0;

        for component in path.components() {
            match component {
                Component::Prefix(prefix) => {
                    normalized.push(prefix.as_os_str());
                }
                Component::RootDir => {
                    normalized.push(Component::RootDir.as_os_str());
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    if normalized
                        .components()
                        .next_back()
                        .is_some_and(|c| matches!(c, Component::Normal(_)))
                    {
                        normalized.pop();
                    } else if normalized.is_absolute() {
                        // Ignore attempts to go above the root for absolute paths.
                    } else {
                        pending_parents += 1;
                    }
                }
                Component::Normal(part) => {
                    while pending_parents > 0 {
                        normalized.push("..");
                        pending_parents -= 1;
                    }
                    normalized.push(part);
                }
            }
        }

        while pending_parents > 0 {
            normalized.push("..");
            pending_parents -= 1;
        }

        if normalized.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            normalized
        }
    }

    // Enforce workspace scoping and emit explicit missing-target errors before canonicalizing.
    let raw = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else if let Some(base_dir) = base {
        base_dir.join(candidate)
    } else {
        workspace.root().join(candidate)
    };

    // Important: normalize `..`/`.` lexically before checking existence.
    // On Unix, a path like `/ws/impl/new/../../spec/x` will fail `exists()` if
    // `/ws/impl/new` doesn't exist yet, even though `/ws/spec/x` does.
    let path = lexical_normalize(&raw);

    if !path.exists() {
        return Err(SpecmanError::MissingTarget(path));
    }

    let canonical = fs::canonicalize(&path)?;
    if !canonical.starts_with(workspace.root()) {
        return Err(SpecmanError::Workspace(format!(
            "locator {} escapes workspace {}",
            canonical.display(),
            workspace.root().display()
        )));
    }
    Ok(canonical)
}

/// Verifies that a dependency reference stays within the workspace boundaries or points to a
/// supported locator (HTTPS URLs or SpecMan resource handles).
pub fn validate_workspace_reference(
    reference: &str,
    parent: &Path,
    workspace: &WorkspacePaths,
) -> Result<(), SpecmanError> {
    if reference.starts_with("http://") {
        return Err(SpecmanError::Dependency(format!(
            "unsupported url scheme in {reference}; use https"
        )));
    }

    if reference.starts_with("https://") {
        return Ok(());
    }

    if let Some(handle) = ResourceHandle::parse(reference)? {
        handle.into_locator(workspace)?;
        return Ok(());
    }

    let candidate = Path::new(reference);
    resolve_workspace_path(candidate, Some(parent), workspace)?;
    Ok(())
}

/// Normalizes a dependency/reference locator for persistence inside YAML front matter.
///
/// Persisted locators must be either:
/// - workspace-relative paths (relative to the containing artifact's directory), or
/// - fully-qualified HTTPS URLs.
///
/// Resource handles (spec://, impl://, scratch://) are accepted as *inputs* but are
/// normalized into workspace-relative paths. Unsupported schemes and workspace escapes
/// result in errors.
pub fn normalize_persisted_reference(
    reference: &str,
    parent: &Path,
    workspace: &WorkspacePaths,
) -> Result<String, SpecmanError> {
    if reference.starts_with("http://") {
        return Err(SpecmanError::Dependency(format!(
            "unsupported url scheme in {reference}; use https"
        )));
    }

    if reference.starts_with("https://") {
        return Ok(reference.to_string());
    }

    let canonical = if let Some(handle) = ResourceHandle::parse(reference)? {
        match handle.into_locator(workspace)? {
            ArtifactLocator::File(path) => path,
            ArtifactLocator::Url(url) => {
                return Ok(url.to_string());
            }
        }
    } else {
        let candidate = Path::new(reference);
        resolve_workspace_path(candidate, Some(parent), workspace)?
    };

    let canonical_parent = fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    let rel = diff_paths(&canonical, &canonical_parent).ok_or_else(|| {
        SpecmanError::Workspace(format!(
            "unable to compute workspace-relative path from {} to {}",
            canonical_parent.display(),
            canonical.display()
        ))
    })?;
    Ok(pathbuf_to_forward_slashes(&rel))
}

/// Create-time variant of `normalize_persisted_reference`.
///
/// For resource handles (`spec://`, `impl://`, `scratch://`), this lowers the handle into the
/// canonical workspace path *without requiring the target to already exist*.
///
/// This is useful for scaffolding artifacts that refer to not-yet-created targets.
pub fn normalize_persisted_reference_for_create(
    reference: &str,
    parent: &Path,
    workspace: &WorkspacePaths,
) -> Result<String, SpecmanError> {
    if reference.starts_with("http://") {
        return Err(SpecmanError::Dependency(format!(
            "unsupported url scheme in {reference}; use https"
        )));
    }

    if reference.starts_with("https://") {
        return Ok(reference.to_string());
    }

    if let Some(handle) = ResourceHandle::parse(reference)? {
        let path = handle.to_path(workspace);
        let canonical_parent = fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
        let rel = diff_paths(&path, &canonical_parent).ok_or_else(|| {
            SpecmanError::Workspace(format!(
                "unable to compute workspace-relative path from {} to {}",
                canonical_parent.display(),
                path.display()
            ))
        })?;
        return Ok(pathbuf_to_forward_slashes(&rel));
    }

    normalize_persisted_reference(reference, parent, workspace)
}

fn pathbuf_to_forward_slashes(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => parts.push("..".into()),
            std::path::Component::Normal(seg) => parts.push(seg.to_string_lossy().into_owned()),
            _ => {}
        }
    }
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

// Minimal, dependency-free equivalent of `pathdiff::diff_paths`.
fn diff_paths(path: &Path, base: &Path) -> Option<PathBuf> {
    use std::path::Component;

    let path_components: Vec<Component<'_>> = path.components().collect();
    let base_components: Vec<Component<'_>> = base.components().collect();

    // If either path has a Windows prefix, require them to match.
    let path_prefix = path_components.first();
    let base_prefix = base_components.first();
    match (path_prefix, base_prefix) {
        (Some(Component::Prefix(_)), Some(Component::Prefix(_))) => {
            if path_prefix != base_prefix {
                return None;
            }
        }
        (Some(Component::Prefix(_)), _) => return None,
        (_, Some(Component::Prefix(_))) => return None,
        _ => {}
    }

    let mut common_len = 0usize;
    let max = std::cmp::min(path_components.len(), base_components.len());
    while common_len < max && path_components[common_len] == base_components[common_len] {
        common_len += 1;
    }

    let mut result = PathBuf::new();
    for comp in base_components.iter().skip(common_len) {
        if matches!(comp, Component::Normal(_)) {
            result.push("..");
        }
    }

    for comp in path_components.iter().skip(common_len) {
        match comp {
            Component::Normal(seg) => result.push(seg),
            Component::ParentDir => result.push(".."),
            Component::CurDir => {}
            _ => {}
        }
    }

    Some(result)
}

/// Provides a tolerant locator resolution that prioritizes workspace-local Markdown files
/// or fully-qualified HTTPS URLs when strict resolution fails. Used for context emission so
/// callers still receive actionable paths even when handles are imperfect.
fn best_effort_locator(
    reference: &str,
    workspace: &WorkspacePaths,
) -> Option<(ArtifactLocator, ResolutionProvenance)> {
    // Attempt to handle resource handles by falling back to docs-based Markdown when the
    // canonical spec/impl/scratch layout is missing.
    if let Ok(Some(handle)) = ResourceHandle::parse(reference) {
        let doc_path = workspace
            .root()
            .join("docs")
            .join(format!("{}.md", handle.slug));
        if doc_path.is_file() {
            return Some((
                ArtifactLocator::File(doc_path),
                ResolutionProvenance::BestMatchFile,
            ));
        }
    }

    // If the reference already looks like HTTPS, treat it as an external spec.
    if reference.starts_with("https://") {
        if let Ok(locator) = ArtifactLocator::from_url(reference) {
            return Some((locator, ResolutionProvenance::BestMatchUrl));
        }
    }

    // Bare domains without a scheme (e.g., spec.commonmark.org) can be promoted to HTTPS.
    // IMPORTANT: Do not promote workspace paths (e.g., "docs/foo.md" or "../foo.md");
    // those should either resolve locally or be treated as unresolved.
    if !reference.contains("://")
        && reference.contains('.')
        && !reference.contains(' ')
        && !reference.contains('/')
        && !reference.contains('\\')
        && !reference.starts_with('.')
    {
        let candidate = format!("https://{reference}");
        if let Ok(locator) = ArtifactLocator::from_url(&candidate) {
            return Some((locator, ResolutionProvenance::BestMatchUrl));
        }
    }

    None
}

/// Resolves dependency references that may point to workspace-relative paths, HTTPS URLs, or
/// SpecMan resource handles.
fn resolve_dependency_locator(
    reference: &str,
    parent: &ArtifactLocator,
    workspace: &WorkspacePaths,
) -> Result<ArtifactLocator, SpecmanError> {
    if reference.starts_with("http://") {
        return Err(SpecmanError::Dependency(format!(
            "unsupported url scheme in {reference}; use https"
        )));
    }

    if reference.starts_with("https://") {
        return ArtifactLocator::from_url(reference);
    }

    if let Some(handle) = ResourceHandle::parse(reference)? {
        return handle.into_locator(workspace);
    }

    if let ArtifactLocator::Url(url) = parent {
        let joined = url.join(reference).map_err(|err| {
            SpecmanError::Dependency(format!("invalid relative url {reference} for {url}: {err}"))
        })?;
        return Ok(ArtifactLocator::Url(joined));
    }

    let base_dir = parent.base_dir();
    ArtifactLocator::from_path(reference, workspace, base_dir.as_deref())
}

/// Resolves scratch-pad target references during scratch creation, supporting workspace paths,
/// HTTPS URLs, and resource handles.
fn resolve_scratch_target_locator(
    reference: &str,
    scratch_locator: &ArtifactLocator,
    workspace: &WorkspacePaths,
) -> Result<ArtifactLocator, SpecmanError> {
    if reference.starts_with("http://") {
        return Err(SpecmanError::Dependency(format!(
            "unsupported url scheme in {reference}; use https"
        )));
    }
    if reference.starts_with("https://") {
        return ArtifactLocator::from_url(reference);
    }

    if let Some(handle) = ResourceHandle::parse(reference)? {
        return handle.into_locator(workspace);
    }

    let primary = ArtifactLocator::from_path(reference, workspace, Some(workspace.root()));
    match primary {
        Ok(locator) => Ok(locator),
        Err(SpecmanError::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
            if let Some(base_dir) = scratch_locator.base_dir() {
                ArtifactLocator::from_path(reference, workspace, Some(base_dir.as_path()))
            } else {
                Err(SpecmanError::Io(err))
            }
        }
        Err(err) => Err(err),
    }
}

/// Resolves scratch-pad dependency entries, allowing bare slugs, workspace paths, HTTPS URLs, and
/// resource handles.
fn resolve_scratch_dependency_locator(
    reference: &str,
    workspace: &WorkspacePaths,
) -> Result<ArtifactLocator, SpecmanError> {
    if reference.starts_with("http://") {
        return Err(SpecmanError::Dependency(format!(
            "unsupported url scheme in {reference}; use https"
        )));
    }
    if reference.starts_with("https://") {
        return ArtifactLocator::from_url(reference);
    }

    if let Some(handle) = ResourceHandle::parse(reference)? {
        return handle.into_locator(workspace);
    }

    if reference.contains('/') || reference.contains('\\') {
        return ArtifactLocator::from_path(reference, workspace, Some(workspace.root()));
    }

    let slug_path = workspace
        .scratchpad_dir()
        .join(reference)
        .join("scratch.md");
    ArtifactLocator::from_path(slug_path, workspace, Some(workspace.root()))
}

fn record_dependency_error(
    metadata: &mut BTreeMap<String, String>,
    reference: &str,
    err: &SpecmanError,
) {
    let entry = format!("{reference}: {err}");
    metadata
        .entry("dependency_errors".into())
        .and_modify(|existing| {
            existing.push_str(" | ");
            existing.push_str(&entry);
        })
        .or_insert(entry);
}

fn path_contains_segment(path: &Path, needle: &str) -> bool {
    path.iter().any(|component| component == OsStr::new(needle))
}

fn parse_version(raw: Option<&str>, metadata: &mut BTreeMap<String, String>) -> Option<SemVer> {
    if let Some(value) = raw {
        match SemVer::parse(value) {
            Ok(v) => Some(v),
            Err(err) => {
                metadata.insert("version_raw".into(), value.into());
                metadata.insert("version_error".into(), err.to_string());
                None
            }
        }
    } else {
        None
    }
}

fn infer_name(locator: &ArtifactLocator) -> String {
    match locator {
        ArtifactLocator::File(path) => infer_name_from_file(path),
        ArtifactLocator::Url(url) => url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .filter(|segment| !segment.is_empty())
            .map(|segment| segment.replace(",", "_"))
            .unwrap_or_else(|| url.host_str().unwrap_or("remote").to_string()),
    }
}

fn infer_name_from_file(path: &Path) -> String {
    if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
        if matches!(file_name, "spec.md" | "impl.md" | "scratch.md") {
            if let Some(dir_name) = path
                .parent()
                .and_then(|dir| dir.file_name())
                .and_then(|s| s.to_str())
            {
                return dir_name.to_string();
            }
        }
    }

    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{FilesystemWorkspaceLocator, WorkspacePaths};
    use std::fs;
    use std::sync::Arc;
    use tempfile::tempdir;

    struct StubFetcher {
        responses: HashMap<String, String>,
    }

    impl StubFetcher {
        fn new(entries: &[(&str, &str)]) -> Self {
            let mut responses = HashMap::new();
            for (url, body) in entries {
                responses.insert((*url).to_string(), (*body).to_string());
            }
            Self { responses }
        }
    }

    impl ContentFetcher for StubFetcher {
        fn fetch(&self, url: &Url) -> Result<String, SpecmanError> {
            self.responses
                .get(url.as_str())
                .cloned()
                .ok_or_else(|| SpecmanError::Dependency(format!("no stub for {url}")))
        }
    }

    #[test]
    fn dependency_tree_tracks_spec_dependencies() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/specman-core")).unwrap();
        fs::create_dir_all(root.join("spec/specman-data-model")).unwrap();

        fs::write(
            root.join("spec/specman-data-model/spec.md"),
            r#"---
name: specman-data-model
version: "1.0.0"
---
# Data Model
"#,
        )
        .unwrap();

        fs::write(
            root.join("spec/specman-core/spec.md"),
            r#"---
name: specman-core
version: "1.0.0"
dependencies:
  - ../specman-data-model/spec.md
---
# Core
"#,
        )
        .unwrap();

        let mapper =
            FilesystemDependencyMapper::new(FilesystemWorkspaceLocator::new(root.to_path_buf()));
        let tree = mapper
            .dependency_tree_from_path(root.join("spec/specman-core/spec.md"))
            .expect("build tree");

        assert_eq!(tree.root.id.name, "specman-core");
        assert_eq!(tree.upstream.len(), 1);
        assert_eq!(tree.upstream[0].to.id.name, "specman-data-model");
        assert!(tree.downstream.is_empty());
    }

    #[test]
    fn dependency_tree_from_locator_accepts_resource_handles() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/specman-core")).unwrap();

        fs::write(
            root.join("spec/specman-core/spec.md"),
            r"---
name: specman-core
---
# SpecMan Core
",
        )
        .unwrap();

        let mapper =
            FilesystemDependencyMapper::new(FilesystemWorkspaceLocator::new(root.to_path_buf()));
        let tree = mapper
            .dependency_tree_from_locator("spec://specman-core")
            .expect("handle builds tree");

        assert_eq!(tree.root.id.name, "specman-core");
    }

    #[test]
    fn dependency_tree_reports_missing_handle_targets() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();

        let mapper =
            FilesystemDependencyMapper::new(FilesystemWorkspaceLocator::new(root.to_path_buf()));

        let err = mapper
            .dependency_tree_from_locator("spec://missing-spec")
            .expect_err("missing handles should error");

        if let SpecmanError::MissingTarget(path) = err {
            assert!(path.to_string_lossy().contains("missing-spec"));
        } else {
            panic!("expected missing-target error for missing handle target");
        }
    }

    #[test]
    fn dependency_tree_best_effort_returns_docs_when_handle_missing() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();

        fs::write(
            root.join("docs/founding-spec.md"),
            r#"---
name: founding-spec
---
# Founding Spec
"#,
        )
        .unwrap();

        let mapper =
            FilesystemDependencyMapper::new(FilesystemWorkspaceLocator::new(root.to_path_buf()));
        let tree = mapper
            .dependency_tree_from_locator_best_effort("spec://founding-spec")
            .expect("best-effort docs fallback");

        assert_eq!(tree.root.id.name, "founding-spec");
        assert_eq!(
            tree.root.resolution,
            Some(ResolutionProvenance::BestMatchFile)
        );
        let resolved = tree
            .root
            .resolved_path
            .as_ref()
            .expect("best-effort should set resolved_path");
        let resolved_path = std::path::Path::new(resolved);
        assert!(
            resolved_path.ends_with(std::path::Path::new("docs").join("founding-spec.md")),
            "unexpected resolved path: {resolved}"
        );
    }

    #[test]
    fn dependency_tree_best_effort_skips_missing_local_dependency_paths() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/specman-core")).unwrap();

        fs::write(
            root.join("spec/specman-core/spec.md"),
            r#"---
name: specman-core
dependencies:
  - ../docs/missing.md
---
# SpecMan Core
"#,
        )
        .unwrap();

        let mapper =
            FilesystemDependencyMapper::new(FilesystemWorkspaceLocator::new(root.to_path_buf()));
        let tree = mapper
            .dependency_tree_from_locator_best_effort("spec://specman-core")
            .expect("best-effort should tolerate missing local deps");

        // Root should load; missing upstream dep should be omitted.
        assert_eq!(tree.root.id.name, "specman-core");
        assert!(tree.upstream.is_empty(), "missing deps should be skipped");

        // Defense-in-depth: ensure no promoted https locator leaked into the tree.
        // (If it did, traversal might attempt network I/O.)
        assert!(
            tree.aggregate
                .iter()
                .all(|edge| edge.to.resolved_path.as_deref().unwrap_or("")
                    != "https://../docs/missing.md"),
            "missing local path must not be promoted to malformed https URL"
        );
    }

    #[test]
    fn best_effort_locator_promotes_bare_domain_to_https() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        let workspace = WorkspacePaths::new(root.clone(), root.join(".specman"));

        let (locator, resolution) =
            best_effort_locator("spec.commonmark.org", &workspace).expect("domain promoted");

        assert_eq!(resolution, ResolutionProvenance::BestMatchUrl);
        match locator {
            ArtifactLocator::Url(url) => {
                assert_eq!(url.as_str(), "https://spec.commonmark.org/");
            }
            other => panic!("expected url locator, got {other:?}"),
        }
    }

    #[test]
    fn best_effort_locator_does_not_promote_workspace_like_paths_to_https() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        let workspace = WorkspacePaths::new(root.clone(), root.join(".specman"));

        assert!(
            best_effort_locator("docs/missing.md", &workspace).is_none(),
            "workspace-ish path with '/' must not be promoted to https"
        );
        assert!(
            best_effort_locator("../missing.md", &workspace).is_none(),
            "relative workspace-ish path must not be promoted to https"
        );
        assert!(
            best_effort_locator("./missing.md", &workspace).is_none(),
            "relative workspace-ish path must not be promoted to https"
        );
    }

    #[test]
    fn resource_handle_parser_normalizes_slug() {
        let handle = ResourceHandle::parse("spec://SpecMan-Core").expect("parse succeeded");
        let handle = handle.expect("handle detected");
        assert_eq!(handle.kind, ArtifactKind::Specification);
        assert_eq!(handle.slug, "specman-core");

        let scratch = ResourceHandle::parse("scratch://Pad_One").expect("parse");
        let scratch = scratch.expect("handle detected");
        assert_eq!(scratch.kind, ArtifactKind::ScratchPad);
        assert_eq!(scratch.slug, "pad_one");
    }

    #[test]
    fn resolve_dependency_locator_supports_resource_handles() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman").join("scratchpad")).unwrap();
        fs::create_dir_all(root.join("spec/specman-core")).unwrap();
        fs::create_dir_all(root.join("impl/workflow-engine")).unwrap();

        fs::write(root.join("spec/specman-core/spec.md"), r"# SpecMan Core\n").unwrap();

        fs::write(
            root.join("impl/workflow-engine/impl.md"),
            r"# Workflow Engine\n",
        )
        .unwrap();

        let root_canonical = root.canonicalize().unwrap();
        let workspace =
            WorkspacePaths::new(root_canonical.clone(), root_canonical.join(".specman"));

        let parent = ArtifactLocator::from_path(
            workspace.impl_dir().join("workflow-engine").join("impl.md"),
            &workspace,
            None,
        )
        .expect("parent locator");

        let resolved = resolve_dependency_locator("spec://specman-core", &parent, &workspace)
            .expect("handle resolves");

        match resolved {
            ArtifactLocator::File(path) => {
                assert!(path.ends_with("spec/specman-core/spec.md"));
            }
            _ => panic!("expected filesystem locator"),
        }
    }

    #[test]
    fn resolve_dependency_locator_rejects_unknown_scheme() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman").join("scratchpad")).unwrap();
        fs::create_dir_all(root.join("impl/workflow-engine")).unwrap();
        fs::write(
            root.join("impl/workflow-engine/impl.md"),
            r"# Workflow Engine\n",
        )
        .unwrap();

        let root_canonical = root.canonicalize().unwrap();
        let workspace =
            WorkspacePaths::new(root_canonical.clone(), root_canonical.join(".specman"));

        let parent = ArtifactLocator::from_path(
            workspace.impl_dir().join("workflow-engine").join("impl.md"),
            &workspace,
            None,
        )
        .expect("parent locator");

        let err = resolve_dependency_locator("ftp://example", &parent, &workspace)
            .expect_err("should reject unsupported scheme");
        if let SpecmanError::Dependency(message) = err {
            assert!(message.contains("unsupported locator scheme"));
        } else {
            panic!("expected dependency error");
        }
    }

    #[test]
    fn dependency_tree_tracks_implementation_references() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/spec-alpha")).unwrap();
        fs::create_dir_all(root.join("spec/reference")).unwrap();
        fs::create_dir_all(root.join("impl/spec-alpha")).unwrap();

        fs::write(
            root.join("spec/spec-alpha/spec.md"),
            r#"---
name: spec-alpha
version: "1.0.0"
---
# Spec Alpha
"#,
        )
        .unwrap();

        fs::write(
            root.join("spec/reference/spec.md"),
            r#"---
name: reference-doc
version: "0.1.0"
---
# Reference
"#,
        )
        .unwrap();

        fs::write(
            root.join("impl/spec-alpha/impl.md"),
            r#"---
spec: ../../spec/spec-alpha/spec.md
name: spec-alpha-impl
version: "1.0.0"
references:
  - ref: ../../spec/reference/spec.md
    type: specification
---
# Implementation Alpha
"#,
        )
        .unwrap();

        let mapper =
            FilesystemDependencyMapper::new(FilesystemWorkspaceLocator::new(root.to_path_buf()));
        let tree = mapper
            .dependency_tree_from_path(root.join("impl/spec-alpha/impl.md"))
            .expect("build impl tree");

        assert_eq!(tree.root.id.name, "spec-alpha-impl");

        let upstream: BTreeSet<_> = tree
            .upstream
            .iter()
            .map(|edge| edge.to.id.name.clone())
            .collect();
        let expected: BTreeSet<_> = ["spec-alpha", "reference-doc"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(upstream, expected);

        let downstream_targets: BTreeSet<_> = tree
            .downstream
            .iter()
            .map(|edge| edge.to.id.name.clone())
            .collect();
        assert!(downstream_targets.is_empty());
        assert_eq!(tree.aggregate.len(), 2);
    }

    #[test]
    fn dependency_tree_discovers_downstream_consumers() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/provider")).unwrap();
        fs::create_dir_all(root.join("spec/consumer")).unwrap();

        fs::write(
            root.join("spec/provider/spec.md"),
            "---\nname: provider\nversion: \"1.0.0\"\n---\n# Provider\n",
        )
        .unwrap();

        fs::write(
            root.join("spec/consumer/spec.md"),
            "---\nname: consumer\nversion: \"1.0.0\"\ndependencies:\n  - ../provider/spec.md\n---\n# Consumer\n",
        )
        .unwrap();

        let mapper =
            FilesystemDependencyMapper::new(FilesystemWorkspaceLocator::new(root.to_path_buf()));
        let tree = mapper
            .dependency_tree_from_path(root.join("spec/provider/spec.md"))
            .expect("provider tree");

        assert_eq!(tree.downstream.len(), 1);
        assert_eq!(tree.downstream[0].from.id.name, "consumer");
        assert!(!tree.downstream[0].optional);
        assert!(tree.has_blocking_dependents());
    }

    #[test]
    fn optional_dependencies_do_not_block_deletions() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/subject")).unwrap();
        fs::create_dir_all(root.join("spec/consumer")).unwrap();

        fs::write(
            root.join("spec/subject/spec.md"),
            "---\nname: subject\nversion: \"1.0.0\"\n---\n# Subject\n",
        )
        .unwrap();

        fs::write(
            root.join("spec/consumer/spec.md"),
            "---\nname: optional-consumer\nversion: \"1.0.0\"\ndependencies:\n  - ref: ../subject/spec.md\n    optional: true\n---\n# Consumer\n",
        )
        .unwrap();

        let mapper =
            FilesystemDependencyMapper::new(FilesystemWorkspaceLocator::new(root.to_path_buf()));
        let tree = mapper
            .dependency_tree_from_path(root.join("spec/subject/spec.md"))
            .expect("subject tree");

        assert_eq!(tree.downstream.len(), 1);
        assert!(tree.downstream[0].optional);
        assert!(!tree.has_blocking_dependents());
    }

    #[test]
    fn dependency_tree_detects_cycles() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/alpha")).unwrap();
        fs::create_dir_all(root.join("spec/beta")).unwrap();

        fs::write(
            root.join("spec/alpha/spec.md"),
            r#"---
name: alpha
version: "1.0.0"
dependencies:
  - ../beta/spec.md
---
# Alpha
"#,
        )
        .unwrap();

        fs::write(
            root.join("spec/beta/spec.md"),
            r#"---
name: beta
version: "1.0.0"
dependencies:
  - ../alpha/spec.md
---
# Beta
"#,
        )
        .unwrap();

        let mapper =
            FilesystemDependencyMapper::new(FilesystemWorkspaceLocator::new(root.to_path_buf()));
        let err = mapper
            .dependency_tree_from_path(root.join("spec/alpha/spec.md"))
            .expect_err("cycle expected");

        match err {
            SpecmanError::Dependency(msg) => assert!(msg.contains("cycle")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn dependency_tree_rejects_workspace_escape() {
        let temp = tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let outside = temp.path().join("outside");
        fs::create_dir_all(workspace.join(".specman")).unwrap();
        fs::create_dir_all(workspace.join("spec/origin")).unwrap();
        fs::create_dir_all(&outside).unwrap();

        let outside_spec = outside.join("spec.md");
        fs::write(
            &outside_spec,
            r#"---
name: outside
version: "0.1.0"
---
# Outside
"#,
        )
        .unwrap();

        fs::write(
            workspace.join("spec/origin/spec.md"),
            format!(
                "---\nname: origin\nversion: \"0.1.0\"\ndependencies:\n  - {}\n---\n# Origin\n",
                outside_spec.display()
            ),
        )
        .unwrap();

        let mapper = FilesystemDependencyMapper::new(FilesystemWorkspaceLocator::new(
            workspace.to_path_buf(),
        ));

        let err = mapper
            .dependency_tree_from_path(workspace.join("spec/origin/spec.md"))
            .expect_err("workspace violation");

        match err {
            SpecmanError::Workspace(msg) => assert!(msg.contains("escapes workspace")),
            other => panic!("expected workspace error, got {other:?}"),
        }
    }

    #[test]
    fn dependency_tree_fetches_https_artifacts_with_stub() {
        let temp = tempdir().unwrap();
        let workspace_root = temp.path().join("workspace");
        fs::create_dir_all(workspace_root.join(".specman")).unwrap();

        let fetcher_entries = [
            (
                "https://example.com/root.md",
                "---\nname: remote-root\nversion: \"1.0.0\"\ndependencies:\n  - https://example.com/child.md\n---\n# Root",
            ),
            (
                "https://example.com/child.md",
                "---\nname: remote-child\nversion: \"0.1.0\"\n---\n# Child",
            ),
        ];
        let fetcher: Arc<dyn ContentFetcher> = Arc::new(StubFetcher::new(&fetcher_entries));

        let mapper = FilesystemDependencyMapper::with_fetcher(
            FilesystemWorkspaceLocator::new(&workspace_root),
            fetcher,
        );

        let tree = mapper
            .dependency_tree_from_locator("https://example.com/root.md")
            .expect("should build tree from stubbed https content");

        assert_eq!(tree.root.id.name, "remote-root");
        assert_eq!(tree.upstream.len(), 1);
        assert_eq!(tree.upstream[0].to.id.name, "remote-child");
    }

    #[test]
    fn parse_front_matter_handles_bom_and_crlf() {
        let doc = "\u{feff}---\r\nname: alpha\r\nversion: \"1.0.0\"\r\n---\r\n# Body";
        let (front, status) = front_matter::optional_front_matter(doc);
        assert!(status.is_none(), "unexpected status: {status:?}");
        let normalized = front.unwrap().replace('\r', "");
        assert_eq!(normalized, "name: alpha\nversion: \"1.0.0\"");
    }

    #[test]
    fn parse_front_matter_reports_missing_when_absent() {
        let doc = "# Heading only\ncontent";
        let (front, status) = front_matter::optional_front_matter(doc);
        assert!(front.is_none());
        assert_eq!(status.as_deref(), Some("missing"));
    }

    #[test]
    fn artifact_locator_rejects_non_https_urls() {
        let err = ArtifactLocator::from_url("http://example.com/spec.md")
            .expect_err("expected rejection");
        match err {
            SpecmanError::Dependency(msg) => assert!(msg.contains("unsupported url scheme")),
            other => panic!("unexpected error type: {other:?}"),
        }
    }

    #[test]
    fn validate_workspace_reference_accepts_resource_handles() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/specman-core")).unwrap();
        fs::create_dir_all(root.join("spec/consumer")).unwrap();

        fs::write(
            root.join("spec/specman-core/spec.md"),
            "---\nname: specman-core\n---\n",
        )
        .unwrap();

        let root_canonical = root.canonicalize().unwrap();
        let workspace =
            WorkspacePaths::new(root_canonical.clone(), root_canonical.join(".specman"));
        let parent_dir = workspace.spec_dir().join("consumer");
        fs::create_dir_all(&parent_dir).unwrap();

        validate_workspace_reference("spec://specman-core", &parent_dir, &workspace)
            .expect("handle should validate");
    }

    #[test]
    fn validate_workspace_reference_rejects_missing_handle_targets() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/consumer")).unwrap();

        let root_canonical = root.canonicalize().unwrap();
        let workspace =
            WorkspacePaths::new(root_canonical.clone(), root_canonical.join(".specman"));
        let parent_dir = workspace.spec_dir().join("consumer");
        fs::create_dir_all(&parent_dir).unwrap();

        let err = validate_workspace_reference("spec://unknown", &parent_dir, &workspace)
            .expect_err("missing handles should error");
        if let SpecmanError::MissingTarget(path) = err {
            assert!(path.to_string_lossy().contains("unknown"));
        } else {
            panic!("expected missing-target error for missing target");
        }
    }

    #[test]
    fn scratch_dependency_resolves_relative_to_workspace_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join(".specman/scratchpad/deletion-lifecycle-apis")).unwrap();
        fs::create_dir_all(root.join("impl/specman-library")).unwrap();
        fs::create_dir_all(root.join("spec/specman-core")).unwrap();

        fs::write(
            root.join("spec/specman-core/spec.md"),
            "---\nname: specman-core\nversion: \"1.0.0\"\n---\n",
        )
        .unwrap();

        fs::write(
            root.join("impl/specman-library/impl.md"),
            r#"---
spec: ../../spec/specman-core/spec.md
name: specman-library
version: "0.1.0"
references: []
---
# Impl
"#,
        )
        .unwrap();

        fs::write(
            root.join(".specman/scratchpad/deletion-lifecycle-apis/scratch.md"),
            r#"---
name: deletion-lifecycle-apis
target: impl/specman-library/impl.md
work_type:
  feat: {}
---
# Scratch
"#,
        )
        .unwrap();

        let locator = FilesystemWorkspaceLocator::new(root.join("impl"));
        let mapper = FilesystemDependencyMapper::new(locator);
        let scratch_file = root.join(".specman/scratchpad/deletion-lifecycle-apis/scratch.md");
        let tree = mapper
            .dependency_tree_from_path(&scratch_file)
            .expect("scratch dependency tree");

        assert_eq!(tree.root.id.name, "deletion-lifecycle-apis");
        let upstream: BTreeSet<_> = tree
            .upstream
            .iter()
            .map(|edge| edge.to.id.name.clone())
            .collect();
        assert!(upstream.contains("specman-library"));
    }

    #[test]
    fn scratch_dependencies_resolve_named_and_path_variants() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join(".specman/scratchpad/base")).unwrap();
        fs::create_dir_all(root.join(".specman/scratchpad/slug-upstream")).unwrap();
        fs::create_dir_all(root.join(".specman/scratchpad/path-upstream")).unwrap();
        fs::create_dir_all(root.join("impl/specman-library")).unwrap();
        fs::create_dir_all(root.join("spec/specman-core")).unwrap();

        fs::write(
            root.join("spec/specman-core/spec.md"),
            r#"---
name: specman-core
version: "1.0.0"
---
"#,
        )
        .unwrap();

        fs::write(
            root.join("impl/specman-library/impl.md"),
            r#"---
spec: ../../spec/specman-core/spec.md
name: specman-library
version: "0.1.0"
references: []
---
# Impl
"#,
        )
        .unwrap();

        fs::write(
            root.join(".specman/scratchpad/slug-upstream/scratch.md"),
            r#"---
name: slug-upstream
work_type:
    ref: {}
---
# Scratch
"#,
        )
        .unwrap();

        fs::write(
            root.join(".specman/scratchpad/path-upstream/scratch.md"),
            r#"---
name: path-upstream
work_type:
    ref: {}
---
# Scratch
"#,
        )
        .unwrap();

        fs::write(
            root.join(".specman/scratchpad/base/scratch.md"),
            r#"---
name: base
target: impl/specman-library/impl.md
dependencies:
    - slug-upstream
    - .specman/scratchpad/path-upstream/scratch.md
work_type:
    ref: {}
---
# Scratch
"#,
        )
        .unwrap();

        let locator = FilesystemWorkspaceLocator::new(root.join("impl"));
        let mapper = FilesystemDependencyMapper::new(locator);
        let scratch_file = root.join(".specman/scratchpad/base/scratch.md");
        let tree = mapper
            .dependency_tree_from_path(&scratch_file)
            .expect("scratch dependency tree");

        let upstream: BTreeSet<_> = tree
            .upstream
            .iter()
            .map(|edge| edge.to.id.name.clone())
            .collect();
        assert!(upstream.contains("slug-upstream"));
        assert!(upstream.contains("path-upstream"));
    }

    #[test]
    fn downstream_scratchpads_are_discovered() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join(".specman/scratchpad/target")).unwrap();
        fs::create_dir_all(root.join(".specman/scratchpad/slug-dependent")).unwrap();
        fs::create_dir_all(root.join(".specman/scratchpad/path-dependent")).unwrap();

        fs::write(
            root.join(".specman/scratchpad/target/scratch.md"),
            r#"---
name: target
work_type:
    ref: {}
---
# Target Scratch
"#,
        )
        .unwrap();

        fs::write(
            root.join(".specman/scratchpad/slug-dependent/scratch.md"),
            r#"---
name: slug-dependent
dependencies:
    - target
work_type:
    ref: {}
---
# Dependent Scratch
"#,
        )
        .unwrap();

        fs::write(
            root.join(".specman/scratchpad/path-dependent/scratch.md"),
            r#"---
name: path-dependent
dependencies:
    - .specman/scratchpad/target/scratch.md
work_type:
    ref: {}
---
# Dependent Scratch
"#,
        )
        .unwrap();

        let locator = FilesystemWorkspaceLocator::new(root.join(".specman"));
        let mapper = FilesystemDependencyMapper::new(locator);
        let scratch_file = root.join(".specman/scratchpad/target/scratch.md");
        let tree = mapper
            .dependency_tree_from_path(&scratch_file)
            .expect("scratch dependency tree");

        let downstream: BTreeSet<_> = tree
            .downstream
            .iter()
            .map(|edge| edge.from.id.name.clone())
            .collect();
        assert!(downstream.contains("slug-dependent"));
        assert!(downstream.contains("path-dependent"));
        assert!(tree.has_blocking_dependents());
    }
}
