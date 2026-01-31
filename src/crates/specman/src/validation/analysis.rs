use crate::graph::tree::{ArtifactId, DependencyTree};
use crate::core::env::SpecmanEnv;
use crate::core::error::SpecmanError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeletionImpact {
    pub dependencies: DependencyTree,
    pub blocked: bool,
}

pub fn check_deletion_impact(
    env: &SpecmanEnv,
    target: &ArtifactId,
) -> Result<DeletionImpact, SpecmanError> {
    let dependencies = env.mapping.dependency_tree(target)?;
    let blocked = dependencies.has_blocking_dependents();
    Ok(DeletionImpact {
        dependencies,
        blocked,
    })
}
