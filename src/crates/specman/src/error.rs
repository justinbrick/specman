use std::fmt;

use thiserror::Error;

/// High-level error type shared across SpecMan components.
#[derive(Debug, Error)]
pub enum SpecmanError {
    #[error("template error: {0}")]
    Template(String),
    #[error("dependency error: {0}")]
    Dependency(String),
    #[error("workspace error: {0}")]
    Workspace(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<serde_json::Error> for SpecmanError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl SpecmanError {
    pub fn context<T: fmt::Display>(self, ctx: T) -> Self {
        match self {
            SpecmanError::Template(msg) => SpecmanError::Template(format!("{ctx}: {msg}")),
            SpecmanError::Dependency(msg) => SpecmanError::Dependency(format!("{ctx}: {msg}")),
            SpecmanError::Workspace(msg) => SpecmanError::Workspace(format!("{ctx}: {msg}")),
            SpecmanError::Serialization(msg) => {
                SpecmanError::Serialization(format!("{ctx}: {msg}"))
            }
            SpecmanError::Io(err) => SpecmanError::Io(err),
        }
    }
}
