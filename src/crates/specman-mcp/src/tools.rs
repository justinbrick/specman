use std::path::PathBuf;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::schemars::JsonSchema;
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};

use specman::{
    ArtifactId, ArtifactKind, CreateRequest, DefaultLifecycleController,
    FilesystemDependencyMapper, FilesystemWorkspaceLocator, MarkdownTemplateEngine,
    PersistedArtifact, Specman, TemplateCatalog, WorkspaceLocator, WorkspacePersistence,
};

use crate::error::{McpError, invalid_params, to_mcp_error};
use crate::resources::{artifact_handle, resolved_path_or_artifact_path, workspace_relative_path};
use crate::server::SpecmanMcpServer;

pub(crate) fn build_tool_router() -> ToolRouter<SpecmanMcpServer> {
    SpecmanMcpServer::tool_router()
}

/// Structured workspace data exposed over MCP tools.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceInfo {
    pub root: String,
    pub dot_specman: String,
    pub spec_dir: String,
    pub impl_dir: String,
    pub scratchpad_dir: String,
}

/// Deterministic result payload returned by the `create_artifact` MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateArtifactResult {
    pub id: ArtifactId,
    pub handle: String,
    /// Canonical workspace-relative path to the created artifact.
    pub path: String,
}

#[tool_router]
impl SpecmanMcpServer {
    #[tool(
        name = "create_artifact",
        description = "Create a SpecMan artifact (spec, impl, or scratch pad) from a core CreateRequest"
    )]
    pub(crate) async fn create_artifact(
        &self,
        Parameters(request): Parameters<CreateRequest>,
    ) -> Result<Json<CreateArtifactResult>, McpError> {
        let normalized = self.normalize_create_request(request)?;
        let specman = self.build_specman()?;
        let persisted = specman.create(normalized).map_err(to_mcp_error)?;

        Ok(Json(create_artifact_result(&persisted)))
    }
}

fn create_artifact_result(persisted: &PersistedArtifact) -> CreateArtifactResult {
    let relative = workspace_relative_path(persisted.workspace.root(), &persisted.path)
        .unwrap_or_else(|| persisted.path.display().to_string());
    let handle = match persisted.artifact.kind {
        ArtifactKind::Specification => format!("spec://{}", persisted.artifact.name),
        ArtifactKind::Implementation => format!("impl://{}", persisted.artifact.name),
        ArtifactKind::ScratchPad => format!("scratch://{}", persisted.artifact.name),
    };
    CreateArtifactResult {
        id: persisted.artifact.clone(),
        handle,
        path: relative,
    }
}

impl SpecmanMcpServer {
    fn build_specman(
        &self,
    ) -> Result<
        Specman<
            FilesystemDependencyMapper<std::sync::Arc<FilesystemWorkspaceLocator>>,
            MarkdownTemplateEngine,
            std::sync::Arc<FilesystemWorkspaceLocator>,
        >,
        McpError,
    > {
        let locator = self.workspace.clone();
        let workspace = locator.workspace().map_err(to_mcp_error)?;

        let mapper = FilesystemDependencyMapper::new(locator.clone());
        let inventory = mapper.inventory_handle();
        let templates = MarkdownTemplateEngine::new();
        let controller = DefaultLifecycleController::new(mapper, templates);
        let catalog = TemplateCatalog::new(workspace);
        let persistence = WorkspacePersistence::with_inventory(locator, inventory);

        Ok(Specman::new(controller, catalog, persistence))
    }

    fn normalize_locator_to_workspace_path(&self, locator: &str) -> Result<String, McpError> {
        let trimmed = locator.trim();
        if trimmed.is_empty() {
            return Err(invalid_params("locator must not be empty"));
        }

        if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
            return Err(invalid_params(
                "workspace target locators must not be URLs; use spec://, impl://, scratch://, or a workspace-relative path",
            ));
        }

        let workspace = self.workspace.workspace().map_err(to_mcp_error)?;
        let tree = self
            .dependency_mapper
            .dependency_tree_from_locator(trimmed)
            .map_err(to_mcp_error)?;

        let resolved = resolved_path_or_artifact_path(&tree.root, &workspace);
        let absolute = PathBuf::from(resolved);
        workspace_relative_path(workspace.root(), &absolute)
            .ok_or_else(|| invalid_params("locator must resolve within the workspace"))
    }

    fn normalize_locator_to_handle(
        &self,
        locator: &str,
    ) -> Result<specman::ArtifactSummary, McpError> {
        let trimmed = locator.trim();
        if trimmed.is_empty() {
            return Err(invalid_params("locator must not be empty"));
        }

        if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
            return Err(invalid_params(
                "workspace target locators must not be URLs; use spec://, impl://, scratch://, or a workspace-relative path",
            ));
        }

        let tree = self
            .dependency_mapper
            .dependency_tree_from_locator(trimmed)
            .map_err(to_mcp_error)?;
        Ok(tree.root)
    }

    fn normalize_create_request(&self, request: CreateRequest) -> Result<CreateRequest, McpError> {
        match request {
            CreateRequest::Custom { .. } => Err(invalid_params(
                "CreateRequest::Custom is not supported via MCP; use Specification, Implementation, or ScratchPad",
            )),
            CreateRequest::Implementation { context } => {
                let target_summary = self.normalize_locator_to_handle(&context.target)?;
                if target_summary.id.kind != ArtifactKind::Specification {
                    return Err(invalid_params(
                        "implementation targets must resolve to a specification (spec://... or a spec path)",
                    ));
                }
                let target = artifact_handle(&target_summary);
                Ok(CreateRequest::Implementation {
                    context: specman::ImplContext {
                        name: context.name.trim().to_string(),
                        target,
                    },
                })
            }
            CreateRequest::ScratchPad { context } => {
                let target = self.normalize_locator_to_workspace_path(&context.target)?;
                Ok(CreateRequest::ScratchPad {
                    context: specman::ScratchPadCreateContext {
                        name: context.name.trim().to_string(),
                        target,
                        work_type: context.work_type,
                    },
                })
            }
            CreateRequest::Specification { context } => Ok(CreateRequest::Specification {
                context: specman::SpecContext {
                    name: context.name.trim().to_string(),
                    title: context.title.trim().to_string(),
                },
            }),
        }
    }
}
