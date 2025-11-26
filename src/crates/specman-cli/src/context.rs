use std::path::PathBuf;

use specman::InMemoryAdapter;
use specman::dependency_tree::FilesystemDependencyMapper;
use specman::lifecycle::DefaultLifecycleController;
use specman::persistence::WorkspacePersistence;
use specman::template::MarkdownTemplateEngine;
use specman::workspace::{FilesystemWorkspaceLocator, WorkspaceLocator, WorkspacePaths};
use std::sync::Arc;

use crate::error::CliError;
use crate::templates::TemplateCatalog;
use crate::util::Verbosity;

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
            Arc<InMemoryAdapter>,
        >,
    >, // Centralized lifecycle guard rails shared across commands.
    pub verbosity: Verbosity,
}

impl CliSession {
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
        let persistence = Arc::new(WorkspacePersistence::new(workspace_locator.clone()));
        let template_engine = Arc::new(MarkdownTemplateEngine::default());
        let templates = TemplateCatalog::new(workspace_paths.clone());
        let data_adapter = Arc::new(InMemoryAdapter::new());
        let lifecycle = Arc::new(DefaultLifecycleController::new(
            dependency_mapper.clone(),
            template_engine.clone(),
            data_adapter.clone(),
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
}
