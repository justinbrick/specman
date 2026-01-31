use crate::graph::tree::{ArtifactId, DependencyTree};
use crate::core::env::SpecmanEnv;
use crate::core::error::{LifecycleError, SpecmanError};
use crate::storage::persistence::{ArtifactRemovalStore, RemovedArtifact};

#[derive(Clone, Debug, Default)]
pub struct DeleteOptions {
    pub force: bool,
    pub dry_run: bool,
}

#[derive(Clone, Debug)]
pub enum DeleteResult {
    Removed(RemovedArtifact),
    DryRun(DeletePlan),
}

#[derive(Clone, Debug)]
pub struct DeletePlan {
    pub dependencies: DependencyTree,
    pub blocked: bool,
}

pub fn delete_artifact(
    env: &SpecmanEnv,
    target: &ArtifactId,
    opts: DeleteOptions,
) -> Result<DeleteResult, SpecmanError> {
    let dependencies = env.mapping.dependency_tree(target)?;
    let blocked = dependencies.has_blocking_dependents();

    if opts.dry_run {
        return Ok(DeleteResult::DryRun(DeletePlan {
            dependencies,
            blocked,
        }));
    }

    if blocked && !opts.force {
        return Err(LifecycleError::DeletionBlocked { target: target.clone() }.into());
    }

    let removed = env.persistence.remove_artifact(target)?;

    Ok(DeleteResult::Removed(removed))
}
