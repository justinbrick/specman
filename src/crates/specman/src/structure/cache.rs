use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::dependency_tree::ArtifactKind;
use crate::error::SpecmanError;
use crate::front_matter::{
    ArtifactFrontMatter, ImplementationFrontMatter, ScratchFrontMatter, SpecificationFrontMatter,
};
use crate::workspace::{normalize_workspace_path, WorkspacePaths};

use super::index::{
    ArtifactKey, ArtifactRecord, ConstraintIdentifier, ConstraintRecord, HeadingIdentifier,
    HeadingRecord, RelationshipEdge, RelationshipKind, WORKSPACE_INDEX_SCHEMA_VERSION,
    WorkspaceIndex,
};

const INDEX_DIR_NAME: &str = "index";
const LOCK_FILE_NAME: &str = ".lock";
const MANIFEST_FILE_NAME: &str = "manifest.json";

fn index_file_name() -> String {
    format!("index.v{}.json", WORKSPACE_INDEX_SCHEMA_VERSION)
}

/// Disk-backed cache for the workspace structure index.
///
/// Rooted at `.specman/cache/index`.
#[derive(Clone, Debug)]
pub struct IndexCache {
    root: PathBuf,
}

impl IndexCache {
    pub fn new(workspace: &WorkspacePaths) -> Self {
        Self {
            root: workspace.dot_specman().join("cache").join(INDEX_DIR_NAME),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn purge(&self) -> Result<(), SpecmanError> {
        if self.root.is_dir() {
            fs::remove_dir_all(&self.root)?;
        }
        Ok(())
    }

    pub fn load_if_fresh(
        &self,
        workspace: &WorkspacePaths,
        canonical_spec_impl: &[(ArtifactKind, PathBuf)],
    ) -> Result<Option<(WorkspaceIndex, Vec<UnresolvedHeadingRef>)>, SpecmanError> {
        self.fail_fast_if_locked()?;
        let manifest = match self.read_manifest()? {
            Some(m) => m,
            None => return Ok(None),
        };

        if manifest.schema_version != WORKSPACE_INDEX_SCHEMA_VERSION {
            return Ok(None);
        }

        let workspace_fingerprint = root_fingerprint(workspace)?;
        if manifest.workspace_root_fingerprint != workspace_fingerprint {
            return Ok(None);
        }

        if !self.is_fresh(&manifest, workspace, canonical_spec_impl)? {
            return Ok(None);
        }

        let index_path = self.root.join(&manifest.index_file);
        if !index_path.is_file() {
            return Ok(None);
        }
        let content = fs::read_to_string(&index_path)?;
        let persisted: PersistedWorkspaceIndex = serde_json::from_str(&content).map_err(|err| {
            SpecmanError::Serialization(format!(
                "invalid cached workspace index {}: {err}",
                index_path.display()
            ))
        })?;

        if persisted.schema_version != WORKSPACE_INDEX_SCHEMA_VERSION {
            return Ok(None);
        }

        let unresolved = persisted
            .unresolved_heading_refs
            .clone()
            .into_iter()
            .map(UnresolvedHeadingRef::from)
            .collect();
        let index = persisted.rehydrate(workspace)?;
        Ok(Some((index, unresolved)))
    }

    pub fn save(
        &self,
        workspace: &WorkspacePaths,
        canonical_spec_impl: &[(ArtifactKind, PathBuf)],
        index: &WorkspaceIndex,
        unresolved_refs: &[UnresolvedHeadingRef],
    ) -> Result<(), SpecmanError> {
        let lock = IndexCacheLock::acquire(&self.root)?;

        fs::create_dir_all(&self.root).map_err(|err| {
            SpecmanError::Workspace(format!(
                "failed to prepare index cache directory {}: {err}",
                self.root.display()
            ))
        })?;

        let manifest = build_manifest_for_save(workspace, canonical_spec_impl, &index_file_name())?;

        let persisted = PersistedWorkspaceIndex::from_index(index, unresolved_refs);

        let index_path = self.root.join(&manifest.index_file);
        let manifest_path = self.root.join(MANIFEST_FILE_NAME);

        write_atomic_json(&index_path, &persisted)?;
        write_atomic_json(&manifest_path, &manifest)?;

        drop(lock);
        Ok(())
    }

    fn is_fresh(
        &self,
        manifest: &IndexManifest,
        workspace: &WorkspacePaths,
        canonical_spec_impl: &[(ArtifactKind, PathBuf)],
    ) -> Result<bool, SpecmanError> {
        let expected: BTreeSet<ManifestArtifactEntry> = canonical_spec_impl
            .iter()
            .map(|(kind, path)| ManifestArtifactEntry::from_path(*kind, path, workspace))
            .collect::<Result<_, _>>()?;

        let actual: BTreeSet<ManifestArtifactEntry> = manifest.artifacts.iter().cloned().collect();
        Ok(expected == actual)
    }

    fn read_manifest(&self) -> Result<Option<IndexManifest>, SpecmanError> {
        let manifest_path = self.root.join(MANIFEST_FILE_NAME);
        if !manifest_path.is_file() {
            return Ok(None);
        }
        let content = fs::read_to_string(&manifest_path)?;
        let manifest: IndexManifest = serde_json::from_str(&content).map_err(|err| {
            SpecmanError::Serialization(format!(
                "invalid index cache manifest {}: {err}",
                manifest_path.display()
            ))
        })?;
        Ok(Some(manifest))
    }

    fn fail_fast_if_locked(&self) -> Result<(), SpecmanError> {
        let lock_path = self.root.join(LOCK_FILE_NAME);
        if lock_path.exists() {
            return Err(SpecmanError::Workspace(format!(
                "workspace index cache is locked by another process: {}",
                lock_path.display()
            )));
        }
        Ok(())
    }
}

struct IndexCacheLock {
    path: PathBuf,
}

impl IndexCacheLock {
    fn acquire(dir: &Path) -> Result<Self, SpecmanError> {
        fs::create_dir_all(dir).map_err(|err| {
            SpecmanError::Workspace(format!(
                "failed to prepare index cache directory {}: {err}",
                dir.display()
            ))
        })?;
        let lock_path = dir.join(LOCK_FILE_NAME);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(_) => Ok(Self { path: lock_path }),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                Err(SpecmanError::Workspace(format!(
                    "workspace index cache is locked by another process: {}",
                    lock_path.display()
                )))
            }
            Err(err) => Err(SpecmanError::Workspace(format!(
                "failed to create index cache lock {}: {err}",
                lock_path.display()
            ))),
        }
    }
}

impl Drop for IndexCacheLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct ManifestArtifactEntry {
    workspace_path: String,
    kind: ArtifactKind,
    mtime_unix_ms: u64,
    size: u64,
}

impl ManifestArtifactEntry {
    fn from_path(
        kind: ArtifactKind,
        canonical_path: &Path,
        workspace: &WorkspacePaths,
    ) -> Result<Self, SpecmanError> {
        if !canonical_path.starts_with(workspace.root()) {
            return Err(SpecmanError::Workspace(format!(
                "artifact {} escapes workspace {}",
                canonical_path.display(),
                workspace.root().display()
            )));
        }
        let workspace_path = canonical_path
            .strip_prefix(workspace.root())
            .map_err(|_| {
                SpecmanError::Workspace(format!(
                    "unable to compute workspace-relative path for {}",
                    canonical_path.display()
                ))
            })?
            .to_string_lossy()
            .replace('\\', "/");

        let metadata = fs::metadata(canonical_path)?;
        let size = metadata.len();
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let mtime_unix_ms = system_time_to_unix_ms(modified);
        Ok(Self {
            workspace_path,
            kind,
            mtime_unix_ms,
            size,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct IndexManifest {
    schema_version: u32,
    workspace_root_fingerprint: String,
    generated_at_unix_ms: u64,
    index_file: String,
    artifacts: Vec<ManifestArtifactEntry>,
}

fn build_manifest(
    fingerprint: String,
    workspace: &WorkspacePaths,
    canonical_spec_impl: &[(ArtifactKind, PathBuf)],
    index_file: &str,
) -> Result<IndexManifest, SpecmanError> {
    let mut artifacts: Vec<ManifestArtifactEntry> = canonical_spec_impl
        .iter()
        .map(|(kind, path)| ManifestArtifactEntry::from_path(*kind, path, workspace))
        .collect::<Result<_, _>>()?;
    artifacts.sort();

    Ok(IndexManifest {
        schema_version: WORKSPACE_INDEX_SCHEMA_VERSION,
        workspace_root_fingerprint: fingerprint,
        generated_at_unix_ms: now_unix_ms(),
        index_file: index_file.to_string(),
        artifacts,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
enum PersistedFrontMatter {
    Specification(SpecificationFrontMatter),
    Implementation(ImplementationFrontMatter),
    Scratch(ScratchFrontMatter),
}

impl From<&ArtifactFrontMatter> for PersistedFrontMatter {
    fn from(value: &ArtifactFrontMatter) -> Self {
        match value {
            ArtifactFrontMatter::Specification(v) => Self::Specification(v.clone()),
            ArtifactFrontMatter::Implementation(v) => Self::Implementation(v.clone()),
            ArtifactFrontMatter::Scratch(v) => Self::Scratch(v.clone()),
        }
    }
}

impl From<PersistedFrontMatter> for ArtifactFrontMatter {
    fn from(value: PersistedFrontMatter) -> Self {
        match value {
            PersistedFrontMatter::Specification(v) => ArtifactFrontMatter::Specification(v),
            PersistedFrontMatter::Implementation(v) => ArtifactFrontMatter::Implementation(v),
            PersistedFrontMatter::Scratch(v) => ArtifactFrontMatter::Scratch(v),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PersistedArtifactKey {
    kind: ArtifactKind,
    workspace_path: String,
}

impl From<&ArtifactKey> for PersistedArtifactKey {
    fn from(value: &ArtifactKey) -> Self {
        Self {
            kind: value.kind,
            workspace_path: value.workspace_path.clone(),
        }
    }
}

impl From<PersistedArtifactKey> for ArtifactKey {
    fn from(value: PersistedArtifactKey) -> Self {
        Self {
            kind: value.kind,
            workspace_path: value.workspace_path,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedArtifactRecord {
    key: PersistedArtifactKey,
    front_matter: Option<PersistedFrontMatter>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PersistedHeadingIdentifier {
    artifact: PersistedArtifactKey,
    slug: String,
}

impl From<&HeadingIdentifier> for PersistedHeadingIdentifier {
    fn from(value: &HeadingIdentifier) -> Self {
        Self {
            artifact: PersistedArtifactKey::from(&value.artifact),
            slug: value.slug.clone(),
        }
    }
}

impl From<PersistedHeadingIdentifier> for HeadingIdentifier {
    fn from(value: PersistedHeadingIdentifier) -> Self {
        Self {
            artifact: ArtifactKey::from(value.artifact),
            slug: value.slug,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedHeadingRecord {
    id: PersistedHeadingIdentifier,
    level: u8,
    title: String,
    order: usize,
    parent: Option<PersistedHeadingIdentifier>,
    children: Vec<PersistedHeadingIdentifier>,
    content: String,
    referenced_headings: Vec<PersistedHeadingIdentifier>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PersistedConstraintIdentifier {
    artifact: PersistedArtifactKey,
    group: String,
}

impl From<&ConstraintIdentifier> for PersistedConstraintIdentifier {
    fn from(value: &ConstraintIdentifier) -> Self {
        Self {
            artifact: PersistedArtifactKey::from(&value.artifact),
            group: value.group.clone(),
        }
    }
}

impl From<PersistedConstraintIdentifier> for ConstraintIdentifier {
    fn from(value: PersistedConstraintIdentifier) -> Self {
        Self {
            artifact: ArtifactKey::from(value.artifact),
            group: value.group,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedConstraintRecord {
    id: PersistedConstraintIdentifier,
    heading: PersistedHeadingIdentifier,
    line: usize,
    referenced_headings: Vec<PersistedHeadingIdentifier>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedRelationshipEdge {
    kind: RelationshipKind,
    from: String,
    to: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedWorkspaceIndex {
    schema_version: u32,
    artifacts: Vec<PersistedArtifactRecord>,
    headings: Vec<PersistedHeadingRecord>,
    constraints: Vec<PersistedConstraintRecord>,
    relationships: Vec<PersistedRelationshipEdge>,
    unresolved_heading_refs: Vec<PersistedUnresolvedHeadingRef>,
}

impl PersistedWorkspaceIndex {
    fn from_index(index: &WorkspaceIndex, unresolved_refs: &[UnresolvedHeadingRef]) -> Self {
        let mut artifacts: Vec<PersistedArtifactRecord> = index
            .artifacts
            .values()
            .filter(|record| record.key.kind != ArtifactKind::ScratchPad)
            .map(|record| PersistedArtifactRecord {
                key: PersistedArtifactKey::from(&record.key),
                front_matter: record.front_matter.as_ref().map(PersistedFrontMatter::from),
            })
            .collect();
        artifacts.sort_by(|a, b| a.key.cmp(&b.key));

        let mut derived_unresolved: Vec<UnresolvedHeadingRef> = Vec::new();

        let mut headings: Vec<PersistedHeadingRecord> = index
            .headings
            .values()
            .filter(|record| record.id.artifact.kind != ArtifactKind::ScratchPad)
            .map(|record| PersistedHeadingRecord {
                id: PersistedHeadingIdentifier::from(&record.id),
                level: record.level,
                title: record.title.clone(),
                order: record.order,
                parent: record.parent.as_ref().map(PersistedHeadingIdentifier::from),
                children: record
                    .children
                    .iter()
                    .map(PersistedHeadingIdentifier::from)
                    .collect(),
                content: record.content.clone(),
                referenced_headings: record
                    .referenced_headings
                    .iter()
                    .filter(|id| id.artifact.kind != ArtifactKind::ScratchPad)
                    .map(PersistedHeadingIdentifier::from)
                    .collect(),
            })
            .collect();
        headings.sort_by(|a, b| a.id.cmp(&b.id));

        // Convert any spec/impl heading references that target scratch into unresolved refs.
        // This preserves full-index semantics even though scratch artifacts are never persisted.
        for record in index.headings.values() {
            if record.id.artifact.kind == ArtifactKind::ScratchPad {
                continue;
            }
            for target in &record.referenced_headings {
                if target.artifact.kind != ArtifactKind::ScratchPad {
                    continue;
                }
                derived_unresolved.push(UnresolvedHeadingRef {
                    from: record.id.clone(),
                    target: UnresolvedTarget::InterDoc {
                        workspace_path: target.artifact.workspace_path.clone(),
                        slug: target.slug.clone(),
                    },
                });
            }
        }

        let mut constraints: Vec<PersistedConstraintRecord> = index
            .constraints
            .values()
            .filter(|record| record.id.artifact.kind != ArtifactKind::ScratchPad)
            .map(|record| PersistedConstraintRecord {
                id: PersistedConstraintIdentifier::from(&record.id),
                heading: PersistedHeadingIdentifier::from(&record.heading),
                line: record.line,
                referenced_headings: record
                    .referenced_headings
                    .iter()
                    .filter(|id| id.artifact.kind != ArtifactKind::ScratchPad)
                    .map(PersistedHeadingIdentifier::from)
                    .collect(),
            })
            .collect();
        constraints.sort_by(|a, b| a.id.cmp(&b.id));

        let relationships: Vec<PersistedRelationshipEdge> = index
            .relationships
            .iter()
            .filter(|edge| {
                // Drop any relationships that mention scratchpad workspace paths.
                // Scratch pads are not persisted in the cache.
                !edge.from.starts_with(".specman/scratchpad/")
                    && !edge.to.starts_with(".specman/scratchpad/")
            })
            .map(|edge| PersistedRelationshipEdge {
                kind: edge.kind,
                from: edge.from.clone(),
                to: edge.to.clone(),
            })
            .collect();

        let mut all_unresolved: Vec<UnresolvedHeadingRef> = unresolved_refs.to_vec();
        all_unresolved.extend(derived_unresolved);
        let unresolved_heading_refs = all_unresolved
            .iter()
            .map(PersistedUnresolvedHeadingRef::from)
            .collect();

        Self {
            schema_version: WORKSPACE_INDEX_SCHEMA_VERSION,
            artifacts,
            headings,
            constraints,
            relationships,
            unresolved_heading_refs,
        }
    }

    fn rehydrate(self, workspace: &WorkspacePaths) -> Result<WorkspaceIndex, SpecmanError> {
        let mut index = WorkspaceIndex {
            schema_version: self.schema_version,
            workspace_root: workspace.root().to_path_buf(),
            ..Default::default()
        };

        for record in self.artifacts {
            let key: ArtifactKey = record.key.into();
            let absolute_path = workspace.root().join(&key.workspace_path);
            let normalized = normalize_workspace_path(&absolute_path);
            let normalized_root = normalize_workspace_path(workspace.root());
            if !normalized.starts_with(workspace.root()) && !normalized.starts_with(&normalized_root) {
                return Err(SpecmanError::Workspace(format!(
                    "cached artifact {} escapes workspace {}",
                    normalized.display(),
                    workspace.root().display()
                )));
            }
            let front_matter = record.front_matter.map(ArtifactFrontMatter::from);
            index.artifacts.insert(
                key.clone(),
                ArtifactRecord {
                    key,
                    absolute_path: normalized,
                    front_matter,
                },
            );
        }

        for record in self.headings {
            let id: HeadingIdentifier = record.id.into();
            index.headings.insert(
                id.clone(),
                HeadingRecord {
                    id,
                    level: record.level,
                    title: record.title,
                    order: record.order,
                    parent: record.parent.map(HeadingIdentifier::from),
                    children: record
                        .children
                        .into_iter()
                        .map(HeadingIdentifier::from)
                        .collect(),
                    content: record.content,
                    referenced_headings: record
                        .referenced_headings
                        .into_iter()
                        .map(HeadingIdentifier::from)
                        .collect(),
                },
            );
        }

        for record in self.constraints {
            let id: ConstraintIdentifier = record.id.into();
            index.constraints.insert(
                id.clone(),
                ConstraintRecord {
                    id,
                    heading: HeadingIdentifier::from(record.heading),
                    line: record.line,
                    referenced_headings: record
                        .referenced_headings
                        .into_iter()
                        .map(HeadingIdentifier::from)
                        .collect(),
                },
            );
        }

        index.relationships = self
            .relationships
            .into_iter()
            .map(|edge| RelationshipEdge {
                kind: edge.kind,
                from: edge.from,
                to: edge.to,
            })
            .collect();

        Ok(index)
    }
}

/// Unresolved heading refs discovered while parsing spec/impl artifacts.
///
/// These are persisted so that scratch pads (which are never stored in the cache)
/// can still satisfy links from cached documents without changing indexing semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnresolvedHeadingRef {
    pub from: HeadingIdentifier,
    pub target: UnresolvedTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnresolvedTarget {
    IntraDoc {
        slug: String,
    },
    InterDoc {
        workspace_path: String,
        slug: String,
    },
    File {
        workspace_path: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedUnresolvedHeadingRef {
    from: PersistedHeadingIdentifier,
    target: PersistedUnresolvedTarget,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
enum PersistedUnresolvedTarget {
    IntraDoc {
        slug: String,
    },
    InterDoc {
        workspace_path: String,
        slug: String,
    },
    File {
        workspace_path: String,
    },
}

impl From<&UnresolvedHeadingRef> for PersistedUnresolvedHeadingRef {
    fn from(value: &UnresolvedHeadingRef) -> Self {
        Self {
            from: PersistedHeadingIdentifier::from(&value.from),
            target: match &value.target {
                UnresolvedTarget::IntraDoc { slug } => {
                    PersistedUnresolvedTarget::IntraDoc { slug: slug.clone() }
                }
                UnresolvedTarget::InterDoc {
                    workspace_path,
                    slug,
                } => PersistedUnresolvedTarget::InterDoc {
                    workspace_path: workspace_path.clone(),
                    slug: slug.clone(),
                },
                UnresolvedTarget::File { workspace_path } => PersistedUnresolvedTarget::File {
                    workspace_path: workspace_path.clone(),
                },
            },
        }
    }
}

impl From<PersistedUnresolvedHeadingRef> for UnresolvedHeadingRef {
    fn from(value: PersistedUnresolvedHeadingRef) -> Self {
        Self {
            from: HeadingIdentifier::from(value.from),
            target: match value.target {
                PersistedUnresolvedTarget::IntraDoc { slug } => UnresolvedTarget::IntraDoc { slug },
                PersistedUnresolvedTarget::InterDoc {
                    workspace_path,
                    slug,
                } => UnresolvedTarget::InterDoc {
                    workspace_path,
                    slug,
                },
                PersistedUnresolvedTarget::File { workspace_path } => {
                    UnresolvedTarget::File { workspace_path }
                }
            },
        }
    }
}

fn write_atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<(), SpecmanError> {
    let tmp_path = path.with_extension("tmp");
    let payload = serde_json::to_string_pretty(value)?;
    fs::write(&tmp_path, payload).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        SpecmanError::Workspace(format!(
            "failed to write temporary cache file {}: {err}",
            tmp_path.display()
        ))
    })?;

    if path.is_file() {
        fs::remove_file(path).map_err(|err| {
            let _ = fs::remove_file(&tmp_path);
            SpecmanError::Workspace(format!(
                "failed to remove previous cache file {}: {err}",
                path.display()
            ))
        })?;
    }

    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        SpecmanError::Workspace(format!(
            "failed to publish cache file {}: {err}",
            path.display()
        ))
    })
}

fn system_time_to_unix_ms(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

fn now_unix_ms() -> u64 {
    system_time_to_unix_ms(SystemTime::now())
}

fn root_fingerprint(workspace: &WorkspacePaths) -> Result<String, SpecmanError> {
    let path = workspace.dot_specman().join("root_fingerprint");
    if path.is_file() {
        let content = fs::read_to_string(&path)?;
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Err(SpecmanError::Workspace(format!(
                "workspace root fingerprint {} is empty",
                path.display()
            )));
        }
        return Ok(trimmed.to_string());
    }
    fs::create_dir_all(workspace.dot_specman()).map_err(|err| {
        SpecmanError::Workspace(format!(
            "failed to prepare workspace dot directory {}: {err}",
            workspace.dot_specman().display()
        ))
    })?;
    let uuid = generate_uuid_v4();
    fs::write(&path, format!("{uuid}\n")).map_err(|err| {
        SpecmanError::Workspace(format!(
            "failed to write workspace root fingerprint {}: {err}",
            path.display()
        ))
    })?;
    Ok(uuid)
}

fn generate_uuid_v4() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for (idx, b) in bytes.iter_mut().enumerate() {
            *b = ((now >> (idx * 8)) & 0xFF) as u8;
        }
    }

    // Set version (4) and variant (RFC 4122).
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

pub(crate) fn resolve_unresolved_refs(
    index: &mut WorkspaceIndex,
    unresolved: Vec<UnresolvedHeadingRef>,
) {
    let mut artifact_by_path: BTreeMap<String, ArtifactKey> = BTreeMap::new();
    for key in index.artifacts.keys() {
        artifact_by_path.insert(key.workspace_path.clone(), key.clone());
    }

    for pending in unresolved {
        match pending.target {
            UnresolvedTarget::IntraDoc { slug } => {
                let to = HeadingIdentifier {
                    artifact: pending.from.artifact.clone(),
                    slug,
                };
                if index.headings.contains_key(&to) {
                    attach_heading_reference(index, &pending.from, &to);
                    index.relationships.push(RelationshipEdge {
                        kind: RelationshipKind::HeadingToHeading,
                        from: heading_ref_string(&pending.from),
                        to: heading_ref_string(&to),
                    });
                }
            }
            UnresolvedTarget::InterDoc {
                workspace_path,
                slug,
            } => {
                let Some(artifact_key) = artifact_by_path.get(&workspace_path).cloned() else {
                    continue;
                };
                let to = HeadingIdentifier {
                    artifact: artifact_key,
                    slug,
                };
                if index.headings.contains_key(&to) {
                    attach_heading_reference(index, &pending.from, &to);
                    index.relationships.push(RelationshipEdge {
                        kind: RelationshipKind::HeadingToHeading,
                        from: heading_ref_string(&pending.from),
                        to: heading_ref_string(&to),
                    });
                }
            }
            UnresolvedTarget::File { workspace_path } => {
                index.relationships.push(RelationshipEdge {
                    kind: RelationshipKind::HeadingToFile,
                    from: heading_ref_string(&pending.from),
                    to: workspace_path,
                });
            }
        }
    }
}

fn attach_heading_reference(
    index: &mut WorkspaceIndex,
    from: &HeadingIdentifier,
    to: &HeadingIdentifier,
) {
    if let Some(from_record) = index.headings.get_mut(from) {
        from_record.referenced_headings.push(to.clone());
    }
}

fn heading_ref_string(id: &HeadingIdentifier) -> String {
    format!("{}#{}", id.artifact.workspace_path, id.slug)
}

fn build_manifest_for_save(
    workspace: &WorkspacePaths,
    canonical_spec_impl: &[(ArtifactKind, PathBuf)],
    index_file: &str,
) -> Result<IndexManifest, SpecmanError> {
    let fingerprint = root_fingerprint(workspace)?;
    build_manifest(fingerprint, workspace, canonical_spec_impl, index_file)
}
