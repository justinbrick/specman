//! SpecMan data model helpers for workspace, scratch pad, and dependency tooling.

#![allow(dead_code)]

use schemars::JsonSchema;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpecmanWorkspace {
    /// Absolute path to the workspace root directory. MUST contain a `.specman` child directory.
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

#[derive(
    Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct ArtifactLocator {
    /// Relative filesystem path or absolute URL pointing at the artifact.
    pub value: String,
}

impl ArtifactLocator {
    pub fn new<S: Into<String>>(value: S) -> Self {
        Self {
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "locator", rename_all = "snake_case")]
pub enum TargetArtifact {
    Specification(ArtifactLocator),
    Implementation(ArtifactLocator),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkType {
    Draft,
    Revision,
    Feat,
    Ref,
}

impl WorkType {
    fn as_str(&self) -> &'static str {
        match self {
            WorkType::Draft => "draft",
            WorkType::Revision => "revision",
            WorkType::Feat => "feat",
            WorkType::Ref => "ref",
        }
    }
}

impl FromStr for WorkType {
    type Err = ScratchpadError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(WorkType::Draft),
            "revision" => Ok(WorkType::Revision),
            "feat" => Ok(WorkType::Feat),
            "ref" => Ok(WorkType::Ref),
            other => Err(ScratchpadError::InvalidMetadata(format!(
                "unsupported work type `{other}`"
            ))),
        }
    }
}

fn empty_json_object() -> JsonValue {
    JsonValue::Object(JsonMap::new())
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
    #[serde(default = "empty_json_object")]
    pub front_matter: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImplementingLanguage {
    pub language: String,
    #[serde(default = "empty_json_object")]
    pub properties: JsonValue,
    #[serde(default)]
    pub libraries: Vec<LibraryReference>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LibraryReference {
    pub name: String,
    #[serde(default = "empty_json_object")]
    pub metadata: JsonValue,
}

impl<'de> Deserialize<'de> for LibraryReference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct LibraryVisitor;

        impl<'de> Visitor<'de> for LibraryVisitor {
            type Value = LibraryReference;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a library reference string or mapping")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(LibraryReference {
                    name: value.to_owned(),
                    metadata: empty_json_object(),
                })
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut name: Option<String> = None;
                let mut metadata = JsonMap::new();

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "name" => {
                            if name.is_some() {
                                return Err(de::Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        }
                        other => {
                            let value: JsonValue = map.next_value()?;
                            metadata.insert(other.to_owned(), value);
                        }
                    }
                }

                let name = name.ok_or_else(|| de::Error::missing_field("name"))?;
                Ok(LibraryReference {
                    name,
                    metadata: JsonValue::Object(metadata),
                })
            }
        }

        deserializer.deserialize_any(LibraryVisitor)
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImplementationMetadata {
    pub spec: String,
    pub name: Option<String>,
    pub version: String,
    pub location: Option<String>,
    #[serde(default)]
    pub library: Option<LibraryReference>,
    pub primary_language: ImplementingLanguage,
    #[serde(default)]
    pub secondary_languages: Vec<ImplementingLanguage>,
    #[serde(default)]
    pub references: Vec<ImplementationReference>,
}

#[derive(Debug, Clone, Default)]
pub struct ResolveWorkspaceConfig {
    pub cwd: Option<PathBuf>,
    pub stop_at: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ScratchpadPlan {
    pub workspace: SpecmanWorkspace,
    pub scratchpad_name: String,
    pub metadata: ScratchpadMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScratchpadPlanResult {
    pub root: PathBuf,
    pub scratchpad: ScratchpadDirectory,
    pub directories: Vec<PathBuf>,
    pub files: BTreeSet<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct DependencyGraphConfig {
    pub max_depth: Option<usize>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyGraphMode {
    Downstream,
    Upstream,
    Full,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyArtifactKind {
    Specification,
    Implementation,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpecificationDependencyNode {
    pub locator: ArtifactLocator,
    pub kind: DependencyArtifactKind,
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImplementationDependencyNode {
    pub locator: ArtifactLocator,
    pub kind: DependencyArtifactKind,
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DependencyEdge {
    pub from: ArtifactLocator,
    pub to: ArtifactLocator,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpecificationDependencyGraph {
    pub root: SpecificationDependencyNode,
    pub nodes: Vec<SpecificationDependencyNode>,
    pub edges: Vec<DependencyEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImplementationDependencyGraph {
    pub root: ImplementationDependencyNode,
    pub nodes: Vec<ImplementationDependencyNode>,
    pub edges: Vec<DependencyEdge>,
}

/// Resolves the active SpecMan workspace by locating the nearest ancestor containing `.specman`.
pub async fn resolve_workspace(
    config: ResolveWorkspaceConfig,
) -> Result<SpecmanWorkspace, WorkspaceError> {
    let start = match config.cwd.clone() {
        Some(path) => path,
        None => std::env::current_dir().map_err(|err| WorkspaceError::Io(err.to_string()))?,
    };

    let mut current = start.clone();

    loop {
        let dot_folder = current.join(".specman");
        if dot_folder.is_dir() {
            return Ok(SpecmanWorkspace { root: current });
        }

        if let Some(ref stop_at) = config.stop_at {
            if paths_equal(&current, stop_at) {
                break;
            }
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    Err(WorkspaceError::NotFound(start))
}

/// Parses scratch pad metadata from YAML front matter and enforces normative constraints.
pub fn parse_scratchpad_metadata(input: &str) -> Result<ScratchpadMetadata, ScratchpadError> {
    #[derive(Deserialize)]
    struct RawScratchpadMetadata {
        target: String,
        work_type: String,
        branch: Option<String>,
        #[serde(flatten)]
        extras: BTreeMap<String, serde_yaml::Value>,
    }

    let raw: RawScratchpadMetadata = serde_yaml::from_str(input)
        .map_err(|err| ScratchpadError::InvalidMetadata(format!("{err}")))?;

    let work_type = WorkType::from_str(raw.work_type.as_str())?;
    let target = validate_target_for_work_type(work_type, raw.target.as_str())?;

    if let Some(branch) = raw.branch.as_ref() {
        validate_branch_format(branch, work_type)?;
    }

    let front_matter = extras_to_json(raw.extras)?;

    Ok(ScratchpadMetadata {
        target,
        work_type,
        branch: raw.branch,
        front_matter,
    })
}

/// Produces the canonical folder structure for a new scratch pad.
pub fn plan_scratchpad_layout(
    plan: &ScratchpadPlan,
) -> Result<ScratchpadPlanResult, ScratchpadError> {
    validate_scratchpad_name(&plan.scratchpad_name)?;

    let dot_folder_path = plan.workspace.root.join(".specman");
    if !dot_folder_path.is_dir() {
        return Err(ScratchpadError::ConstraintViolation(format!(
            "workspace `{}` does not contain required `.specman` directory",
            plan.workspace.root.display()
        )));
    }

    let scratchpad_root = dot_folder_path.join("scratchpad");
    let scratchpad_path = scratchpad_root.join(&plan.scratchpad_name);

    if let Some(branch) = plan.metadata.branch.as_ref() {
        let target_slug = derive_target_slug(&plan.metadata.target).ok_or_else(|| {
            ScratchpadError::InvalidTarget("unable to derive target name from locator".into())
        })?;
        validate_branch_format(branch, plan.metadata.work_type)?;
        validate_branch_against_name(branch, &target_slug, &plan.scratchpad_name)?;
    }

    let scratchpad_dir = ScratchpadDirectory {
        name: plan.scratchpad_name.clone(),
        path: scratchpad_path.clone(),
    };
    let root_path = scratchpad_dir.path.clone();

    let mut directories = vec![scratchpad_root.clone(), scratchpad_path.clone()];
    directories.sort();
    directories.dedup();

    let mut files = BTreeSet::new();
    files.insert(scratchpad_path.join("scratch.md"));

    Ok(ScratchpadPlanResult {
        root: root_path,
        scratchpad: scratchpad_dir,
        directories,
        files,
    })
}

/// Builds a dependency graph for specification artifacts using declared dependencies.
pub fn build_specification_dependency_graph(
    root: &Path,
    mode: DependencyGraphMode,
    config: DependencyGraphConfig,
) -> Result<SpecificationDependencyGraph, DependencyGraphError> {
    if !root.exists() {
        return Err(DependencyGraphError::UnknownArtifact(locator_from_path(
            root,
        )));
    }

    if matches!(mode, DependencyGraphMode::Upstream) {
        return Err(DependencyGraphError::UnsupportedMode(
            "upstream traversal for specifications is not implemented".into(),
        ));
    }

    let root_locator = locator_from_path(root);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    let root_node = SpecificationDependencyNode {
        locator: root_locator.clone(),
        kind: DependencyArtifactKind::Specification,
        depth: 0,
    };
    nodes.push(root_node.clone());

    let max_depth = config.max_depth.unwrap_or(usize::MAX);
    if max_depth == 0 {
        return Ok(SpecificationDependencyGraph {
            root: root_node,
            nodes,
            edges,
        });
    }

    let canonical_root = canonicalize_or_fallback(root)?;
    let mut queue: VecDeque<(PathBuf, ArtifactLocator, usize)> = VecDeque::new();
    queue.push_back((canonical_root.clone(), root_locator.clone(), 0));

    let mut visited_paths: HashSet<PathBuf> = HashSet::new();
    visited_paths.insert(canonical_root);
    let mut known_nodes: HashSet<String> = HashSet::new();
    known_nodes.insert(root_locator.value.clone());

    while let Some((current_path, from_locator, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let dependencies = parse_spec_dependencies(&current_path)?;
        for entry in dependencies {
            let locator = entry.locator.clone();
            let locator_key = locator.value.clone();
            let node_depth = depth + 1;

            if known_nodes.insert(locator_key.clone()) {
                nodes.push(SpecificationDependencyNode {
                    locator: locator.clone(),
                    kind: DependencyArtifactKind::Specification,
                    depth: node_depth,
                });
            }

            edges.push(DependencyEdge {
                from: from_locator.clone(),
                to: locator.clone(),
            });

            if node_depth < max_depth {
                if let Some(next_path) = entry.resolved_path {
                    if let Ok(canon_next) = canonicalize_or_fallback(next_path.as_path()) {
                        if visited_paths.insert(canon_next.clone()) {
                            queue.push_back((canon_next, locator, node_depth));
                        }
                    }
                }
            }
        }
    }

    Ok(SpecificationDependencyGraph {
        root: root_node,
        nodes,
        edges,
    })
}

/// Builds a dependency graph for implementation artifacts using declared references.
pub fn build_implementation_dependency_graph(
    root: &Path,
    mode: DependencyGraphMode,
    config: DependencyGraphConfig,
) -> Result<ImplementationDependencyGraph, DependencyGraphError> {
    if !root.exists() {
        return Err(DependencyGraphError::UnknownArtifact(locator_from_path(
            root,
        )));
    }

    if matches!(mode, DependencyGraphMode::Upstream) {
        return Err(DependencyGraphError::UnsupportedMode(
            "upstream traversal for implementations is not implemented".into(),
        ));
    }

    let root_locator = locator_from_path(root);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    let root_node = ImplementationDependencyNode {
        locator: root_locator.clone(),
        kind: DependencyArtifactKind::Implementation,
        depth: 0,
    };
    nodes.push(root_node.clone());

    if config.max_depth == Some(0) {
        return Ok(ImplementationDependencyGraph {
            root: root_node,
            nodes,
            edges,
        });
    }

    let metadata = parse_implementation_front_matter(root)?;
    let mut known_nodes: HashSet<String> = HashSet::new();
    known_nodes.insert(root_locator.value.clone());

    let spec_locator = ArtifactLocator::new(metadata.spec.clone());
    if known_nodes.insert(spec_locator.value.clone()) {
        nodes.push(ImplementationDependencyNode {
            locator: spec_locator.clone(),
            kind: DependencyArtifactKind::Specification,
            depth: 1,
        });
    }
    edges.push(DependencyEdge {
        from: root_locator.clone(),
        to: spec_locator,
    });

    for reference in metadata.references.iter() {
        let kind = match reference.kind.as_str() {
            "implementation" => DependencyArtifactKind::Implementation,
            "specification" => DependencyArtifactKind::Specification,
            other => {
                return Err(DependencyGraphError::InvalidFrontMatter(format!(
                    "unsupported reference type `{other}`"
                )));
            }
        };

        let locator = ArtifactLocator::new(reference.locator.clone());
        if known_nodes.insert(locator.value.clone()) {
            nodes.push(ImplementationDependencyNode {
                locator: locator.clone(),
                kind,
                depth: 1,
            });
        }

        edges.push(DependencyEdge {
            from: root_locator.clone(),
            to: locator,
        });
    }

    Ok(ImplementationDependencyGraph {
        root: root_node,
        nodes,
        edges,
    })
}

#[derive(Debug, Clone)]
pub enum ScratchpadError {
    InvalidName(String),
    InvalidTarget(String),
    InvalidBranch(String),
    InvalidMetadata(String),
    ConstraintViolation(String),
}

#[derive(Debug, Clone)]
pub enum WorkspaceError {
    NotFound(PathBuf),
    Io(String),
}

#[derive(Debug, Clone)]
pub enum DependencyGraphError {
    CycleDetected { path: Vec<ArtifactLocator> },
    DepthExceeded,
    UnknownArtifact(ArtifactLocator),
    UnsupportedMode(String),
    InvalidFrontMatter(String),
    Io(String),
}

struct DependencyEntry {
    locator: ArtifactLocator,
    resolved_path: Option<PathBuf>,
}

fn paths_equal(lhs: &Path, rhs: &Path) -> bool {
    if let (Ok(left), Ok(right)) = (fs::canonicalize(lhs), fs::canonicalize(rhs)) {
        left == right
    } else {
        lhs == rhs
    }
}

fn validate_target_for_work_type(
    work_type: WorkType,
    raw: &str,
) -> Result<TargetArtifact, ScratchpadError> {
    let locator = ArtifactLocator::new(raw.to_owned());

    if is_url(raw) {
        if matches!(work_type, WorkType::Draft | WorkType::Revision) {
            return Err(ScratchpadError::InvalidTarget(
                "draft/revision scratch pads must target a workspace-relative specification".into(),
            ));
        }
        return Ok(TargetArtifact::Implementation(locator));
    }

    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(ScratchpadError::InvalidTarget(
            "target locators MUST be relative paths".into(),
        ));
    }
    if path.components().next().is_none() {
        return Err(ScratchpadError::InvalidTarget(
            "target locator MUST reference a specification or implementation path".into(),
        ));
    }

    match work_type {
        WorkType::Draft | WorkType::Revision => {
            if path.components().next().unwrap().as_os_str() != "spec" {
                return Err(ScratchpadError::InvalidTarget(
                    "draft/revision work MUST target a specification under `spec/`".into(),
                ));
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                return Err(ScratchpadError::InvalidTarget(
                    "specification targets MUST reference Markdown documents".into(),
                ));
            }
            Ok(TargetArtifact::Specification(locator))
        }
        WorkType::Feat | WorkType::Ref => {
            if path.components().next().unwrap().as_os_str() != "impl" {
                return Err(ScratchpadError::InvalidTarget(
                    "feat/ref work MUST target an implementation under `impl/`".into(),
                ));
            }
            Ok(TargetArtifact::Implementation(locator))
        }
    }
}

fn validate_branch_format(branch: &str, work_type: WorkType) -> Result<(), ScratchpadError> {
    let parts: Vec<&str> = branch.split('/').collect();
    if parts.len() != 3 {
        return Err(ScratchpadError::InvalidBranch(
            "branch MUST follow `{target_name}/{work_type}/{scratch_pad_name}`".into(),
        ));
    }
    if parts[1] != work_type.as_str() {
        return Err(ScratchpadError::InvalidBranch(format!(
            "branch middle segment MUST be `{}`",
            work_type.as_str()
        )));
    }
    Ok(())
}

fn validate_branch_against_name(
    branch: &str,
    target_slug: &str,
    scratchpad_name: &str,
) -> Result<(), ScratchpadError> {
    let parts: Vec<&str> = branch.split('/').collect();
    if parts.len() != 3 {
        return Err(ScratchpadError::InvalidBranch(
            "branch MUST follow `{target_name}/{work_type}/{scratch_pad_name}`".into(),
        ));
    }
    if parts[0] != target_slug {
        return Err(ScratchpadError::InvalidBranch(format!(
            "branch target segment MUST be `{target_slug}`"
        )));
    }
    if parts[2] != scratchpad_name {
        return Err(ScratchpadError::InvalidBranch(format!(
            "branch scratch pad segment MUST be `{scratchpad_name}`"
        )));
    }
    Ok(())
}

fn extras_to_json(
    extras: BTreeMap<String, serde_yaml::Value>,
) -> Result<JsonValue, ScratchpadError> {
    if extras.is_empty() {
        return Ok(empty_json_object());
    }

    let mut object = JsonMap::new();
    for (key, value) in extras {
        let json_value = serde_json::to_value(value).map_err(|err| {
            ScratchpadError::InvalidMetadata(format!(
                "failed to serialise extra field `{key}`: {err}"
            ))
        })?;
        object.insert(key, json_value);
    }

    Ok(JsonValue::Object(object))
}

fn validate_scratchpad_name(name: &str) -> Result<(), ScratchpadError> {
    if name.trim().is_empty() {
        return Err(ScratchpadError::InvalidName(
            "scratch pad name MUST NOT be empty".into(),
        ));
    }

    let words: Vec<&str> = name.split('-').filter(|w| !w.is_empty()).collect();
    if words.len() > 4 {
        return Err(ScratchpadError::InvalidName(
            "scratch pad name MUST contain four hyphen-separated words or fewer".into(),
        ));
    }

    if words
        .iter()
        .any(|word| !word.chars().all(|c| c.is_ascii_lowercase()))
    {
        return Err(ScratchpadError::InvalidName(
            "scratch pad name MUST be lowercase and hyphen separated".into(),
        ));
    }

    Ok(())
}

fn derive_target_slug(target: &TargetArtifact) -> Option<String> {
    let locator = match target {
        TargetArtifact::Specification(locator) | TargetArtifact::Implementation(locator) => locator,
    };

    if is_url(locator.value.as_str()) {
        return Url::parse(locator.value.as_str())
            .ok()
            .and_then(|url| url.path_segments()?.filter(|seg| !seg.is_empty()).last())
            .map(|segment| segment.trim_end_matches(".md").to_owned());
    }

    let path = Path::new(locator.value.as_str());
    if let Some(parent) = path.parent() {
        if let Some(name) = parent.file_name() {
            return Some(name.to_string_lossy().into_owned());
        }
    }
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
}

fn is_url(value: &str) -> bool {
    Url::parse(value).is_ok()
}

fn canonicalize_or_fallback(path: &Path) -> Result<PathBuf, DependencyGraphError> {
    fs::canonicalize(path)
        .or_else(|_| Ok(path.to_path_buf()))
        .map_err(|err: std::io::Error| DependencyGraphError::Io(err.to_string()))
}

fn parse_spec_dependencies(path: &Path) -> Result<Vec<DependencyEntry>, DependencyGraphError> {
    let Some(front_matter) = read_front_matter(path)? else {
        return Ok(Vec::new());
    };

    #[derive(Deserialize)]
    struct SpecFrontMatter {
        #[serde(default)]
        dependencies: Vec<serde_yaml::Value>,
    }

    let raw: SpecFrontMatter = serde_yaml::from_str(&front_matter).map_err(|err| {
        DependencyGraphError::InvalidFrontMatter(format!(
            "failed to parse specification front matter: {err}"
        ))
    })?;

    let base_dir = path.parent().unwrap_or_else(|| Path::new(""));

    let mut result = Vec::new();
    for dependency in raw.dependencies {
        let entry = dependency_entry_from_value(base_dir, dependency)?;
        result.push(entry);
    }

    Ok(result)
}

fn dependency_entry_from_value(
    base_dir: &Path,
    value: serde_yaml::Value,
) -> Result<DependencyEntry, DependencyGraphError> {
    match value {
        serde_yaml::Value::String(raw) => Ok(create_dependency_entry(base_dir, raw)),
        serde_yaml::Value::Mapping(map) => {
            if let Some(node) = map.get(&serde_yaml::Value::from("ref")) {
                if let Some(raw) = node.as_str() {
                    return Ok(create_dependency_entry(base_dir, raw.to_owned()));
                }
            }
            Err(DependencyGraphError::InvalidFrontMatter(
                "dependency objects MUST contain a `ref` field".into(),
            ))
        }
        _ => Err(DependencyGraphError::InvalidFrontMatter(
            "dependencies MUST be strings or objects".into(),
        )),
    }
}

fn create_dependency_entry(base_dir: &Path, raw: String) -> DependencyEntry {
    if is_url(raw.as_str()) {
        DependencyEntry {
            locator: ArtifactLocator::new(raw),
            resolved_path: None,
        }
    } else {
        let resolved = base_dir.join(raw.as_str());
        DependencyEntry {
            locator: ArtifactLocator::new(raw),
            resolved_path: Some(resolved),
        }
    }
}

fn parse_implementation_front_matter(
    path: &Path,
) -> Result<ImplementationMetadata, DependencyGraphError> {
    let Some(front_matter) = read_front_matter(path)? else {
        return Err(DependencyGraphError::InvalidFrontMatter(
            "implementation documents MUST include front matter".into(),
        ));
    };

    serde_yaml::from_str(&front_matter).map_err(|err| {
        DependencyGraphError::InvalidFrontMatter(format!(
            "failed to parse implementation front matter: {err}"
        ))
    })
}

fn locator_from_path(path: &Path) -> ArtifactLocator {
    ArtifactLocator::new(path.to_string_lossy().into_owned())
}

fn read_front_matter(path: &Path) -> Result<Option<String>, DependencyGraphError> {
    let content =
        fs::read_to_string(path).map_err(|err| DependencyGraphError::Io(err.to_string()))?;

    let mut lines = content.lines();
    if !matches!(lines.next(), Some(start) if start.trim() == "---") {
        return Ok(None);
    }

    let mut front_matter = String::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        front_matter.push_str(line);
        front_matter.push('\n');
    }

    if front_matter.is_empty() {
        Ok(None)
    } else {
        Ok(Some(front_matter))
    }
}
