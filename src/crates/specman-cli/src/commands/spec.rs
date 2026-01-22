use std::fs;
use std::path::Path;

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde::Serialize;
use specman::dependency_tree::{
    ArtifactId, ArtifactKind, ArtifactSummary, DependencyTree,
};
use specman::front_matter::{self, DependencyEntry, ArtifactIdentityFields, SpecificationFrontMatter};
use specman::ops::{self, CreateSpecOptions, CreateResult, DeleteOptions, DeleteResult};

use crate::commands::CommandResult;
use crate::commands::dependencies::{self, DependencyScope};
use crate::context::CliSession;
use crate::error::{CliError, ExitStatus};
use crate::util;

pub type DeletionTree = DependencyTree;

#[derive(Clone, Debug, Serialize)]
pub struct SpecSummary {
    pub name: String,
    pub version: Option<String>,
    pub path: String,
}

pub fn command() -> Command {
    Command::new("spec")
        .about("Manage specification artifacts")
        .subcommand(spec_ls_command())
        .subcommand(spec_new_command())
        .subcommand(spec_delete_command())
        .subcommand(spec_dependencies_command())
}

pub fn run(session: &CliSession, matches: &ArgMatches) -> Result<CommandResult, CliError> {
    match matches.subcommand() {
        Some(("ls", _)) => list_specs(session),
        Some(("new", sub)) => create_spec(session, sub),
        Some(("delete", sub)) => delete_spec(session, sub),
        Some(("dependencies", sub)) => spec_dependencies(session, sub),
        _ => Err(CliError::new("unsupported spec command", ExitStatus::Usage)),
    }
}

fn list_specs(session: &CliSession) -> Result<CommandResult, CliError> {
    let mut specs = Vec::new();
    let spec_dir = session.workspace_paths.spec_dir();
    if spec_dir.is_dir() {
        for entry in fs::read_dir(spec_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let path = entry.path().join("spec.md");
                if path.is_file() {
                    specs.push(read_spec_summary(&path)?);
                }
            }
        }
        specs.sort_by(|a, b| a.name.cmp(&b.name));
    }
    Ok(CommandResult::SpecList { specs })
}

fn create_spec(session: &CliSession, matches: &ArgMatches) -> Result<CommandResult, CliError> {
    let positional = matches.get_one::<String>("name").cloned();
    let named = matches.get_one::<String>("name-flag").cloned();
    let name = named.or(positional).ok_or_else(|| {
        CliError::new(
            "spec name is required (positional or --name)",
            ExitStatus::Usage,
        )
    })?;
    util::validate_slug(&name, "specification")?;

    let version = matches
        .get_one::<String>("version")
        .cloned()
        .unwrap_or_else(|| "1.0.0".to_string());
    let dependencies = parse_dependencies(matches.get_one::<String>("dependencies"))?;

    let folder = session.workspace_paths.spec_dir().join(&name);
    if folder.exists() {
        return Err(CliError::new(
            format!("specification {name} already exists"),
            ExitStatus::Usage,
        ));
    }

    let mut dep_entries = Vec::new();
    for d in dependencies {
        dep_entries.push(DependencyEntry::Simple(d));
    }

    let front_matter = SpecificationFrontMatter {
        identity: ArtifactIdentityFields {
            version: Some(version),
            ..Default::default()
        },
        dependencies: dep_entries,
        ..Default::default()
    };

    let result = ops::create_specification(
        &session.env,
        CreateSpecOptions {
            name: name.clone(),
            title: name.clone(),
            dry_run: false,
            front_matter: Some(front_matter),
        },
    )
    .map_err(CliError::from)?;

    let path = match result {
        CreateResult::Persisted(p) => p.path,
        CreateResult::DryRun(_) => unreachable!(),
    };

    let summary = read_spec_summary(&path)?;

    Ok(CommandResult::SpecCreated {
        summary,
        path: path.display().to_string(),
    })
}

fn delete_spec(session: &CliSession, matches: &ArgMatches) -> Result<CommandResult, CliError> {
    let name = matches
        .get_one::<String>("name")
        .cloned()
        .ok_or_else(|| CliError::new("spec name required", ExitStatus::Usage))?;
    let forced = matches.get_flag("force");
    let artifact = ArtifactId {
        kind: ArtifactKind::Specification,
        name: name.clone(),
    };
    let folder = session.workspace_paths.spec_dir().join(&name);
    if !folder.exists() {
        return Err(CliError::new(
            format!("specification {name} does not exist"),
            ExitStatus::Usage,
        ));
    }

    let impact = specman::analysis::check_deletion_impact(&session.env, &artifact)
        .map_err(CliError::from)?;

    if impact.blocked && !forced {
        return Err(CliError::new(
            format!("refusing to delete {name}; downstream artifacts detected (use --force)"),
            ExitStatus::Data,
        ));
    }
    
    let result = ops::delete_artifact(
        &session.env,
        &artifact,
        DeleteOptions {
            force: forced,
            dry_run: false,
        },
    )
    .map_err(CliError::from)?;

    let removed = match result {
        DeleteResult::Removed(r) => r,
        DeleteResult::DryRun(_) => unreachable!(),
    };

    let removed_path = util::workspace_relative(session.workspace_paths.root(), &removed.directory);

    let summary = SpecSummary {
        name,
        version: version_from_summary(&impact.dependencies.root),
        path: folder.display().to_string(),
    };

    Ok(CommandResult::SpecDeleted {
        summary,
        forced,
        tree: impact.dependencies,
        removed_path,
    })
}

fn spec_ls_command() -> Command {
    Command::new("ls").about("List specifications in the workspace")
}

fn spec_new_command() -> Command {
    Command::new("new")
        .about("Create a new specification from the template")
        .arg(Arg::new("name").value_name("NAME"))
        .arg(
            Arg::new("name-flag")
                .long("name")
                .value_name("NAME")
                .help("Explicit specification name. Overrides positional value when provided."),
        )
        .arg(
            Arg::new("dependencies")
                .long("dependencies")
                .value_name("DEP1,DEP2")
                .help("Comma-separated dependency locators inserted into front matter."),
        )
        .arg(
            Arg::new("version")
                .long("version")
                .value_name("SEMVER")
                .default_value("1.0.0")
                .help("Version recorded in the front matter."),
        )
}

fn spec_delete_command() -> Command {
    Command::new("delete")
        .about("Delete a specification after dependency checks")
        .arg(
            Arg::new("name")
                .required(true)
                .value_name("NAME")
                .help("Specification name to delete"),
        )
        .arg(
            Arg::new("force")
                .long("force")
                .action(ArgAction::SetTrue)
                .help("Override dependency blockers after printing the dependency tree."),
        )
}

fn spec_dependencies_command() -> Command {
    dependencies::with_direction_flags(
        Command::new("dependencies")
            .about("Render the dependency tree for a specification")
            .arg(
                Arg::new("name")
                    .required(true)
                    .value_name("NAME")
                    .help("Specification slug (folder name)"),
            ),
    )
}

fn spec_dependencies(
    session: &CliSession,
    matches: &ArgMatches,
) -> Result<CommandResult, CliError> {
    let name = matches
        .get_one::<String>("name")
        .cloned()
        .ok_or_else(|| CliError::new("spec name required", ExitStatus::Usage))?;
    util::validate_slug(&name, "specification")?;

    let spec_file = session
        .workspace_paths
        .spec_dir()
        .join(&name)
        .join("spec.md");
    if !spec_file.is_file() {
        return Err(CliError::new(
            format!("specification {name} does not exist"),
            ExitStatus::Usage,
        ));
    }

    let view = dependencies::parse_view(matches)?;
    let artifact = ArtifactId {
        kind: ArtifactKind::Specification,
        name,
    };
    let tree = session
        .env
        .mapping
        .dependency_tree(&artifact)
        .map_err(CliError::from)?;

    Ok(CommandResult::DependencyTree {
        scope: DependencyScope::Specification,
        view,
        tree,
    })
}

fn parse_dependencies(raw: Option<&String>) -> Result<Vec<String>, CliError> {
    let mut deps = Vec::new();
    if let Some(value) = raw {
        for token in value.split(',') {
            let dep = token.trim();
            if dep.is_empty() {
                continue;
            }
            util::validate_locator(dep, "dependency")?;
            deps.push(dep.to_string());
        }
    }
    Ok(deps)
}

fn read_spec_summary(path: &Path) -> Result<SpecSummary, CliError> {
    let content = fs::read_to_string(path)?;
    let split = front_matter::split_front_matter(&content)
        .map_err(|err| CliError::new(err.to_string(), ExitStatus::Config))?;
    let fm: SpecificationFrontMatter = serde_yaml::from_str(split.yaml)
        .map_err(|err| CliError::new(err.to_string(), ExitStatus::Config))?;
    let name = fm
        .identity
        .name
        .clone()
        .unwrap_or_else(|| infer_name_from_path(path));
    let version = fm.identity.version.clone();
    Ok(SpecSummary {
        name,
        version,
        path: path.display().to_string(),
    })
}

fn infer_name_from_path(path: &Path) -> String {
    path.parent()
        .and_then(|dir| dir.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn version_from_summary(summary: &ArtifactSummary) -> Option<String> {
    summary.version.as_ref().map(ToString::to_string)
}
