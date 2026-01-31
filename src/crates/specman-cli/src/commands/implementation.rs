use std::fs;
use std::path::{Path, PathBuf};

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde::Serialize;
use specman::{
    ArtifactId, ArtifactKind, ArtifactSummary, DependencyTree,
    ImplementationFrontMatter, SpecificationFrontMatter, ArtifactIdentityFields, ReferenceEntry,
    CreateImplOptions, CreateResult, DeleteOptions, DeleteResult,
    create_implementation, delete_artifact, split_front_matter,
};

use crate::commands::CommandResult;
use crate::commands::dependencies::{self, DependencyScope};
use crate::context::CliSession;
use crate::error::{CliError, ExitStatus};
use crate::util;

pub type DeletionTree = DependencyTree;

#[derive(Clone, Debug, Serialize)]
pub struct ImplSummary {
    pub name: String,
    pub spec_locator: Option<String>,
    pub spec_identifier: Option<String>,
    pub version: Option<String>,
    pub path: String,
}

pub fn command() -> Command {
    Command::new("impl")
        .about("Manage implementation artifacts")
        .subcommand(ls_command())
        .subcommand(new_command())
        .subcommand(delete_command())
        .subcommand(dependencies_command())
}

pub fn run(session: &CliSession, matches: &ArgMatches) -> Result<CommandResult, CliError> {
    match matches.subcommand() {
        Some(("ls", _)) => list_impls(session),
        Some(("new", sub)) => create_impl(session, sub),
        Some(("delete", sub)) => delete_impl(session, sub),
        Some(("dependencies", sub)) => impl_dependencies(session, sub),
        _ => Err(CliError::new("unsupported impl command", ExitStatus::Usage)),
    }
}

fn list_impls(session: &CliSession) -> Result<CommandResult, CliError> {
    let mut implementations = Vec::new();
    let impl_dir = session.workspace_paths.impl_dir();
    if impl_dir.is_dir() {
        for entry in fs::read_dir(impl_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let path = entry.path().join("impl.md");
                if path.is_file() {
                    implementations.push(read_impl_summary(session.workspace_paths.root(), &path)?);
                }
            }
        }
        implementations.sort_by(|a, b| a.name.cmp(&b.name));
    }
    Ok(CommandResult::ImplList { implementations })
}

fn create_impl(session: &CliSession, matches: &ArgMatches) -> Result<CommandResult, CliError> {
    let positional = matches.get_one::<String>("name");
    let flag = matches.get_one::<String>("name-flag");
    if positional.is_some() && flag.is_some() {
        return Err(CliError::new(
            "specify the implementation name either positionally or with --name, not both",
            ExitStatus::Usage,
        ));
    }
    let name = flag
        .cloned()
        .or_else(|| positional.cloned())
        .ok_or_else(|| CliError::new("implementation name required", ExitStatus::Usage))?;
    util::validate_slug(&name, "implementation")?;

    let spec_locator = matches
        .get_one::<String>("spec")
        .ok_or_else(|| CliError::new("--spec is required", ExitStatus::Usage))?;
    let resolved_spec = resolve_spec_locator(session, spec_locator)?;

    let location = matches
        .get_one::<String>("location")
        .cloned()
        .unwrap_or_else(|| format!("src/{name}"));

    let folder = session.workspace_paths.impl_dir().join(&name);
    if folder.exists() {
        return Err(CliError::new(
            format!("implementation {name} already exists"),
            ExitStatus::Usage,
        ));
    }

    let mut references = Vec::new();
    // Add implicit reference to spec (legacy behavior preserved)
    references.push(ReferenceEntry {
        reference: resolved_spec.clone(),
        reference_type: Some("specification".to_string()),
        optional: Some(false),
    });

    let front_matter = ImplementationFrontMatter {
        identity: ArtifactIdentityFields::default(),
        spec: Some(resolved_spec.clone()),
        location: Some(location),
        references,
        ..Default::default()
    };

    let result = create_implementation(
        &session.env,
        CreateImplOptions {
            name: name.clone(),
            target: resolved_spec.clone(),
            dry_run: false,
            front_matter: Some(front_matter),
        },
    )
    .map_err(CliError::from)?;

    let path = match result {
        CreateResult::Persisted(p) => p.path,
        CreateResult::DryRun(_) => unreachable!(),
    };

    let summary = read_impl_summary(session.workspace_paths.root(), &path)?;

    Ok(CommandResult::ImplCreated {
        summary,
        path: path.display().to_string(),
    })
}

fn delete_impl(session: &CliSession, matches: &ArgMatches) -> Result<CommandResult, CliError> {
    let name = matches
        .get_one::<String>("name")
        .cloned()
        .ok_or_else(|| CliError::new("implementation name required", ExitStatus::Usage))?;
    let forced = matches.get_flag("force");
    let artifact = ArtifactId {
        kind: ArtifactKind::Implementation,
        name: name.clone(),
    };
    let folder = session.workspace_paths.impl_dir().join(&name);
    if !folder.exists() {
        return Err(CliError::new(
            format!("implementation {name} does not exist"),
            ExitStatus::Usage,
        ));
    }

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
    
    let removed = match result {
        DeleteResult::Removed(r) => r,
        DeleteResult::DryRun(_) => unreachable!(),
    };
    
    let removed_path = util::workspace_relative(session.workspace_paths.root(), &removed.directory);

    let spec_locator = impact.dependencies.root.metadata.get("spec").cloned();
    let spec_identifier = spec_locator
        .as_deref()
        .and_then(|locator| spec_identifier_from_locator(session.workspace_paths.root(), locator));
    let summary = ImplSummary {
        name,
        spec_locator,
        spec_identifier,
        version: version_from_summary(&impact.dependencies.root),
        path: util::workspace_relative(session.workspace_paths.root(), &folder),
    };

    Ok(CommandResult::ImplDeleted {
        summary,
        forced,
        tree: impact.dependencies,
        removed_path,
    })
}

fn ls_command() -> Command {
    Command::new("ls").about("List implementations in the workspace")
}

fn new_command() -> Command {
    Command::new("new")
        .about("Create a new implementation")
        .arg(Arg::new("name").value_name("NAME"))
        .arg(
            Arg::new("name-flag").long("name").value_name("NAME").help(
                "Explicit implementation name. If set, the positional argument must be omitted.",
            ),
        )
        .arg(
            Arg::new("spec")
                .long("spec")
                .required(true)
                .value_name("SPEC")
                .help("Target specification name, workspace-relative path, or HTTPS URL"),
        )
        .arg(
            Arg::new("location")
                .long("location")
                .value_name("PATH")
                .help("Relative path to the implementation code location"),
        )
}

fn delete_command() -> Command {
    Command::new("delete")
        .about("Delete an implementation after dependency checks")
        .arg(
            Arg::new("name")
                .required(true)
                .value_name("NAME")
                .help("Implementation name to delete"),
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
            .about("Render the dependency tree for an implementation")
            .arg(
                Arg::new("name")
                    .required(true)
                    .value_name("NAME")
                    .help("Implementation slug (folder name)"),
            ),
    )
}

fn impl_dependencies(
    session: &CliSession,
    matches: &ArgMatches,
) -> Result<CommandResult, CliError> {
    let name = matches
        .get_one::<String>("name")
        .cloned()
        .ok_or_else(|| CliError::new("implementation name required", ExitStatus::Usage))?;
    util::validate_slug(&name, "implementation")?;

    let impl_file = session
        .workspace_paths
        .impl_dir()
        .join(&name)
        .join("impl.md");
    if !impl_file.is_file() {
        return Err(CliError::new(
            format!("implementation {name} does not exist"),
            ExitStatus::Usage,
        ));
    }

    let view = dependencies::parse_view(matches)?;
    let artifact = ArtifactId {
        kind: ArtifactKind::Implementation,
        name,
    };
    let tree = session
        .env
        .mapping
        .dependency_tree(&artifact)
        .map_err(CliError::from)?;

    Ok(CommandResult::DependencyTree {
        scope: DependencyScope::Implementation,
        view,
        tree,
    })
}

fn read_impl_summary(root: &Path, path: &Path) -> Result<ImplSummary, CliError> {
    let content = fs::read_to_string(path)?;
    let split = split_front_matter(&content)
        .map_err(|err| CliError::new(err.to_string(), ExitStatus::Config))?;
    let fm: ImplementationFrontMatter = serde_yaml::from_str(split.yaml)
        .map_err(|err| CliError::new(err.to_string(), ExitStatus::Config))?;
    let spec_locator = fm.spec.clone();
    let spec_identifier = spec_locator
        .as_deref()
        .and_then(|locator| spec_identifier_from_locator(root, locator));
    Ok(ImplSummary {
        name: fm
            .identity
            .name
            .clone()
            .unwrap_or_else(|| infer_name_from_path(path)),
        spec_locator,
        spec_identifier,
        version: fm.identity.version.clone(),
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

fn version_from_summary(summary: &ArtifactSummary) -> Option<String> {
    summary.version.as_ref().map(ToString::to_string)
}

fn resolve_spec_locator(session: &CliSession, input: &str) -> Result<String, CliError> {
    if input.starts_with("https://") {
        return Ok(input.to_string());
    }
    if input.contains("://") {
        return Err(CliError::new(
            format!("unsupported spec locator scheme: {input}"),
            ExitStatus::Usage,
        ));
    }

    let workspace_root = session.workspace_paths.root();
    let direct = workspace_root.join(input);
    if direct.is_file() {
        return Ok(util::workspace_relative(workspace_root, &direct));
    }

    let shorthand = workspace_root.join("spec").join(input).join("spec.md");
    if shorthand.is_file() {
        return Ok(util::workspace_relative(workspace_root, &shorthand));
    }

    Err(CliError::new(
        format!("unable to resolve specification locator '{input}'"),
        ExitStatus::Usage,
    ))
}

fn spec_identifier_from_locator(root: &Path, locator: &str) -> Option<String> {
    if locator.starts_with("https://") {
        return Some(locator.to_string());
    }
    let relative = Path::new(locator);
    let resolved: PathBuf = if relative.is_absolute() {
        relative.into()
    } else {
        root.join(relative)
    };
    if !resolved.is_file() {
        return Some(locator.to_string());
    }
    let content = fs::read_to_string(&resolved).ok()?;
    let split = split_front_matter(&content).ok()?;
    let fm: SpecificationFrontMatter = serde_yaml::from_str(split.yaml).ok()?;
    match (fm.identity.name, fm.identity.version) {
        (Some(name), Some(version)) => Some(format!("{name}@{version}")),
        (Some(name), None) => Some(name),
        (None, Some(version)) => Some(format!("(unknown)@{version}")),
        _ => Some(locator.to_string()),
    }
}
