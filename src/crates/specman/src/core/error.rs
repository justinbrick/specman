use std::fmt;
use std::path::PathBuf;

use thiserror::Error;

use crate::graph::tree::ArtifactId;

/// Structured lifecycle failures surfaced by higher-level lifecycle orchestration.
#[derive(Debug, Error)]
pub enum LifecycleError {
    #[error("deletion blocked for {target}")]
    DeletionBlocked { target: ArtifactId },
    #[error("deletion plan target mismatch: request={requested}, plan={planned}")]
    PlanTargetMismatch {
        requested: ArtifactId,
        planned: ArtifactId,
    },
    #[error("{context}: {source}")]
    Context {
        context: String,
        #[source]
        source: Box<LifecycleError>,
    },
}

/// High-level error type shared across SpecMan components.
#[derive(Debug, Error)]
pub enum SpecmanError {
    #[error("template error: {0}")]
    Template(String),
    #[error("unknown work type: {0}")]
    UnknownWorkType(String),
    #[error("dependency error: {0}")]
    Dependency(String),
    #[error("missing target: {0}")]
    MissingTarget(PathBuf),
    #[error(transparent)]
    Lifecycle(#[from] LifecycleError),
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
            SpecmanError::UnknownWorkType(kind) => {
                SpecmanError::UnknownWorkType(format!("{ctx}: {kind}"))
            }
            SpecmanError::Dependency(msg) => SpecmanError::Dependency(format!("{ctx}: {msg}")),
            SpecmanError::MissingTarget(path) => SpecmanError::MissingTarget(path),
            SpecmanError::Lifecycle(err) => SpecmanError::Lifecycle(LifecycleError::Context {
                context: ctx.to_string(),
                source: Box::new(err),
            }),
            SpecmanError::Workspace(msg) => SpecmanError::Workspace(format!("{ctx}: {msg}")),
            SpecmanError::Serialization(msg) => {
                SpecmanError::Serialization(format!("{ctx}: {msg}"))
            }
            SpecmanError::Io(err) => SpecmanError::Io(err),
        }
    }
}
