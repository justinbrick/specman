//! SpecMan MCP adapter entrypoint.

mod error;
mod handle;
mod resources;
mod server;
mod session;
mod telemetry;

pub use crate::error::{Result, SpecmanMcpError};
pub use crate::handle::{McpResourceHandle, ResourceTarget};
pub use crate::resources::{
    ResourceCatalog, ResourceDescriptor, ResourceRead, ResourceVariant, WorkspaceArtifactCounts,
    WorkspaceDescription,
};
pub use crate::server::{PROTOCOL_VERSION_TAG, SpecmanMcpServer};
pub use crate::session::{SessionManager, WorkspaceSessionGuard};
pub use crate::telemetry::{OperationEnvelope, OperationEnvelopeSink, OperationStatus};

use specman::workspace::{FilesystemWorkspaceLocator, WorkspaceLocator};
use std::{path::PathBuf, sync::Arc};

/// Default relative path for the telemetry log.
const DEFAULT_TELEMETRY_LOG: &str = ".specman/logs/specman-mcp.jsonl";

/// Boots the MCP server using STDIN/STDOUT transport.
pub fn bootstrap_stdio() -> Result<()> {
    tracing_subscriber::fmt::try_init().ok();

    let locator = Arc::new(FilesystemWorkspaceLocator::from_current_dir()?);
    let workspace = locator.workspace()?;
    let log_path = ensure_telemetry_path(workspace.root())?;
    let telemetry = OperationEnvelopeSink::new(log_path)?;
    let server = SpecmanMcpServer::new(locator.clone(), telemetry)?;
    let workspace_root = server.workspace_root()?;

    // TODO: integrate rmcp transport wiring once rmcp APIs are available in this crate.
    tracing::info!(
        root = %workspace_root.display(),
        "specman-mcp bootstrap complete (transport wiring pending)"
    );

    Ok(())
}

fn ensure_telemetry_path(root: &std::path::Path) -> Result<PathBuf> {
    let path = root.join(DEFAULT_TELEMETRY_LOG);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(path)
}
