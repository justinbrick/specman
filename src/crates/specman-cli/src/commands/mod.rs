use serde::Serialize;

use crate::error::ExitStatus;

pub mod implementation;
pub mod scratch;
pub mod spec;
pub mod status;

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandResult {
    Status {
        reports: Vec<status::StatusReport>,
        healthy: bool,
    },
    SpecList {
        specs: Vec<spec::SpecSummary>,
    },
    SpecCreated {
        summary: spec::SpecSummary,
        path: String,
    },
    SpecDeleted {
        summary: spec::SpecSummary,
        forced: bool,
        tree: spec::DeletionTree,
        removed_path: String,
    },
    ImplList {
        implementations: Vec<implementation::ImplSummary>,
    },
    ImplCreated {
        summary: implementation::ImplSummary,
        path: String,
    },
    ImplDeleted {
        summary: implementation::ImplSummary,
        forced: bool,
        tree: implementation::DeletionTree,
        removed_path: String,
    },
    ScratchList {
        pads: Vec<scratch::ScratchSummary>,
        missing_metadata: bool,
    },
    ScratchCreated {
        summary: scratch::ScratchSummary,
        path: String,
    },
    ScratchDeleted {
        summary: scratch::ScratchSummary,
        forced: bool,
        tree: scratch::DeletionTree,
        removed_path: String,
    },
}

impl CommandResult {
    pub fn exit_status(&self) -> ExitStatus {
        match self {
            CommandResult::Status { healthy, .. } => {
                if *healthy {
                    ExitStatus::Ok
                } else {
                    ExitStatus::Data
                }
            }
            CommandResult::ScratchList {
                missing_metadata, ..
            } => {
                if *missing_metadata {
                    ExitStatus::Data
                } else {
                    ExitStatus::Ok
                }
            }
            _ => ExitStatus::Ok,
        }
    }
}
