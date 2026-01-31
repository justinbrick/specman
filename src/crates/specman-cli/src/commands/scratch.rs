use std::fs;
use std::path::Path;

use clap::{Arg, ArgAction, ArgMatches, Command, ValueEnum, builder::EnumValueParser};
use serde::Serialize;
use specman::{
    ArtifactId, ArtifactKind, DependencyTree,
    ScratchFrontMatter, ScratchRefactorMetadata,
    ScratchRevisionMetadata, ScratchWorkType, ScratchWorkloadExtras,
    CreateResult, CreateScratchOptions, DeleteOptions, DeleteResult,
    create_scratch_pad, delete_artifact, split_front_matter,
};

use crate::commands::CommandResult;
use crate::commands::dependencies::{self, DependencyScope};
use crate::context::CliSession;
use crate::error::{CliError, ExitStatus};
use crate::util;

#[derive(Clone, Debug, Serialize)]
pub struct ScratchSummary {
    pub name: String,
    pub target: Option<String>,
    pub branch: Option<String>,
    pub work_type: Option<String>,
    pub path: String,
}

pub type DeletionTree = DependencyTree;

#[derive(Clone, Debug, ValueEnum)]
pub enum ScratchType {
    Feat,
    Ref,
    Revision,
}

impl ScratchType {
    fn as_key(&self) -> &'static str {
        match self {
            ScratchType::Feat => "feat",
            ScratchType::Ref => "ref",
            ScratchType::Revision => "revision",
        }
    }
}

pub fn command() -> Command {
    Command::new("scratch")
        .about("Manage scratch pads")
        .subcommand(ls_command())
        .subcommand(new_command())
        .subcommand(delete_command())
        .subcommand(dependencies_command())
}

pub fn run(session: &CliSession, matches: &ArgMatches) -> Result<CommandResult, CliError> {
    match matches.subcommand() {
        Some(("ls", _)) => list_scratchpads(session),
        Some(("new", sub)) => create_scratchpad(session, sub),
        Some(("delete", sub)) => delete_scratchpad(session, sub),
        Some(("dependencies", sub)) => scratch_dependencies(session, sub),
        _ => Err(CliError::new(
            "unsupported scratch command",
            ExitStatus::Usage,
        )),
    }
}

fn list_scratchpads(session: &CliSession) -> Result<CommandResult, CliError> {
    let mut pads = Vec::new();
    let mut missing_metadata = false;
    let scratch_dir = session.workspace_paths.scratchpad_dir();
    if scratch_dir.is_dir() {
        for entry in fs::read_dir(scratch_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let path = entry.path().join("scratch.md");
                if path.is_file() {
                    let summary = read_scratch_summary(session.workspace_paths.root(), &path)?;
                    if summary.target.is_none() || summary.work_type.is_none() {
                        missing_metadata = true;
                    }
                    pads.push(summary);
                }
            }
        }
        pads.sort_by(|a, b| a.name.cmp(&b.name));
    }
    Ok(CommandResult::ScratchList {
        pads,
        missing_metadata,
    })
}

fn create_scratchpad(
    session: &CliSession,
    matches: &ArgMatches,
) -> Result<CommandResult, CliError> {
    let name = matches
        .get_one::<String>("name")
        .cloned()
        .ok_or_else(|| CliError::new("--name is required", ExitStatus::Usage))?;
    util::validate_slug(&name, "scratch pad")?;

    let target = matches
        .get_one::<String>("target")
        .cloned()
        .ok_or_else(|| CliError::new("--target is required", ExitStatus::Usage))?;
    util::validate_locator(&target, "scratch target")?;

    let work_type_enum = matches
        .get_one::<ScratchType>("type")
        .expect("clap ensures required option");
    let work_key = work_type_enum.as_key();

    let branch = matches
        .get_one::<String>("branch")
        .cloned()
        .unwrap_or_else(|| default_branch(&target, work_key, &name));

    let work_type = match work_type_enum {
        ScratchType::Feat => ScratchWorkType::Feat(ScratchWorkloadExtras::default()),
        ScratchType::Ref => ScratchWorkType::Refactor(ScratchRefactorMetadata::default()),
        ScratchType::Revision => ScratchWorkType::Revision(ScratchRevisionMetadata::default()),
    };

    let result = create_scratch_pad(
        &session.env,
        CreateScratchOptions {
            name: name.clone(),
            target,
            branch: Some(branch),
            work_type,
            dry_run: false,
            front_matter: None,
        },
    )
    .map_err(CliError::from)?;

    let path = match result {
        CreateResult::Persisted(p) => p.path,
        CreateResult::DryRun(_) => unreachable!(),
    };

    let summary = read_scratch_summary(session.workspace_paths.root(), &path)?;

    Ok(CommandResult::ScratchCreated {
        summary,
        path: util::workspace_relative(session.workspace_paths.root(), &path),
    })
}

fn delete_scratchpad(
    session: &CliSession,
    matches: &ArgMatches,
) -> Result<CommandResult, CliError> {
    let name = matches
        .get_one::<String>("name")
        .cloned()
        .ok_or_else(|| CliError::new("scratch pad name required", ExitStatus::Usage))?;
    let forced = matches.get_flag("force");
    let artifact = ArtifactId {
        kind: ArtifactKind::ScratchPad,
        name: name.clone(),
    };
    let folder = session.workspace_paths.scratchpad_dir().join(&name);
    if !folder.exists() {
        return Err(CliError::new(
            format!("scratch pad {name} does not exist"),
            ExitStatus::Usage,
        ));
    }

    let scratch_file = folder.join("scratch.md");
    let summary = if scratch_file.is_file() {
        read_scratch_summary(session.workspace_paths.root(), &scratch_file)?
    } else {
        ScratchSummary {
            name: name.clone(),
            target: None,
            branch: None,
            work_type: None,
            path: util::workspace_relative(session.workspace_paths.root(), &folder),
        }
    };

    let impact = specman::check_deletion_impact(&session.env, &artifact)
        .map_err(CliError::from)?;

    if impact.blocked && !forced {
        return Err(CliError::new(
            format!("refusing to delete {name}; downstream artifacts detected (use --force)"),
            ExitStatus::Data,
        ));
    }

    let result = delete_artifact(
        &session.env,
        &artifact,
        DeleteOptions {
            force: forced,
            dry_run: false,
        },
    )
    .map_err(CliError::from)?;

    let _removed = match result {
        DeleteResult::Removed(r) => r,
        DeleteResult::DryRun(_) => unreachable!(),
    };

    Ok(CommandResult::ScratchDeleted {
        summary,
        forced,
        tree: impact.dependencies,
        removed_path: util::workspace_relative(
            session.workspace_paths.root(),
            &session.workspace_paths.scratchpad_dir().join(&name),
        ),
    })
}

fn ls_command() -> Command {
    Command::new("ls").about("List scratch pads in the workspace")
}

fn new_command() -> Command {
    Command::new("new")
        .about("Create a new scratch pad")
        .arg(
            Arg::new("name")
                .required(true)
                .long("name")
                .value_name("NAME")
                .help("Scratch pad slug (lowercase, hyphenated)"),
        )
        .arg(
            Arg::new("target")
                .required(true)
                .long("target")
                .value_name("PATH|URL")
                .help("Target specification or implementation path/URL"),
        )
        .arg(
            Arg::new("type")
                .required(true)
                .long("type")
                .value_name("feat|ref|revision")
                .value_parser(EnumValueParser::<ScratchType>::new())
                .help("Scratch pad work type"),
        )
        .arg(
            Arg::new("branch")
                .long("branch")
                .value_name("BRANCH")
                .help("Override the default {target}/{type}/{name} branch naming scheme"),
        )
}

fn delete_command() -> Command {
    Command::new("delete")
        .about("Delete a scratch pad")
        .arg(
            Arg::new("name")
                .required(true)
                .value_name("NAME")
                .help("Scratch pad name to delete"),
        )
        .arg(
            Arg::new("force")
                .long("force")
                .action(ArgAction::SetTrue)
                .help("Override dependency blockers after printing the dependency tree."),
        )
}

fn dependencies_command() -> Command {
    dependencies::with_direction_flags(
        Command::new("dependencies")
            .about("Render the dependency tree for a scratch pad")
            .arg(
                Arg::new("name")
                    .required(true)
                    .value_name("NAME")
                    .help("Scratch pad slug (folder name)"),
            ),
    )
}

fn scratch_dependencies(
    session: &CliSession,
    matches: &ArgMatches,
) -> Result<CommandResult, CliError> {
    let name = matches
        .get_one::<String>("name")
        .cloned()
        .ok_or_else(|| CliError::new("scratch pad name required", ExitStatus::Usage))?;
    util::validate_slug(&name, "scratch pad")?;

    let scratch_file = session
        .workspace_paths
        .scratchpad_dir()
        .join(&name)
        .join("scratch.md");
    if !scratch_file.is_file() {
        return Err(CliError::new(
            format!("scratch pad {name} does not exist"),
            ExitStatus::Usage,
        ));
    }

    let view = dependencies::parse_view(matches)?;
    let artifact = ArtifactId {
        kind: ArtifactKind::ScratchPad,
        name,
    };
    let tree = session
        .env
        .mapping
        .dependency_tree(&artifact)
        .map_err(CliError::from)?;

    Ok(CommandResult::DependencyTree {
        scope: DependencyScope::ScratchPad,
        view,
        tree,
    })
}

fn read_scratch_summary(root: &Path, path: &Path) -> Result<ScratchSummary, CliError> {
    let content = fs::read_to_string(path)?;
    let split = split_front_matter(&content)
        .map_err(|err| CliError::new(err.to_string(), ExitStatus::Config))?;
    let fm: ScratchFrontMatter = serde_yaml::from_str(split.yaml)
        .map_err(|err| CliError::new(err.to_string(), ExitStatus::Config))?;
    let branch = fm.branch.clone();
    let work_type = fm
        .work_type
        .as_ref()
        .map(|ty| ty.kind().as_str().to_string());
    let target = fm.target.clone().and_then(|value| {
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    });
    Ok(ScratchSummary {
        name: fm
            .identity
            .name
            .clone()
            .unwrap_or_else(|| infer_name_from_path(path)),
        target,
        branch,
        work_type,
        path: util::workspace_relative(root, path),
    })
}

fn infer_name_from_path(path: &Path) -> String {
    path.parent()
        .and_then(|dir| dir.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn default_branch(target: &str, work_type: &str, scratch_name: &str) -> String {
    let target_slug = target
        .split('/')
        .next_back()
        .and_then(|segment| segment.split('.').next())
        .unwrap_or(target);
    format!("{target_slug}/{work_type}/{scratch_name}")
}
