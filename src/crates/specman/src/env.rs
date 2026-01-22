use crate::adapter::DataModelAdapter;
use crate::dependency_tree::{DependencyMapping, FilesystemDependencyMapper};
use crate::error::SpecmanError;
use crate::persistence::WorkspacePersistence;
use crate::template::{MarkdownTemplateEngine, TemplateEngine};
use crate::template_catalog::TemplateCatalog;
use crate::workspace::{FilesystemWorkspaceLocator, WorkspaceLocator};
use std::path::Path;
use std::sync::Arc;

pub type DefaultWorkspaceLocator = Arc<FilesystemWorkspaceLocator>;
pub type DefaultPersistence = WorkspacePersistence<DefaultWorkspaceLocator>;

/// Shared environment components for Specman operations.
/// Holds the necessary state and services to perform artifact management.
pub struct SpecmanEnv {
    pub catalog: TemplateCatalog,
    pub persistence: DefaultPersistence,
    pub mapping: Arc<dyn DependencyMapping>,
    pub templates: Arc<dyn TemplateEngine>,
}

impl SpecmanEnv {
    /// Initialize the environment from the current working directory.
    pub fn from_current_dir() -> Result<Self, SpecmanError> {
        let locator = Arc::new(FilesystemWorkspaceLocator::from_current_dir()?);
        Self::new(locator, None)
    }

    /// Initialize the environment from a specific path.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, SpecmanError> {
        let locator = Arc::new(FilesystemWorkspaceLocator::new(path.as_ref().to_path_buf()));
        Self::new(locator, None)
    }

    /// Create a new environment with explicit locator and optional data adapter.
    pub fn new(
        locator: DefaultWorkspaceLocator,
        adapter: Option<Arc<dyn DataModelAdapter>>,
    ) -> Result<Self, SpecmanError> {
        let workspace = locator.workspace()?;

        let mapper = FilesystemDependencyMapper::new(locator.clone());
        let inventory = mapper.inventory_handle();

        let catalog = TemplateCatalog::new(workspace);
        let persistence = if let Some(a) = adapter {
            WorkspacePersistence::with_inventory_and_adapter(locator.clone(), inventory, a)
        } else {
            WorkspacePersistence::with_inventory(locator.clone(), inventory)
        };
        let templates = MarkdownTemplateEngine::new();

        Ok(Self {
            catalog,
            persistence,
            mapping: Arc::new(mapper),
            templates: Arc::new(templates),
        })
    }
}
