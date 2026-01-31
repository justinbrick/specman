use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use clap::Command;
use serde::Serialize;
use specman::{
    ArtifactId, ArtifactKind,
    HttpsMethod, HttpsValidationMode, HttpsValidationOptions, IssueSeverity,
    ReferenceValidationOptions, ReferenceValidationStatus, TransitiveOptions,
    validate_references,
};

use crate::commands::CommandResult;
use crate::context::CliSession;
use crate::error::CliError;
use crate::util;

#[derive(Clone, Debug, Serialize)]
pub struct StatusReport {
    pub name: String,
    pub kind: String,
    pub path: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<StatusError>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StatusError {
    pub severity: String,
    pub message: String,
    pub location: Option<String>,
}

pub fn command() -> Command {
    Command::new("status")
        .about("Validate specifications and implementations")
        .arg(
            clap::Arg::new("local")
                .long("local")
                .action(clap::ArgAction::SetTrue)
                .help("Disable network reachability checks during validation"),
        )
}

pub fn run(session: &CliSession, matches: &clap::ArgMatches) -> Result<CommandResult, CliError> {
    let mut reports = Vec::new();
    let mut healthy = true;

    let local = matches.get_flag("local");
    let options = ReferenceValidationOptions {
        https: HttpsValidationOptions {
            mode: if local {
                HttpsValidationMode::SyntaxOnly
            } else {
                HttpsValidationMode::Reachability {
                    timeout: Duration::from_secs(30),
                    max_redirects: 10,
                    method: HttpsMethod::Head,
                }
            },
        },
        transitive: TransitiveOptions::default(),
    };

    for artifact in collect_artifacts(session) {
        let path = artifact_path(&session.workspace_paths, &artifact);
        let mut errors = Vec::new();

        match session.env.mapping.dependency_tree(&artifact) {
            Ok(_) => {}
            Err(err) => {
                errors.push(StatusError {
                    severity: "error".into(),
                    message: err.to_string(),
                    location: None,
                });
            }
        }

        match validate_references(
            &path,
            &session.workspace_paths,
            options.clone(),
        ) {
            Ok(report) => {
                for issue in report.errors {
                    if issue.severity == IssueSeverity::Error {
                        errors.push(StatusError {
                            severity: "error".into(),
                            message: issue.message,
                            location: Some(format!(
                                "{}:{}",
                                issue.source.document,
                                issue
                                    .source
                                    .range
                                    .map(|r| r.start.line.to_string())
                                    .unwrap_or_else(|| "?".into())
                            )),
                        });
                    }
                }
                if report.status == ReferenceValidationStatus::Failure && errors.is_empty() {
                    // Fallback if status is failure but no errors were pushed (unlikely)
                    errors.push(StatusError {
                        severity: "error".into(),
                        message: "Reference validation failure (unknown cause)".into(),
                        location: None,
                    });
                }
            }
            Err(err) => {
                errors.push(StatusError {
                    severity: "error".into(),
                    message: format!("Reference validation execution failed: {err}"),
                    location: None,
                });
            }
        }

        let ok = errors.is_empty();
        if !ok {
            healthy = false;
        }

        reports.push(StatusReport {
            name: artifact.name.clone(),
            kind: artifact_kind(&artifact),
            path,
            ok,
            message: if ok {
                None
            } else {
                Some("Validation failed".into())
            },
            errors,
        });
    }
    reports.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(CommandResult::Status { reports, healthy })
}

fn collect_artifacts(session: &CliSession) -> Vec<ArtifactId> {
    let mut artifacts = Vec::new();
    artifacts.extend(read_dir_artifacts(
        session.workspace_paths.spec_dir(),
        ArtifactKind::Specification,
    ));
    artifacts.extend(read_dir_artifacts(
        session.workspace_paths.impl_dir(),
        ArtifactKind::Implementation,
    ));
    artifacts.extend(read_dir_artifacts(
        session.workspace_paths.scratchpad_dir(),
        ArtifactKind::ScratchPad,
    ));
    artifacts
}

fn read_dir_artifacts(dir: PathBuf, kind: ArtifactKind) -> Vec<ArtifactId> {
    let mut result = Vec::new();
    if dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    if let Some(name) = entry.file_name().to_str() {
                        result.push(ArtifactId {
                            kind,
                            name: name.to_string(),
                        });
                    }
                }
            }
        }
    }
    result
}

fn artifact_path(paths: &specman::WorkspacePaths, artifact: &ArtifactId) -> String {
    let rel = match artifact.kind {
        ArtifactKind::Specification => paths.spec_dir().join(&artifact.name).join("spec.md"),
        ArtifactKind::Implementation => paths.impl_dir().join(&artifact.name).join("impl.md"),
        ArtifactKind::ScratchPad => paths
            .scratchpad_dir()
            .join(&artifact.name)
            .join("scratch.md"),
    };
    util::workspace_relative(paths.root(), &rel)
}

fn artifact_kind(artifact: &ArtifactId) -> String {
    match artifact.kind {
        ArtifactKind::Specification => "spec".into(),
        ArtifactKind::Implementation => "impl".into(),
        ArtifactKind::ScratchPad => "scratch".into(),
    }
}
