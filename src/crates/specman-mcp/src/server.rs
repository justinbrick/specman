use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::{router::Router, router::prompt::PromptRouter, tool::ToolRouter};
use rmcp::service::ServerInitializeError;
use rmcp::{service::ServiceExt, transport};

use specman::{FilesystemDependencyMapper, FilesystemWorkspaceLocator, SpecmanError};

use crate::error::to_mcp_error;
use crate::prompts::build_prompt_router;
use crate::tools::build_tool_router;

#[derive(Clone)]
pub struct SpecmanMcpServer {
    pub(crate) workspace: Arc<FilesystemWorkspaceLocator>,
    pub(crate) dependency_mapper: Arc<FilesystemDependencyMapper<Arc<FilesystemWorkspaceLocator>>>,
    pub(crate) tool_router: ToolRouter<Self>,
    pub(crate) prompt_router: PromptRouter<Self>,
}

impl SpecmanMcpServer {
    pub fn new() -> Result<Self, SpecmanError> {
        let cwd = std::env::current_dir()?;
        Self::new_with_root(cwd)
    }

    pub fn new_with_root(root: impl Into<PathBuf>) -> Result<Self, SpecmanError> {
        let workspace = Arc::new(FilesystemWorkspaceLocator::new(root));
        let dependency_mapper = Arc::new(FilesystemDependencyMapper::new(workspace.clone()));

        Ok(Self {
            workspace,
            dependency_mapper,
            tool_router: build_tool_router(),
            prompt_router: build_prompt_router(),
        })
    }

    /// Start a stdio-based MCP server and wait until the transport closes.
    pub async fn run_stdio(self) -> Result<(), ServerInitializeError> {
        let tools = self.tool_router.clone();
        let prompts = self.prompt_router.clone();
        let router = Router::new(self).with_tools(tools).with_prompts(prompts);
        let service = router.serve(transport::io::stdio()).await?;

        // Hold the service open until the peer closes the transport.
        let _ = service.waiting().await;
        Ok(())
    }
}

/// Convenience entry point that builds the server and runs it over stdio.
/// Accepts an optional workspace root; when `None`, the current working directory is used.
pub async fn run_stdio_server_with_root(
    workspace_root: Option<PathBuf>,
) -> Result<(), ServerInitializeError> {
    let server = match workspace_root {
        Some(root) => SpecmanMcpServer::new_with_root(root),
        None => SpecmanMcpServer::new(),
    }
    .map_err(|err| ServerInitializeError::InitializeFailed(to_mcp_error(err)))?;
    server.run_stdio().await
}

/// Convenience entry point that defaults to the current working directory.
pub async fn run_stdio_server() -> Result<(), ServerInitializeError> {
    run_stdio_server_with_root(None).await
}
