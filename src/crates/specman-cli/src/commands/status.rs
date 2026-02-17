use std::time::Duration;

use clap::Command;
use crate::commands::CommandResult;
use crate::context::CliSession;
use crate::error::CliError;

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
    let local = matches.get_flag("local");
    // [ENSURES: concept-workspace-status.requirements:TEST]
    let options = specman::ReferenceValidationOptions {
        https: specman::HttpsValidationOptions {
            mode: if local {
                specman::HttpsValidationMode::SyntaxOnly
            } else {
                specman::HttpsValidationMode::Reachability {
                    timeout: Duration::from_secs(30),
                    max_redirects: 10,
                    method: specman::HttpsMethod::Head,
                }
            },
        },
        transitive: specman::TransitiveOptions::default(),
    };

    let report = specman::validate_workspace_status(
        session.workspace_paths.root().to_path_buf(),
        specman::WorkspaceStatusConfig {
            structure: true,
            references: true,
            cycles: true,
            compliance: true,
            scratchpads: true,
            reference_options: Some(options),
        },
    )?;

    Ok(CommandResult::Status(report))
}

