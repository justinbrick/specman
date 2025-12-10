use crate::error::{Result, SpecmanMcpError};
use crate::handle::{McpResourceHandle, ResourceTarget};
use crate::session::WorkspaceSessionGuard;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specman::dependency_tree::{
    ArtifactKind, ArtifactSummary, DependencyTree, FilesystemDependencyMapper,
};
use specman::workspace::{WorkspaceLocator, WorkspacePaths};
use std::fs;
use std::path::PathBuf;

/// Resource service responsible for workspace description, listing, and read operations.
pub struct ResourceCatalog<L>
where
    L: WorkspaceLocator + Clone,
{
    locator: L,
    mapper: FilesystemDependencyMapper<L>,
}

impl<L> ResourceCatalog<L>
where
    L: WorkspaceLocator + Clone,
{
    /// Creates a new catalog wired to the provided workspace locator.
    pub fn new(locator: L) -> Self {
        Self {
            mapper: FilesystemDependencyMapper::new(locator.clone()),
            locator,
        }
    }

    fn workspace_paths(&self) -> Result<WorkspacePaths> {
        Ok(self.locator.workspace()?)
    }

    /// Returns a guard suitable for enforcing path normalization rules.
    pub fn guard(&self) -> Result<WorkspaceSessionGuard> {
        Ok(WorkspaceSessionGuard::new(self.workspace_paths()?))
    }

    /// Returns the canonical workspace root path.
    pub fn workspace_root(&self) -> Result<PathBuf> {
        Ok(self.workspace_paths()?.root().to_path_buf())
    }

    /// Summarizes the workspace roots and artifact counts to satisfy `workspace.describe`.
    pub fn describe_workspace(&self) -> Result<WorkspaceDescription> {
        let paths = self.workspace_paths()?;
        let specs = discover_artifacts(&paths, ArtifactKind::Specification)?;
        let impls = discover_artifacts(&paths, ArtifactKind::Implementation)?;
        let pads = discover_artifacts(&paths, ArtifactKind::ScratchPad)?;

        Ok(WorkspaceDescription {
            root: paths.root().to_path_buf(),
            spec_dir: paths.spec_dir(),
            impl_dir: paths.impl_dir(),
            scratchpad_dir: paths.scratchpad_dir(),
            counts: WorkspaceArtifactCounts {
                specifications: specs.len(),
                implementations: impls.len(),
                scratchpads: pads.len(),
            },
        })
    }

    /// Lists every MCP resource handle, including `/dependencies` virtual handles.
    pub fn list(&self) -> Result<Vec<ResourceDescriptor>> {
        let paths = self.workspace_paths()?;
        let mut entries = Vec::new();

        for kind in [
            ArtifactKind::Specification,
            ArtifactKind::Implementation,
            ArtifactKind::ScratchPad,
        ] {
            for artifact in discover_artifacts(&paths, kind)? {
                let tree = self.mapper.dependency_tree_from_path(&artifact.path)?;
                entries.push(ResourceDescriptor {
                    handle: artifact.handle.uri(),
                    variant: ResourceVariant::Artifact,
                    artifact_kind: kind,
                    path: artifact.path.clone(),
                    summary: tree.root.clone(),
                });
                entries.push(ResourceDescriptor {
                    handle: artifact.handle.dependencies_uri(),
                    variant: ResourceVariant::Dependencies,
                    artifact_kind: kind,
                    path: artifact.path.clone(),
                    summary: tree.root,
                });
            }
        }

        entries.sort_by(|a, b| a.handle.cmp(&b.handle));
        Ok(entries)
    }

    /// Reads either the artifact body or its dependency tree, depending on the handle.
    pub fn read(&self, reference: &str) -> Result<ResourceRead> {
        let target = ResourceTarget::parse(reference)?
            .ok_or_else(|| SpecmanMcpError::invalid_handle(reference))?;
        let paths = self.workspace_paths()?;
        match target {
            ResourceTarget::Artifact(handle) => self.read_artifact(handle, &paths),
            ResourceTarget::Dependencies(handle) => self.read_dependencies(handle, &paths),
        }
    }

    fn read_artifact(
        &self,
        handle: McpResourceHandle,
        workspace: &WorkspacePaths,
    ) -> Result<ResourceRead> {
        let path = handle.to_path(workspace);
        if !path.is_file() {
            return Err(SpecmanMcpError::resource(format!(
                "artifact {} not found at {}",
                handle.uri(),
                path.display()
            )));
        }
        let canonical = fs::canonicalize(&path)?;
        let content = fs::read_to_string(&canonical)?;
        let tree = self.mapper.dependency_tree_from_path(&canonical)?;
        Ok(ResourceRead::Artifact(ArtifactResource {
            handle: handle.uri(),
            path: canonical,
            summary: tree.root,
            content,
        }))
    }

    fn read_dependencies(
        &self,
        handle: McpResourceHandle,
        workspace: &WorkspacePaths,
    ) -> Result<ResourceRead> {
        let path = handle.to_path(workspace);
        if !path.is_file() {
            return Err(SpecmanMcpError::resource(format!(
                "artifact {} not found at {}",
                handle.dependencies_uri(),
                path.display()
            )));
        }
        let canonical = fs::canonicalize(&path)?;
        let tree = self.mapper.dependency_tree_from_path(&canonical)?;
        Ok(ResourceRead::Dependencies(DependencyResource {
            handle: handle.dependencies_uri(),
            tree,
        }))
    }
}

/// Workspace-level metadata returned to MCP clients.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceDescription {
    pub root: PathBuf,
    pub spec_dir: PathBuf,
    pub impl_dir: PathBuf,
    pub scratchpad_dir: PathBuf,
    pub counts: WorkspaceArtifactCounts,
}

/// Artifact counts grouped by kind.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct WorkspaceArtifactCounts {
    pub specifications: usize,
    pub implementations: usize,
    pub scratchpads: usize,
}

/// Descriptor returned from `resources/list`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResourceDescriptor {
    pub handle: String,
    pub variant: ResourceVariant,
    pub artifact_kind: ArtifactKind,
    pub path: PathBuf,
    pub summary: ArtifactSummary,
}

/// Identifies whether the descriptor refers to the artifact body or `/dependencies` virtual handle.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceVariant {
    Artifact,
    Dependencies,
}

/// Result of invoking `resources/read`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum ResourceRead {
    Artifact(ArtifactResource),
    Dependencies(DependencyResource),
}

/// Artifact body payload returned from `resources/read`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactResource {
    pub handle: String,
    pub path: PathBuf,
    pub summary: ArtifactSummary,
    pub content: String,
}

/// Dependency tree payload returned from `<handle>/dependencies`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DependencyResource {
    pub handle: String,
    pub tree: DependencyTree,
}

#[derive(Clone, Debug)]
struct ArtifactRecord {
    handle: McpResourceHandle,
    path: PathBuf,
}

fn discover_artifacts(paths: &WorkspacePaths, kind: ArtifactKind) -> Result<Vec<ArtifactRecord>> {
    let (dir, marker) = match kind {
        ArtifactKind::Specification => (paths.spec_dir(), "spec.md"),
        ArtifactKind::Implementation => (paths.impl_dir(), "impl.md"),
        ArtifactKind::ScratchPad => (paths.scratchpad_dir(), "scratch.md"),
    };
    collect_named_files(dir, marker, kind)
}

fn collect_named_files(dir: PathBuf, marker: &str, kind: ArtifactKind) -> Result<Vec<ArtifactRecord>> {
    let mut artifacts = Vec::new();
    if !dir.is_dir() {
        return Ok(artifacts);
    }

    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let slug = entry
            .file_name()
            .to_string_lossy()
            .to_string();
        let handle = match McpResourceHandle::from_slug(kind, &slug) {
            Ok(handle) => handle,
            Err(err) => {
                tracing::warn!(?err, slug, "skipping artifact due to invalid slug");
                continue;
            }
        };
        let candidate = entry.path().join(marker);
        if !candidate.is_file() {
            continue;
        }
        artifacts.push(ArtifactRecord {
            handle,
            path: candidate,
        });
    }

    artifacts.sort_by(|a, b| a.handle.uri().cmp(&b.handle.uri()));
    for artifact in &mut artifacts {
        artifact.path = fs::canonicalize(&artifact.path)?;
    }
    Ok(artifacts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use specman::workspace::FilesystemWorkspaceLocator;
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    }

    fn bootstrap_workspace() -> (tempfile::TempDir, std::sync::Arc<FilesystemWorkspaceLocator>) {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman").join("scratchpad")).unwrap();
        fs::create_dir_all(root.join("spec").join("specman-core")).unwrap();
        fs::create_dir_all(root.join("impl").join("workflow-engine")).unwrap();

        write_file(
            &root.join("spec/specman-core/spec.md"),
            r"---
name: specman-core
version: '1.0.0'
---
# SpecMan Core
",
        );
        write_file(
            &root.join("impl/workflow-engine/impl.md"),
            r"---
name: workflow-engine
spec: spec://specman-core
---
# Impl
",
        );

        let locator = std::sync::Arc::new(FilesystemWorkspaceLocator::new(root));
        (temp, locator)
    }

    #[test]
    fn list_and_read_resources() {
        let (_temp, locator) = bootstrap_workspace();
        let catalog = ResourceCatalog::new(locator.clone());

        let descriptors = catalog.list().expect("list resources");
        assert!(descriptors.iter().any(|d| d.handle == "spec://specman-core"));
        assert!(descriptors
            .iter()
            .any(|d| d.handle == "spec://specman-core/dependencies"));

        let artifact = catalog
            .read("spec://specman-core")
            .expect("read artifact");
        match artifact {
            ResourceRead::Artifact(payload) => {
                assert!(payload.content.contains("SpecMan Core"));
            }
            _ => panic!("expected artifact payload"),
        }

        let deps = catalog
            .read("spec://specman-core/dependencies")
            .expect("read deps");
        match deps {
            ResourceRead::Dependencies(payload) => {
                assert_eq!(payload.tree.root.id.name, "specman-core");
            }
            _ => panic!("expected dependencies payload"),
        }
    }
}
