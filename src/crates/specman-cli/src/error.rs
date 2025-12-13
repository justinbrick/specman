use std::fmt;
use std::process::ExitCode;

use clap::error::ErrorKind as ClapErrorKind;
use specman::SpecmanError;
use specman::error::LifecycleError;

const EX_OK: u8 = 0;
const EX_USAGE: u8 = 64;
const EX_DATAERR: u8 = 65;
const EX_SOFTWARE: u8 = 70;
const EX_OSERR: u8 = 71;
const EX_CONFIG: u8 = 78;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitStatus {
    Ok,
    Usage,
    Data,
    Io,
    Config,
    Software,
}

impl ExitStatus {
    pub fn code(self) -> u8 {
        match self {
            ExitStatus::Ok => EX_OK,
            ExitStatus::Usage => EX_USAGE,
            ExitStatus::Data => EX_DATAERR,
            ExitStatus::Io => EX_OSERR,
            ExitStatus::Config => EX_CONFIG,
            ExitStatus::Software => EX_SOFTWARE,
        }
    }
}

#[derive(Debug)]
pub struct CliError {
    message: String,
    status: ExitStatus,
}

impl CliError {
    pub fn new(message: impl Into<String>, status: ExitStatus) -> Self {
        Self {
            message: message.into(),
            status,
        }
    }

    pub fn exit_code(&self) -> ExitCode {
        ExitCode::from(self.status.code())
    }

    pub fn print(&self) {
        if !self.message.is_empty() {
            eprintln!("{}", self.message);
        }
    }
}

impl From<SpecmanError> for CliError {
    fn from(err: SpecmanError) -> Self {
        let status = match &err {
            SpecmanError::Template(_)
            | SpecmanError::Dependency(_)
            | SpecmanError::MissingTarget(_) => ExitStatus::Data,
            SpecmanError::Lifecycle(err) => match err {
                LifecycleError::DeletionBlocked { .. } => ExitStatus::Data,
                LifecycleError::PlanTargetMismatch { .. } => ExitStatus::Software,
                LifecycleError::Context { source, .. } => match source.as_ref() {
                    LifecycleError::DeletionBlocked { .. } => ExitStatus::Data,
                    LifecycleError::PlanTargetMismatch { .. } => ExitStatus::Software,
                    LifecycleError::Context { .. } => ExitStatus::Software,
                },
            },
            SpecmanError::Workspace(_) => ExitStatus::Usage,
            SpecmanError::Serialization(_) => ExitStatus::Software,
            SpecmanError::Io(_) => ExitStatus::Io,
        };
        CliError::new(err.to_string(), status)
    }
}

impl From<clap::Error> for CliError {
    fn from(err: clap::Error) -> Self {
        let status = match err.kind() {
            ClapErrorKind::DisplayHelp | ClapErrorKind::DisplayVersion => ExitStatus::Ok,
            _ => ExitStatus::Usage,
        };
        if status == ExitStatus::Ok {
            let _ = err.print();
            CliError::new(String::new(), status)
        } else {
            CliError::new(err.to_string(), status)
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> Self {
        CliError::new(err.to_string(), ExitStatus::Io)
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CliError {}
