use crate::error::Result;
use crate::resources::{
    ResourceCatalog, ResourceDescriptor, ResourceRead, WorkspaceDescription,
};
use crate::session::{SessionManager, WorkspaceSessionGuard};
use crate::telemetry::OperationEnvelopeSink;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specman::workspace::WorkspaceLocator;
use std::path::PathBuf;
use std::sync::Arc;

/// Protocol tag advertised during MCP negotiation.
pub const PROTOCOL_VERSION_TAG: &str = "2025-11-25";

/// High-level server object used by the rmcp transport layer.
pub struct SpecmanMcpServer<L>
where
    L: WorkspaceLocator + Clone,
{
    catalog: ResourceCatalog<L>,
    sessions: Arc<SessionManager>,
    telemetry: Arc<OperationEnvelopeSink>,
    protocol_version: String,
}

impl<L> SpecmanMcpServer<L>
where
    L: WorkspaceLocator + Clone,
{
    /// Creates a new server with the provided workspace locator and telemetry sink.
    pub fn new(locator: L, telemetry: OperationEnvelopeSink) -> Result<Self> {
        Ok(Self {
            catalog: ResourceCatalog::new(locator),
            sessions: Arc::new(SessionManager::default()),
            telemetry: Arc::new(telemetry),
            protocol_version: PROTOCOL_VERSION_TAG.to_string(),
        })
    }

    /// Returns the supported protocol tag.
    pub fn protocol_version(&self) -> &str {
        &self.protocol_version
    }

    /// Returns the canonical workspace root path.
    pub fn workspace_root(&self) -> Result<PathBuf> {
        self.catalog.workspace_root()
    }

    /// Describes the workspace for clients.
    pub fn describe_workspace(&self) -> Result<WorkspaceDescription> {
        self.catalog.describe_workspace()
    }

    /// Lists available resources.
    pub fn list_resources(&self) -> Result<Vec<ResourceDescriptor>> {
        self.catalog.list()
    }

    /// Reads a resource handle.
    pub fn read_resource(&self, reference: &str) -> Result<ResourceRead> {
        self.catalog.read(reference)
    }

    /// Provides a guard for workspace normalization rules.
    pub fn guard(&self) -> Result<WorkspaceSessionGuard> {
        self.catalog.guard()
    }

    /// Returns shared session state for transport wiring.
    pub fn session_manager(&self) -> Arc<SessionManager> {
        self.sessions.clone()
    }

    /// Returns the telemetry sink.
    pub fn telemetry_sink(&self) -> Arc<OperationEnvelopeSink> {
        self.telemetry.clone()
    }
}

/// Minimal capability descriptor stub until rmcp wiring is ready.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityDescriptor {
    pub identifier: String,
    pub concept_ref: String,
    pub min_version: String,
    pub max_version: Option<String>,
}