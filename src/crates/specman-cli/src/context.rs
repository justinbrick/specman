use std::path::PathBuf;

use specman::dependency_tree::{ArtifactId, DependencyMapping, FilesystemDependencyMapper};
use specman::lifecycle::DefaultLifecycleController;
use specman::persistence::WorkspacePersistence;
use specman::template::MarkdownTemplateEngine;
use specman::workspace::{FilesystemWorkspaceLocator, WorkspaceLocator, WorkspacePaths};
use specman::{DataModelAdapter, InMemoryAdapter, SpecmanError};
use std::sync::Arc;

use crate::error::CliError;
use crate::templates::TemplateCatalog;
use crate::util::Verbosity;

/// Aggregates the workspace context, adapters, and shared services required by the
/// SpecMan CLI. This keeps Workspace Context Resolution deterministic and ensures
/// every command reuses the same dependency mapper, template engine, and lifecycle
/// controller for the duration of a single invocation (see
/// spec/specman-cli/spec.md#concept-workspace-context-resolution).
pub struct CliSession {
    pub workspace_paths: WorkspacePaths,
    pub dependency_mapper: Arc<FilesystemDependencyMapper<Arc<FilesystemWorkspaceLocator>>>,
    pub persistence: Arc<WorkspacePersistence<Arc<FilesystemWorkspaceLocator>>>,
    pub template_engine: Arc<MarkdownTemplateEngine>,
    pub templates: TemplateCatalog,
    pub lifecycle: Arc<
        DefaultLifecycleController<
            Arc<FilesystemDependencyMapper<Arc<FilesystemWorkspaceLocator>>>,
            Arc<MarkdownTemplateEngine>,
        >,
    >, // Centralized lifecycle guard rails shared across commands.
    pub verbosity: Verbosity,
}

impl CliSession {
    /// Creates a new session by resolving the workspace root (optionally honoring
    /// `--workspace`), instantiating the default adapter stack, and wiring lifecycle
    /// automation so downstream commands can satisfy the Workspace Context Resolution
    /// and Data Model Activation concepts.
    pub fn bootstrap(
        workspace_override: Option<String>,
        verbosity: Verbosity,
    ) -> Result<Self, CliError> {
        let locator = match workspace_override {
            Some(path) => {
                let locator = FilesystemWorkspaceLocator::new(PathBuf::from(path));
                locator.workspace()?;
                locator
            }
            None => FilesystemWorkspaceLocator::from_current_dir()?,
        };

        let workspace_locator = Arc::new(locator);
        let workspace_paths = workspace_locator.workspace()?;
        let dependency_mapper =
            Arc::new(FilesystemDependencyMapper::new(workspace_locator.clone()));
        let data_adapter: Arc<dyn DataModelAdapter> = Arc::new(InMemoryAdapter::new());
        let persistence = Arc::new(WorkspacePersistence::with_inventory_and_adapter(
            workspace_locator.clone(),
            dependency_mapper.inventory_handle(),
            data_adapter.clone(),
        ));
        let template_engine = Arc::new(MarkdownTemplateEngine::default());
        let templates = TemplateCatalog::new(workspace_paths.clone());
        let lifecycle = Arc::new(DefaultLifecycleController::new(
            dependency_mapper.clone(),
            template_engine.clone(),
        ));

        Ok(Self {
            workspace_paths,
            dependency_mapper,
            persistence,
            template_engine,
            templates,
            lifecycle,
            verbosity,
        })
    }

    /// Recomputes and persists the dependency tree for the provided artifact.
    pub fn record_dependency_tree(&self, artifact: &ArtifactId) -> Result<(), SpecmanError> {
        let tree = self.dependency_mapper.dependency_tree(artifact)?;
        self.persistence.save_dependency_tree(artifact, &tree)?;
        Ok(())
    }
}
