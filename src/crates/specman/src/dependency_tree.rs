use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::SpecmanError;
use crate::front_matter::{self, DependencyEntry, RawFrontMatter};
use crate::shared_function::SemVer;
use crate::workspace::{WorkspaceLocator, WorkspacePaths};

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
            .map_err(|err| SpecmanError::Dependency(format!("failed to fetch {}: {}", url, err)))?;
        let status = response.status();
        if !(200..300).contains(&status) {
            return Err(SpecmanError::Dependency(format!(
                "received {} from {}",
                status, url
            )));
        }
        response.into_string().map_err(|err| {
            SpecmanError::Dependency(format!("failed reading body from {}: {}", url, err))
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

/// Lightweight summary that includes version data for dependency planning.
#[derive(
    Clone, Debug, Serialize, Deserialize, JsonSchema, Default, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct ArtifactSummary {
    pub id: ArtifactId,
    pub version: Option<SemVer>,
    pub metadata: BTreeMap<String, String>,
}

/// Directed edge between two artifacts.
#[derive(
    Clone, Debug, Serialize, Deserialize, JsonSchema, Default, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct DependencyEdge {
    pub from: ArtifactSummary,
    pub to: ArtifactSummary,
    pub relation: DependencyRelation,
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
                .any(|edge| edge.from.id.kind == ArtifactKind::ScratchPad),
            _ => !self.downstream.is_empty(),
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
    workspace: L,
    fetcher: Arc<dyn ContentFetcher>,
}

impl<L: WorkspaceLocator> FilesystemDependencyMapper<L> {
    pub fn new(workspace: L) -> Self {
        Self {
            workspace,
            fetcher: Arc::new(HttpFetcher::default()),
        }
    }

    pub fn with_fetcher(workspace: L, fetcher: Arc<dyn ContentFetcher>) -> Self {
        Self { workspace, fetcher }
    }

    /// Builds a dependency tree by reading the artifact located at the provided filesystem path.
    pub fn dependency_tree_from_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<DependencyTree, SpecmanError> {
        let workspace = self.workspace.workspace()?;
        let locator = ArtifactLocator::from_path(path.as_ref(), &workspace, None)?;
        self.build_tree(locator, workspace)
    }

    /// Builds a dependency tree by fetching the artifact from the provided HTTPS URL.
    pub fn dependency_tree_from_url(&self, url: &str) -> Result<DependencyTree, SpecmanError> {
        let workspace = self.workspace.workspace()?;
        let locator = ArtifactLocator::from_url(url)?;
        self.build_tree(locator, workspace)
    }

    fn build_tree(
        &self,
        root_locator: ArtifactLocator,
        workspace: WorkspacePaths,
    ) -> Result<DependencyTree, SpecmanError> {
        let mut traversal = Traversal::new(workspace, self.fetcher.clone());
        let root = traversal.visit(&root_locator)?;
        let mut aggregate: Vec<_> = traversal.edges.iter().cloned().collect();
        let upstream: Vec<DependencyEdge> = aggregate
            .iter()
            .filter(|edge| matches!(edge.relation, DependencyRelation::Upstream))
            .cloned()
            .collect();
        let mut downstream: Vec<DependencyEdge> = aggregate
            .iter()
            .filter(|edge| matches!(edge.relation, DependencyRelation::Downstream))
            .cloned()
            .collect();

        if root.id.kind == ArtifactKind::ScratchPad {
            let dependents = collect_scratchpad_dependents(&root, &traversal.workspace)?;
            for dependent in dependents {
                let edge = DependencyEdge {
                    from: dependent,
                    to: root.clone(),
                    relation: DependencyRelation::Downstream,
                };
                aggregate.push(edge.clone());
                downstream.push(edge);
            }
        }

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
}

impl<L: WorkspaceLocator> DependencyMapping for FilesystemDependencyMapper<L> {
    fn dependency_tree(&self, root: &ArtifactId) -> Result<DependencyTree, SpecmanError> {
        let workspace = self.workspace.workspace()?;
        let locator = self.locator_for_artifact(root, &workspace)?;
        self.build_tree(locator, workspace)
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
}

#[derive(Clone, Debug)]
enum ArtifactLocator {
    File(PathBuf),
    Url(Url),
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
}

impl Traversal {
    fn new(workspace: WorkspacePaths, fetcher: Arc<dyn ContentFetcher>) -> Self {
        Self {
            workspace,
            edges: BTreeSet::new(),
            visited: HashMap::new(),
            stack: Vec::new(),
            fetcher,
        }
    }

    fn visit(&mut self, locator: &ArtifactLocator) -> Result<ArtifactSummary, SpecmanError> {
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
        let document = ArtifactDocument::load(locator, &self.workspace, self.fetcher.as_ref())?;
        let summary = document.summary.clone();

        for dependency in document.dependencies {
            let child = self.visit(&dependency.locator)?;
            self.record_edge(summary.clone(), child);
        }

        self.stack.pop();
        self.visited.insert(key, summary.clone());

        Ok(summary)
    }

    fn record_edge(&mut self, parent: ArtifactSummary, child: ArtifactSummary) {
        let upstream = DependencyEdge {
            from: parent.clone(),
            to: child.clone(),
            relation: DependencyRelation::Upstream,
        };
        let downstream = DependencyEdge {
            from: child,
            to: parent,
            relation: DependencyRelation::Downstream,
        };
        self.edges.insert(upstream);
        self.edges.insert(downstream);
    }
}

impl ArtifactDocument {
    fn load(
        locator: &ArtifactLocator,
        workspace: &WorkspacePaths,
        fetcher: &dyn ContentFetcher,
    ) -> Result<Self, SpecmanError> {
        let raw = locator.load(fetcher)?;
        let mut metadata = BTreeMap::new();
        metadata.insert("locator".into(), locator.describe());

        let (frontmatter, status) = front_matter::optional_front_matter(&raw);
        if let Some(status) = status {
            metadata.insert("metadata_status".into(), status);
        }

        let parsed = frontmatter.and_then(|fm| match serde_yaml::from_str::<RawFrontMatter>(fm) {
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
            let kind = infer_kind(front, locator);
            let version = parse_version(front.version.as_deref(), &mut metadata);
            let name = front.name.clone().unwrap_or_else(|| infer_name(locator));
            let deps = resolve_dependencies(front, kind, locator, workspace)?;
            (name, version, kind, deps)
        } else {
            (
                infer_name(locator),
                None,
                infer_kind(&RawFrontMatter::default(), locator),
                Vec::new(),
            )
        };

        let summary = ArtifactSummary {
            id: ArtifactId { kind, name },
            version,
            metadata,
        };

        Ok(Self {
            summary,
            dependencies,
        })
    }
}

fn resolve_dependencies(
    front: &RawFrontMatter,
    kind: ArtifactKind,
    locator: &ArtifactLocator,
    workspace: &WorkspacePaths,
) -> Result<Vec<ArtifactDependency>, SpecmanError> {
    let mut deps = Vec::new();
    match kind {
        ArtifactKind::Specification => {
            for entry in &front.dependencies {
                let reference = match entry {
                    DependencyEntry::Simple(value) => value.as_str(),
                    DependencyEntry::Detailed(obj) => obj.reference.as_str(),
                };
                let locator = resolve_dependency_locator(reference, locator, workspace)?;
                deps.push(ArtifactDependency { locator });
            }
        }
        ArtifactKind::Implementation => {
            if let Some(spec_ref) = &front.spec {
                let locator = resolve_dependency_locator(spec_ref, locator, workspace)?;
                deps.push(ArtifactDependency { locator });
            }
            for reference in &front.references {
                let locator = resolve_dependency_locator(&reference.reference, locator, workspace)?;
                deps.push(ArtifactDependency { locator });
            }
        }
        ArtifactKind::ScratchPad => {
            if let Some(target) = &front.target {
                let locator = resolve_scratch_target_locator(target, workspace)?;
                deps.push(ArtifactDependency { locator });
            }
            for entry in &front.dependencies {
                let reference = match entry {
                    DependencyEntry::Simple(value) => value.as_str(),
                    DependencyEntry::Detailed(obj) => obj.reference.as_str(),
                };
                let locator = resolve_scratch_dependency_locator(reference, workspace)?;
                deps.push(ArtifactDependency { locator });
            }
        }
    }
    Ok(deps)
}

fn infer_kind(front: &RawFrontMatter, locator: &ArtifactLocator) -> ArtifactKind {
    if front.work_type.is_some() {
        return ArtifactKind::ScratchPad;
    }
    if front.spec.is_some() {
        return ArtifactKind::Implementation;
    }
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
    let path = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else if let Some(base_dir) = base {
        base_dir.join(candidate)
    } else {
        workspace.root().join(candidate)
    };

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

    if let ArtifactLocator::Url(url) = parent {
        let joined = url.join(reference).map_err(|err| {
            SpecmanError::Dependency(format!(
                "invalid relative url {} for {}: {}",
                reference, url, err
            ))
        })?;
        return Ok(ArtifactLocator::Url(joined));
    }

    let base_dir = parent.base_dir();
    ArtifactLocator::from_path(reference, workspace, base_dir.as_deref())
}

fn resolve_scratch_target_locator(
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

    let base_dir = Some(workspace.root());
    ArtifactLocator::from_path(reference, workspace, base_dir)
}

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

    if reference.contains('/') || reference.contains('\\') {
        return ArtifactLocator::from_path(reference, workspace, Some(workspace.root()));
    }

    let slug_path = workspace
        .scratchpad_dir()
        .join(reference)
        .join("scratch.md");
    ArtifactLocator::from_path(slug_path, workspace, Some(workspace.root()))
}

fn collect_scratchpad_dependents(
    root: &ArtifactSummary,
    workspace: &WorkspacePaths,
) -> Result<Vec<ArtifactSummary>, SpecmanError> {
    if root.id.kind != ArtifactKind::ScratchPad {
        return Ok(Vec::new());
    }

    let scratchpad_dir = workspace.scratchpad_dir();
    if !scratchpad_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut dependents = Vec::new();
    let root_path = scratchpad_dir.join(&root.id.name).join("scratch.md");
    let root_canonical = fs::canonicalize(&root_path).ok();

    for entry in fs::read_dir(&scratchpad_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let scratch_file = entry.path().join("scratch.md");
        if !scratch_file.is_file() {
            continue;
        }

        let contents = fs::read_to_string(&scratch_file)?;
        let (frontmatter, _) = front_matter::optional_front_matter(&contents);
        let Some(frontmatter) = frontmatter else {
            continue;
        };

        let raw: RawFrontMatter = match serde_yaml::from_str(frontmatter) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if raw.work_type.is_none() {
            continue;
        }

        let mut metadata = BTreeMap::new();
        metadata.insert("locator".into(), scratch_file.display().to_string());
        let version = parse_version(raw.version.as_deref(), &mut metadata);
        let dependent_name = raw
            .name
            .clone()
            .or_else(|| {
                entry
                    .path()
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| scratch_file.display().to_string());

        if dependent_name == root.id.name {
            continue;
        }

        if references_scratchpad(
            &raw.dependencies,
            &root.id.name,
            root_canonical.as_deref(),
            workspace,
        )? {
            dependents.push(ArtifactSummary {
                id: ArtifactId {
                    kind: ArtifactKind::ScratchPad,
                    name: dependent_name,
                },
                version,
                metadata,
            });
        }
    }

    Ok(dependents)
}

fn references_scratchpad(
    dependencies: &[DependencyEntry],
    root_slug: &str,
    root_canonical: Option<&Path>,
    workspace: &WorkspacePaths,
) -> Result<bool, SpecmanError> {
    for entry in dependencies {
        let reference = match entry {
            DependencyEntry::Simple(value) => value.as_str(),
            DependencyEntry::Detailed(obj) => obj.reference.as_str(),
        };

        if reference == root_slug {
            return Ok(true);
        }

        let locator = match resolve_scratch_dependency_locator(reference, workspace) {
            Ok(locator) => locator,
            Err(_) => continue,
        };

        if let ArtifactLocator::File(path) = locator {
            if let Some(root_path) = root_canonical {
                if path == root_path {
                    return Ok(true);
                }
            } else if path
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|s| s.to_str())
                == Some(root_slug)
            {
                return Ok(true);
            }
        }
    }

    Ok(false)
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
            .and_then(|segments| segments.last())
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
    use crate::workspace::FilesystemWorkspaceLocator;
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
                .ok_or_else(|| SpecmanError::Dependency(format!("no stub for {}", url)))
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
        assert_eq!(tree.downstream.len(), 1);
        assert_eq!(tree.downstream[0].from.id.name, "specman-data-model");
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
        let expected_downstream: BTreeSet<_> =
            ["spec-alpha-impl"].into_iter().map(String::from).collect();
        assert_eq!(downstream_targets, expected_downstream);
        assert_eq!(tree.aggregate.len(), 4);
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
            .dependency_tree_from_url("https://example.com/root.md")
            .expect("should build tree from stubbed https content");

        assert_eq!(tree.root.id.name, "remote-root");
        assert_eq!(tree.upstream.len(), 1);
        assert_eq!(tree.upstream[0].to.id.name, "remote-child");
    }

    #[test]
    fn parse_front_matter_handles_bom_and_crlf() {
        let doc = "\u{feff}---\r\nname: alpha\r\nversion: \"1.0.0\"\r\n---\r\n# Body";
        let (front, status) = front_matter::optional_front_matter(doc);
        assert!(status.is_none(), "unexpected status: {:?}", status);
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
