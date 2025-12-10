use serde_json::Error as SerdeError;
use specman::SpecmanError;
use std::fmt;
use thiserror::Error;

/// Convenient alias for adapter results.
pub type Result<T> = std::result::Result<T, SpecmanMcpError>;

/// Error type encompassing workspace discovery, SpecMan library, and adapter-specific failures.
#[derive(Debug, Error)]
pub enum SpecmanMcpError {
    /// Workspace discovery or guard violation failures.
    #[error("workspace error: {0}")]
    Workspace(String),
    /// Semantic issues with resource handles or MCP surface expectations.
    #[error("resource error: {0}")]
    Resource(String),
    /// Filesystem or IO-related errors surfaced while reading artifacts.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Errors bubbled up from the core SpecMan library.
    #[error(transparent)]
    Specman(#[from] SpecmanError),
    /// Serialization/deserialization errors when writing telemetry envelopes.
    #[error("serialization error: {0}")]
    Serialization(#[from] SerdeError),
}

impl SpecmanMcpError {
    /// Creates a workspace-related error with the provided message.
    pub fn workspace(message: impl Into<String>) -> Self {
        Self::Workspace(message.into())
    }

    /// Creates a resource-related error with the provided message.
    pub fn resource(message: impl Into<String>) -> Self {
        Self::Resource(message.into())
    }

    /// Helper used when a handle fails validation.
    pub fn invalid_handle(handle: impl fmt::Display) -> Self {
        Self::Resource(format!("unsupported resource handle: {handle}"))
    }
}
