use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::core::error::SpecmanError;
use crate::graph::tree::{ArtifactId, ArtifactKind, FilesystemDependencyMapper};
use crate::validation::references::{
    IssueSeverity, ReferenceIssueKind, ReferenceSource, ReferenceValidationIssue,
    ReferenceValidator,
};
use crate::validation::{ValidationTag, validate_compliance};
use crate::workspace::{FilesystemWorkspaceLocator, WorkspaceLocator};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceStatusConfig {
    pub structure: bool,
    pub references: bool,
    pub cycles: bool,
    pub compliance: bool,
    pub scratchpads: bool,
}

impl Default for WorkspaceStatusConfig {
    fn default() -> Self {
        Self {
            structure: true,
            references: true,
            cycles: true,
            compliance: true,
            scratchpads: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum StatusResult {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceStatusReport {
    /// Overall workspace status. Fails if any enabled check fails.
    /// [ENSURES: concept-workspace-status.aggregation:CHECK]
    pub global_status: StatusResult,

    /// Status for specification and implementation artifacts.
    pub spec_impl_status: StatusResult,

    /// Status specifically for scratch pad artifacts.
    /// [ENSURES: concept-workspace-status.scratchpads:CHECK]
    pub scratchpad_status: StatusResult,

    /// Detailed status per artifact.
    pub artifacts: BTreeMap<ArtifactId, ArtifactStatus>,

    /// Global errors that aren't tied to a specific artifact (e.g. cycles).
    pub cycle_errors: Vec<String>,

    /// Global structure errors (e.g., failure to build index).
    pub structure_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactStatus {
    /// Errors related to file structure or front matter.
    pub structure_errors: Vec<String>,

    /// Errors from reference validation.
    pub reference_errors: Vec<ReferenceValidationIssue>,

    /// Missing compliance constraints (for implementations).
    pub compliance_missing: Vec<String>,

    /// Orphaned compliance tags (for implementations).
    pub compliance_orphans: Vec<ValidationTag>,
}

impl ArtifactStatus {
    pub fn new() -> Self {
        Self {
            structure_errors: Vec::new(),
            reference_errors: Vec::new(),
            compliance_missing: Vec::new(),
            compliance_orphans: Vec::new(),
        }
    }

    pub fn is_pass(&self) -> bool {
        self.structure_errors.is_empty()
            && self
                .reference_errors
                .iter()
                .all(|e| e.severity != IssueSeverity::Error)
            && self.compliance_missing.is_empty()
    }
}

impl Default for ArtifactStatus {
    fn default() -> Self {
        Self::new()
    }
}

pub fn validate_workspace_status(
    workspace_root: std::path::PathBuf,
    config: WorkspaceStatusConfig,
) -> Result<WorkspaceStatusReport, SpecmanError> {
    let locator = Arc::new(FilesystemWorkspaceLocator::new(workspace_root.clone()));
    let mapper = FilesystemDependencyMapper::new(locator.clone());
    let workspace = locator.workspace()?;

    let mut artifacts = BTreeMap::new();
    let mut global_structure_errors = Vec::new();

    // We get the inventory. Note that we rely on the method .
    // We assume that the inventory contains all artifacts even if they have some errors
    // (though severe structure errors might exclude them,  will capture metadata errors).
    let inventory = mapper.dependency_graph().inventory_snapshot()?;

    for entry in inventory.entries.iter() {
        let id = entry.summary.id.clone();

        // Skip scratchpads if disabled
        if !config.scratchpads && id.kind == ArtifactKind::ScratchPad {
            continue;
        }

        let mut status = ArtifactStatus::new();

        // Structure Check
        if config.structure {
            if let Some(error) = entry.summary.metadata.get("metadata_status") {
                if error != "ok" {
                    status.structure_errors.push(error.clone());
                }
            }
            if let Some(version_error) = entry.summary.metadata.get("version_error") {
                status
                    .structure_errors
                    .push(format!("Invalid version: {}", version_error));
            }
            if let Some(dep_errors) = entry.summary.metadata.get("dependency_errors") {
                status
                    .structure_errors
                    .push(format!("Dependency errors: {}", dep_errors));
            }
        }

        artifacts.insert(id, status);
    }

    // Reference Check
    if config.references {
        let validator = ReferenceValidator::new(&workspace);

        for (id, status) in artifacts.iter_mut() {
            if let Some(entry) = inventory.entries.iter().find(|e| e.summary.id == *id) {
                if let Some(path_str) = &entry.summary.resolved_path {
                    match validator.validate(path_str) {
                        Ok(report) => {
                            status.reference_errors.extend(report.issues);
                        }
                        Err(e) => {
                            status.reference_errors.push(ReferenceValidationIssue {
                                kind: ReferenceIssueKind::Unknown,
                                severity: IssueSeverity::Error,
                                message: e.to_string(),
                                source: ReferenceSource {
                                    document: path_str.clone(),
                                    range: None,
                                },
                                destination: None,
                            });
                        }
                    }
                }
            }
        }
    }

    // Cycle Check
    let mut cycle_errors = Vec::new();
    if config.cycles {
        match mapper.dependency_graph().detect_cycles() {
            Ok(cycles) => cycle_errors = cycles,
            Err(e) => global_structure_errors.push(format!("Cycle detection failed: {}", e)),
        }
    }

    // Compliance Check
    if config.compliance {
        for (id, status) in artifacts.iter_mut() {
            if id.kind == ArtifactKind::Implementation {
                match validate_compliance(&workspace_root, id) {
                    Ok(report) => {
                        status.compliance_missing.extend(report.missing);
                        status.compliance_orphans.extend(report.orphans);
                    }
                    Err(e) => {
                        status
                            .compliance_missing
                            .push(format!("Compliance check failed: {}", e));
                    }
                }
            }
        }
    }

    // Aggregate Status
    let spec_impl_program_pass = artifacts
        .iter()
        .filter(|(id, _)| id.kind != ArtifactKind::ScratchPad)
        .all(|(_, status)| status.is_pass())
        && cycle_errors.is_empty()
        && global_structure_errors.is_empty();

    let scratchpad_pass = if config.scratchpads {
        artifacts
            .iter()
            .filter(|(id, _)| id.kind == ArtifactKind::ScratchPad)
            .all(|(_, status)| status.is_pass())
    } else {
        true
    };

    Ok(WorkspaceStatusReport {
        global_status: if spec_impl_program_pass && scratchpad_pass {
            StatusResult::Pass
        } else {
            StatusResult::Fail
        },
        spec_impl_status: if spec_impl_program_pass {
            StatusResult::Pass
        } else {
            StatusResult::Fail
        },
        scratchpad_status: if scratchpad_pass {
            StatusResult::Pass
        } else {
            StatusResult::Fail
        },
        artifacts,
        cycle_errors,
        structure_errors: global_structure_errors,
    })
}
