use std::path::PathBuf;

use specman::SpecmanEnv;
use specman::workspace::{FilesystemWorkspaceLocator, WorkspaceLocator, WorkspacePaths};
use specman::{DataModelAdapter, InMemoryAdapter};
use std::sync::Arc;

use crate::error::CliError;
use crate::templates::TemplateCatalog as CliTemplateCatalog;
use crate::util::Verbosity;

/// Aggregates the workspace context, adapters, and shared services required by the
/// SpecMan CLI. This keeps Workspace Context Resolution deterministic and ensures
/// every command reuses the same dependency mapper, template engine, and lifecycle
/// controller for the duration of a single invocation (see
/// spec/specman-cli/spec.md#concept-workspace-context-resolution).
pub struct CliSession {
    pub workspace_paths: WorkspacePaths,
    pub templates: CliTemplateCatalog,
    pub env: Arc<SpecmanEnv>,
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
        
        let data_adapter: Arc<dyn DataModelAdapter> = Arc::new(InMemoryAdapter::new());
        let env = SpecmanEnv::new(workspace_locator, Some(data_adapter))?;
        
        let templates = CliTemplateCatalog::new(workspace_paths.clone());

        Ok(Self {
            workspace_paths,
            templates,
            env: Arc::new(env),
            verbosity,
        })
    }
}
