use std::env;
use std::path::{Component, Path, PathBuf};

use clap::{Arg, ArgAction, ArgMatches, Command};
use specman::workspace::{WorkspaceDiscovery, WorkspaceError};

use crate::commands::CommandResult;
use crate::error::{CliError, ExitStatus};

pub fn command() -> Command {
    Command::new("init")
        .about("Initialize a SpecMan workspace")
        .arg(
            Arg::new("path")
                .value_name("PATH")
                .help("Workspace root to initialize. Defaults to the current directory."),
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .action(ArgAction::SetTrue)
                .help("Show the resolved workspace root without writing to disk."),
        )
}

pub fn run(
    workspace_override: Option<String>,
    matches: &ArgMatches,
) -> Result<CommandResult, CliError> {
    let positional = matches.get_one::<String>("path").cloned();
    let target = workspace_override
        .or(positional)
        .unwrap_or_else(|| ".".to_string());
    let dry_run = matches.get_flag("dry-run");

    let root = absolutize(&target)?;
    let plan = plan_workspace(&root)?;

    if dry_run {
        return Ok(CommandResult::WorkspaceInitialized {
            root: plan.root.display().to_string(),
            dot_specman: plan.dot_specman.display().to_string(),
            created: !plan.already_initialized,
            dry_run: true,
        });
    }

    let created = apply_init(&plan)?;
    Ok(CommandResult::WorkspaceInitialized {
        root: plan.root.display().to_string(),
        dot_specman: plan.dot_specman.display().to_string(),
        created,
        dry_run: false,
    })
}

struct InitPlan {
    root: PathBuf,
    dot_specman: PathBuf,
    already_initialized: bool,
}

fn plan_workspace(root: &Path) -> Result<InitPlan, CliError> {
    if root.exists() && !root.is_dir() {
        return Err(CliError::new(
            format!("workspace root {} is not a directory", root.display()),
            ExitStatus::Usage,
        ));
    }

    let dot_specman = root.join(".specman");
    let already_initialized = dot_specman.is_dir();
    for ancestor in root.ancestors().skip(1) {
        let existing = ancestor.join(".specman");
        if existing.is_dir() {
            return Err(CliError::new(
                format!(
                    "cannot initialize workspace at {}; ancestor workspace already exists at {}",
                    root.display(),
                    existing.display()
                ),
                ExitStatus::Usage,
            ));
        }
    }

    Ok(InitPlan {
        root: root.to_path_buf(),
        dot_specman,
        already_initialized,
    })
}

fn apply_init(plan: &InitPlan) -> Result<bool, CliError> {
    if plan.already_initialized {
        return Ok(false);
    }

    WorkspaceDiscovery::create(&plan.root).map_err(map_workspace_error)?;
    Ok(true)
}

fn absolutize(raw: &str) -> Result<PathBuf, CliError> {
    let candidate = PathBuf::from(raw);
    let base = if candidate.is_absolute() {
        candidate
    } else {
        env::current_dir()
            .map_err(|err| CliError::new(err.to_string(), ExitStatus::Io))?
            .join(candidate)
    };

    Ok(lexical_normalize(&base))
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut pending_parents: usize = 0;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized
                    .components()
                    .next_back()
                    .is_some_and(|c| matches!(c, Component::Normal(_)))
                {
                    normalized.pop();
                } else if normalized.is_absolute() {
                    // Ignore attempts to go above the root for absolute paths.
                } else {
                    pending_parents += 1;
                }
            }
            Component::Normal(part) => {
                while pending_parents > 0 {
                    normalized.push("..");
                    pending_parents -= 1;
                }
                normalized.push(part);
            }
        }
    }

    while pending_parents > 0 {
        normalized.push("..");
        pending_parents -= 1;
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

fn map_workspace_error(err: WorkspaceError) -> CliError {
    let status = match err {
        WorkspaceError::Io(_) => ExitStatus::Io,
        _ => ExitStatus::Usage,
    };
    CliError::new(err.to_string(), status)
}
