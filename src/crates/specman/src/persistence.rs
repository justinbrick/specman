use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::adapter::DataModelAdapter;
use crate::dependency_tree::{ArtifactId, ArtifactKind, DependencyInventory, DependencyTree};
use crate::error::SpecmanError;
use crate::front_matter::split_front_matter;
use crate::template::{RenderedTemplate, TemplateProvenance};
use crate::workspace::{WorkspaceLocator, WorkspacePaths};

/// Result of persisting a rendered template to the workspace filesystem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistedArtifact {
    pub artifact: ArtifactId,
    pub path: PathBuf,
    pub workspace: WorkspacePaths,
}

/// Result of removing an artifact directory from the workspace filesystem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovedArtifact {
    pub artifact: ArtifactId,
    pub directory: PathBuf,
    pub workspace: WorkspacePaths,
}

/// Writes rendered templates into canonical workspace locations.
pub struct WorkspacePersistence<L: WorkspaceLocator> {
    locator: L,
    dependency_inventory: Option<Arc<dyn DependencyInventory>>,
    data_adapter: Option<Arc<dyn DataModelAdapter>>,
}

impl<L: WorkspaceLocator> WorkspacePersistence<L> {
    pub fn new(locator: L) -> Self {
        Self {
            locator,
            dependency_inventory: None,
            data_adapter: None,
        }
    }

    pub fn with_inventory(locator: L, dependency_inventory: Arc<dyn DependencyInventory>) -> Self {
        Self {
            locator,
            dependency_inventory: Some(dependency_inventory),
            data_adapter: None,
        }
    }

    pub fn with_adapter(locator: L, data_adapter: Arc<dyn DataModelAdapter>) -> Self {
        Self {
            locator,
            dependency_inventory: None,
            data_adapter: Some(data_adapter),
        }
    }

    pub fn with_inventory_and_adapter(
        locator: L,
        dependency_inventory: Arc<dyn DependencyInventory>,
        data_adapter: Arc<dyn DataModelAdapter>,
    ) -> Self {
        Self {
            locator,
            dependency_inventory: Some(dependency_inventory),
            data_adapter: Some(data_adapter),
        }
    }

    /// Returns the discovered workspace paths for this persistence instance.
    pub fn workspace(&self) -> Result<WorkspacePaths, SpecmanError> {
        self.locator.workspace()
    }

    /// Resolves the canonical filesystem path for the given artifact.
    pub fn artifact_path(&self, artifact: &ArtifactId) -> Result<PathBuf, SpecmanError> {
        let workspace = self.workspace()?;
        resolve_target_path(artifact, &workspace)
    }

    /// Persists a fully composed Markdown document (including YAML front matter when present).
    ///
    /// Unlike `persist`, this does not perform template-token validation.
    pub fn persist_document(
        &self,
        artifact: &ArtifactId,
        document: &str,
    ) -> Result<PersistedArtifact, SpecmanError> {
        ensure_safe_name(&artifact.name)?;

        let workspace = self.workspace()?;
        let target_path = resolve_target_path(artifact, &workspace)?;
        write_body(&target_path, document)?;

        if let Some(inventory) = &self.dependency_inventory {
            inventory.invalidate();
        }
        self.invalidate_tree_in_adapter(artifact)?;

        Ok(PersistedArtifact {
            artifact: artifact.clone(),
            path: target_path,
            workspace,
        })
    }

    /// Persists the rendered template and registers the accompanying dependency tree
    /// through the configured data model adapter when present.
    pub fn persist_with_dependency_tree(
        &self,
        artifact: &ArtifactId,
        rendered: &RenderedTemplate,
        dependencies: &DependencyTree,
    ) -> Result<PersistedArtifact, SpecmanError> {
        let persisted = self.persist(artifact, rendered)?;
        self.save_dependency_tree(artifact, dependencies)?;
        Ok(persisted)
    }

    /// Persists the rendered template to the canonical path defined by the artifact kind.
    pub fn persist(
        &self,
        artifact: &ArtifactId,
        rendered: &RenderedTemplate,
    ) -> Result<PersistedArtifact, SpecmanError> {
        ensure_rendered_tokens_resolved(&rendered.body)?;
        ensure_safe_name(&artifact.name)?;

        let workspace = self.locator.workspace()?;
        let target_path = resolve_target_path(artifact, &workspace)?;
        let output = if let Some(provenance) = &rendered.provenance {
            inject_provenance(&rendered.body, provenance)?
        } else {
            rendered.body.clone()
        };
        write_body(&target_path, &output)?;
        if let Some(inventory) = &self.dependency_inventory {
            inventory.invalidate();
        }

        Ok(PersistedArtifact {
            artifact: artifact.clone(),
            path: target_path,
            workspace,
        })
    }

    /// Recursively removes the canonical artifact directory when dependency guards permit deletion.
    pub fn remove(&self, artifact: &ArtifactId) -> Result<RemovedArtifact, SpecmanError> {
        ensure_safe_name(&artifact.name)?;
        let workspace = self.locator.workspace()?;
        let target_file = resolve_target_path(artifact, &workspace)?;
        let directory = target_file.parent().ok_or_else(|| {
            SpecmanError::Workspace(format!(
                "unable to compute artifact directory for {}",
                artifact.name
            ))
        })?;

        if !directory.exists() {
            return Err(SpecmanError::Workspace(format!(
                "artifact directory does not exist: {}",
                directory.display()
            )));
        }

        // Canonicalize before removal so we return a stable, non-8.3 path representation.
        let canonical_directory = fs::canonicalize(directory)?;
        fs::remove_dir_all(&canonical_directory)?;
        if let Some(inventory) = &self.dependency_inventory {
            inventory.invalidate();
        }
        self.invalidate_tree_in_adapter(artifact)?;

        Ok(RemovedArtifact {
            artifact: artifact.clone(),
            directory: canonical_directory,
            workspace,
        })
    }

    /// Saves the provided dependency tree via the configured data-model adapter.
    pub fn save_dependency_tree(
        &self,
        artifact: &ArtifactId,
        dependencies: &DependencyTree,
    ) -> Result<(), SpecmanError> {
        ensure_dependency_root_matches(artifact, dependencies)?;
        self.save_tree_in_adapter(dependencies)
    }

    /// Invalidates cached dependency data for the specified artifact through the adapter.
    pub fn invalidate_dependency_tree(&self, artifact: &ArtifactId) -> Result<(), SpecmanError> {
        self.invalidate_tree_in_adapter(artifact)
    }

    fn save_tree_in_adapter(&self, dependencies: &DependencyTree) -> Result<(), SpecmanError> {
        if let Some(adapter) = &self.data_adapter {
            adapter.save_dependency_tree(dependencies.clone())?;
        }
        Ok(())
    }

    fn invalidate_tree_in_adapter(&self, artifact: &ArtifactId) -> Result<(), SpecmanError> {
        if let Some(adapter) = &self.data_adapter {
            adapter.invalidate_dependency_tree(artifact)?;
        }
        Ok(())
    }
}

/// Trait abstraction for components capable of removing artifact directories.
pub trait ArtifactRemovalStore: Send + Sync {
    fn remove_artifact(&self, artifact: &ArtifactId) -> Result<RemovedArtifact, SpecmanError>;
}

impl<L: WorkspaceLocator> ArtifactRemovalStore for WorkspacePersistence<L> {
    fn remove_artifact(&self, artifact: &ArtifactId) -> Result<RemovedArtifact, SpecmanError> {
        self.remove(artifact)
    }
}

impl<S> ArtifactRemovalStore for Arc<S>
where
    S: ArtifactRemovalStore,
{
    fn remove_artifact(&self, artifact: &ArtifactId) -> Result<RemovedArtifact, SpecmanError> {
        (**self).remove_artifact(artifact)
    }
}

fn resolve_target_path(
    artifact: &ArtifactId,
    workspace: &WorkspacePaths,
) -> Result<PathBuf, SpecmanError> {
    let (base_dir, file_name) = match artifact.kind {
        ArtifactKind::Specification => (workspace.spec_dir(), "spec.md"),
        ArtifactKind::Implementation => (workspace.impl_dir(), "impl.md"),
        ArtifactKind::ScratchPad => (workspace.scratchpad_dir(), "scratch.md"),
    };

    let folder = base_dir.join(&artifact.name);
    Ok(folder.join(file_name))
}

fn write_body(path: &Path, body: &str) -> Result<(), SpecmanError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, body)?;
    Ok(())
}

fn ensure_dependency_root_matches(
    artifact: &ArtifactId,
    dependencies: &DependencyTree,
) -> Result<(), SpecmanError> {
    if &dependencies.root.id != artifact {
        return Err(SpecmanError::Dependency(format!(
            "dependency tree root {} does not match artifact {}",
            dependencies.root.id.name, artifact.name
        )));
    }
    Ok(())
}

fn inject_provenance(body: &str, provenance: &TemplateProvenance) -> Result<String, SpecmanError> {
    let (body_segment, mut mapping) = match split_front_matter(body) {
        Ok(front) => {
            let mapping: serde_yaml::Mapping = serde_yaml::from_str(front.yaml).map_err(|err| {
                SpecmanError::Template(format!("invalid front matter YAML: {err}"))
            })?;
            (front.body.to_string(), mapping)
        }
        Err(_) => (body.to_string(), serde_yaml::Mapping::new()),
    };
    let prov_value = serde_yaml::to_value(provenance).map_err(|err| {
        SpecmanError::Serialization(format!("unable to encode template provenance: {err}"))
    })?;
    mapping.insert(
        serde_yaml::Value::String("template_source".into()),
        prov_value,
    );
    let mut serialized = serde_yaml::to_string(&mapping).map_err(|err| {
        SpecmanError::Serialization(format!("unable to serialize front matter: {err}"))
    })?;
    if let Some(stripped) = serialized.strip_prefix("---\n") {
        serialized = stripped.to_string();
    }
    if serialized.ends_with("...\n") {
        serialized.truncate(serialized.len() - 4);
    }

    // Preserve the original body segment exactly; only the front matter block changes.
    let updated = format!("---\n{}---\n{}", serialized, body_segment);
    Ok(updated)
}

fn ensure_rendered_tokens_resolved(body: &str) -> Result<(), SpecmanError> {
    if body.contains("{{") {
        return Err(SpecmanError::Template(
            "rendered output still contains template tokens".into(),
        ));
    }
    Ok(())
}

fn ensure_safe_name(name: &str) -> Result<(), SpecmanError> {
    if name.is_empty() {
        return Err(SpecmanError::Workspace(
            "artifact name must not be empty".into(),
        ));
    }

    if name.contains('/') || name.contains('\\') {
        return Err(SpecmanError::Workspace(
            "artifact name must not contain path separators".into(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::DataModelAdapter;
    use crate::dependency_tree::{ArtifactSummary, FilesystemDependencyMapper};
    use crate::template::TemplateDescriptor;
    use crate::workspace::FilesystemWorkspaceLocator;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    fn setup_workspace() -> (
        tempfile::TempDir,
        PathBuf,
        WorkspacePersistence<FilesystemWorkspaceLocator>,
    ) {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        let start = root.join("impl").join("services");
        fs::create_dir_all(&start).unwrap();
        let locator = FilesystemWorkspaceLocator::new(start);
        (temp, root, WorkspacePersistence::new(locator))
    }

    fn workspace_with_locator() -> (tempfile::TempDir, PathBuf, FilesystemWorkspaceLocator) {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        let start = root.join("impl").join("services");
        fs::create_dir_all(&start).unwrap();
        let locator = FilesystemWorkspaceLocator::new(start);
        (temp, root, locator)
    }

    fn rendered(body: &str) -> RenderedTemplate {
        RenderedTemplate {
            body: body.to_string(),
            metadata: TemplateDescriptor::default(),
            provenance: None,
        }
    }

    fn artifact(kind: ArtifactKind, name: &str) -> ArtifactId {
        ArtifactId {
            kind,
            name: name.to_string(),
        }
    }

    fn dependency_tree_for(target: &ArtifactId) -> DependencyTree {
        DependencyTree::empty(ArtifactSummary {
            id: target.clone(),
            ..Default::default()
        })
    }

    #[derive(Default)]
    struct RecordingAdapter {
        saved: Arc<Mutex<Vec<DependencyTree>>>,
        invalidated: Arc<Mutex<Vec<ArtifactId>>>,
    }

    impl RecordingAdapter {
        fn saved_roots(&self) -> Vec<ArtifactId> {
            self.saved
                .lock()
                .unwrap()
                .iter()
                .map(|tree| tree.root.id.clone())
                .collect()
        }

        fn invalidated(&self) -> Vec<ArtifactId> {
            self.invalidated.lock().unwrap().clone()
        }
    }

    impl DataModelAdapter for RecordingAdapter {
        fn save_dependency_tree(&self, tree: DependencyTree) -> Result<(), SpecmanError> {
            self.saved.lock().unwrap().push(tree);
            Ok(())
        }

        fn load_dependency_tree(
            &self,
            _root: &ArtifactId,
        ) -> Result<Option<DependencyTree>, SpecmanError> {
            Ok(None)
        }

        fn invalidate_dependency_tree(&self, root: &ArtifactId) -> Result<(), SpecmanError> {
            self.invalidated.lock().unwrap().push(root.clone());
            Ok(())
        }
    }

    #[test]
    fn persist_specification_creates_directories_and_writes_file() {
        let (_temp, _root, persistence) = setup_workspace();
        let target = artifact(ArtifactKind::Specification, "feature-one");
        let rendered = rendered("---\nname: feature\n---\nbody");

        let result = persistence.persist(&target, &rendered).unwrap();

        assert!(result.path.exists());
        let contents = fs::read_to_string(&result.path).unwrap();
        assert_eq!(contents, rendered.body);
        assert!(
            result
                .path
                .ends_with(std::path::Path::new("spec/feature-one/spec.md"))
        );
    }

    #[test]
    fn persist_scratchpad_targets_dot_folder() {
        let (_temp, _root, persistence) = setup_workspace();
        let target = artifact(ArtifactKind::ScratchPad, "workspace-template-persist");
        let rendered = rendered("scratch content");

        let result = persistence.persist(&target, &rendered).unwrap();
        assert!(result.path.ends_with(std::path::Path::new(
            ".specman/scratchpad/workspace-template-persist/scratch.md"
        )));
    }

    #[test]
    fn persist_rejects_unresolved_tokens() {
        let (_temp, _root, persistence) = setup_workspace();
        let target = artifact(ArtifactKind::Implementation, "specman-library");
        let rendered = rendered("value: {{missing}}");

        let err = persistence.persist(&target, &rendered).unwrap_err();
        assert!(matches!(err, SpecmanError::Template(_)));
    }

    #[test]
    fn remove_specification_deletes_directory() {
        let (_temp, root, persistence) = setup_workspace();
        let folder = root.join("spec").join("feature-one");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("spec.md"), "contents").unwrap();

        // Capture a stable, absolute representation before deletion.
        let folder_canonical = fs::canonicalize(&folder).unwrap();

        let target = artifact(ArtifactKind::Specification, "feature-one");
        let removed = persistence.remove(&target).expect("remove spec");

        assert_eq!(removed.directory, folder_canonical);
        assert!(!folder.exists());
    }

    #[test]
    fn remove_missing_directory_errors() {
        let (_temp, _root, persistence) = setup_workspace();
        let target = artifact(ArtifactKind::Implementation, "unknown");

        let err = persistence.remove(&target).expect_err("missing directory");
        assert!(matches!(err, SpecmanError::Workspace(_)));
    }

    #[test]
    fn remove_scratchpad_deletes_dot_folder() {
        let (_temp, root, persistence) = setup_workspace();
        let folder = root
            .join(".specman")
            .join("scratchpad")
            .join("demo-scratch");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("scratch.md"), "notes").unwrap();

        // Capture a stable, absolute representation before deletion.
        let folder_canonical = fs::canonicalize(&folder).unwrap();

        let target = artifact(ArtifactKind::ScratchPad, "demo-scratch");
        let removed = persistence.remove(&target).expect("remove scratchpad");

        assert_eq!(removed.directory, folder_canonical);
        assert!(!folder.exists());
    }

    #[test]
    fn persist_with_dependency_tree_registers_adapter() {
        let (_temp, _root, locator) = workspace_with_locator();
        let adapter = Arc::new(RecordingAdapter::default());
        let adapter_handle: Arc<dyn DataModelAdapter> = adapter.clone();
        let persistence = WorkspacePersistence::with_adapter(locator, adapter_handle);
        let target = artifact(ArtifactKind::Implementation, "specman-library");
        let rendered = rendered("---\nname: demo\n---\nbody");
        let dependencies = dependency_tree_for(&target);

        let result = persistence
            .persist_with_dependency_tree(&target, &rendered, &dependencies)
            .expect("persist with dependencies");

        assert!(result.path.exists());
        let saved = adapter.saved_roots();
        assert_eq!(saved, vec![target]);
    }

    #[test]
    fn persist_with_dependency_tree_rejects_mismatched_root() {
        let (_temp, _root, locator) = workspace_with_locator();
        let adapter = Arc::new(RecordingAdapter::default());
        let adapter_handle: Arc<dyn DataModelAdapter> = adapter.clone();
        let persistence = WorkspacePersistence::with_adapter(locator, adapter_handle);

        let target = artifact(ArtifactKind::Implementation, "specman-library");
        let rendered = rendered("---\nname: demo\n---\nbody");
        let dependencies = dependency_tree_for(&ArtifactId {
            kind: ArtifactKind::Implementation,
            name: "other".into(),
        });

        let err = persistence
            .persist_with_dependency_tree(&target, &rendered, &dependencies)
            .expect_err("mismatched dependency root");
        assert!(matches!(err, SpecmanError::Dependency(_)));
        assert!(adapter.saved_roots().is_empty());
    }

    #[test]
    fn remove_triggers_dependency_invalidation_when_adapter_configured() {
        let (_temp, root, locator) = workspace_with_locator();
        let adapter = Arc::new(RecordingAdapter::default());
        let adapter_handle: Arc<dyn DataModelAdapter> = adapter.clone();
        let persistence = WorkspacePersistence::with_adapter(locator, adapter_handle);

        let target = artifact(ArtifactKind::Implementation, "specman-library");
        let folder = root.join("impl").join(&target.name);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("impl.md"), "body").unwrap();

        let removed = persistence.remove(&target).expect("remove with adapter");
        assert_eq!(removed.artifact, target);
        assert_eq!(adapter.invalidated(), vec![target.clone()]);
    }

    #[test]
    fn persisting_new_artifact_invalidates_dependency_inventory() {
        let (_temp, root, locator) = workspace_with_locator();
        fs::create_dir_all(root.join("spec")).unwrap();

        let locator = Arc::new(locator);
        let dependency_mapper = FilesystemDependencyMapper::new(locator.clone());
        let persistence = WorkspacePersistence::with_inventory(locator, dependency_mapper.inventory_handle());

        let anchor = artifact(ArtifactKind::Specification, "anchor");
        let anchor_doc = "---\nname: anchor\nversion: '0.1.0'\ndependencies: []\n---\n# Anchor\n";
        persistence
            .persist_document(&anchor, anchor_doc)
            .expect("persist anchor spec");

        dependency_mapper
            .dependency_tree_from_locator("spec://anchor")
            .expect("warm inventory with anchor");

        let fresh = artifact(ArtifactKind::Specification, "fresh");
        let fresh_doc = "---\nname: fresh\nversion: '0.1.0'\ndependencies: []\n---\n# Fresh\n";
        persistence
            .persist_document(&fresh, fresh_doc)
            .expect("persist fresh spec");

        let fresh_tree = dependency_mapper
            .dependency_tree_from_locator("spec://fresh")
            .expect("fresh spec resolves after inventory invalidation");

        assert_eq!(fresh_tree.root.id.name, "fresh");
    }
}
