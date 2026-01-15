use std::path::Path;

use specman::dependency_tree::ArtifactId;
use specman::front_matter::ScratchWorkType;
use specman::metadata::{FrontMatterUpdateOp, FrontMatterUpdateRequest, apply_front_matter_update};
use specman::workspace::WorkspacePaths;

use crate::error::{CliError, ExitStatus};

pub fn update_spec_document(
    content: &str,
    artifact: &ArtifactId,
    artifact_path: &Path,
    workspace: &WorkspacePaths,
    name: &str,
    version: &str,
    dependencies: &[String],
) -> Result<String, CliError> {
    let mut request = FrontMatterUpdateRequest::new()
        .persist(false)
        .with_op(FrontMatterUpdateOp::SetName {
            name: name.to_string(),
        })
        .with_op(FrontMatterUpdateOp::SetVersion {
            version: version.to_string(),
        });

    for dep in dependencies {
        request = request.with_op(FrontMatterUpdateOp::AddDependency {
            ref_: dep.clone(),
            optional: Some(false),
        });
    }

    let (updated, _) =
        apply_front_matter_update(artifact, artifact_path, workspace, content, &request)
            .map_err(|err| CliError::new(err.to_string(), ExitStatus::Config))?;
    Ok(updated)
}

pub fn update_impl_document(
    content: &str,
    artifact: &ArtifactId,
    artifact_path: &Path,
    workspace: &WorkspacePaths,
    name: &str,
    spec_locator: &str,
    location: &str,
) -> Result<String, CliError> {
    let request = FrontMatterUpdateRequest::new()
        .persist(false)
        .with_op(FrontMatterUpdateOp::SetName {
            name: name.to_string(),
        })
        .with_op(FrontMatterUpdateOp::SetSpec {
            ref_: spec_locator.to_string(),
        })
        .with_op(FrontMatterUpdateOp::SetLocation {
            location: location.to_string(),
        })
        .with_op(FrontMatterUpdateOp::AddReference {
            ref_: spec_locator.to_string(),
            type_: Some("specification".to_string()),
            optional: Some(false),
        });

    let (updated, _) =
        apply_front_matter_update(artifact, artifact_path, workspace, content, &request)
            .map_err(|err| CliError::new(err.to_string(), ExitStatus::Config))?;
    Ok(updated)
}

pub fn update_scratch_document(
    content: &str,
    artifact: &ArtifactId,
    artifact_path: &Path,
    workspace: &WorkspacePaths,
    branch: &str,
    work_type: &ScratchWorkType,
) -> Result<String, CliError> {
    let request = FrontMatterUpdateRequest::new()
        .persist(false)
        .with_op(FrontMatterUpdateOp::SetBranch {
            branch: branch.to_string(),
        })
        .with_op(FrontMatterUpdateOp::SetWorkType {
            work_type: work_type.clone(),
        });

    let (updated, _) =
        apply_front_matter_update(artifact, artifact_path, workspace, content, &request)
            .map_err(|err| CliError::new(err.to_string(), ExitStatus::Config))?;
    Ok(updated)
}
