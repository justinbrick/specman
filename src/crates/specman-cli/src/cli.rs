use std::ffi::OsString;
use std::process::ExitCode;

use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::commands;
use crate::context::CliSession;
use crate::error::{CliError, ExitStatus};
use crate::formatter::{OutputFormat, emit_result};
use crate::util::Verbosity;

const NAME: &str = "specman";

pub fn run() -> ExitCode {
    init_tracing();
    match run_cli(std::env::args()) {
        Ok(code) => code,
        Err(err) => {
            err.print();
            err.exit_code()
        }
    }
}

/// Parses CLI arguments, resolves the workspace, and dispatches to the appropriate
/// command while enforcing the CLI Invocation Model (spec/specman-cli/spec.md#concept-cli-invocation-model).
/// Returns a POSIX `sysexits`-compatible `ExitCode` so automation can react deterministically.
pub fn run_cli<I, S>(args: I) -> Result<ExitCode, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
{
    let command = build_cli();
    let matches = command.try_get_matches_from(args)?;

    let verbosity = Verbosity {
        json: matches.get_flag("json"),
        verbose: matches.get_flag("verbose"),
    };
    let output = if verbosity.json {
        OutputFormat::Json
    } else {
        OutputFormat::Text
    };

    let workspace_override = matches.get_one::<String>("workspace").cloned();
    let session = CliSession::bootstrap(workspace_override, verbosity)?;
    if session.verbosity.verbose {
        tracing::info!(
            workspace = %session.workspace_paths.root().display(),
            spec_dir = %session.workspace_paths.spec_dir().display(),
            impl_dir = %session.workspace_paths.impl_dir().display(),
            scratch_dir = %session.workspace_paths.scratchpad_dir().display(),
            "resolved workspace context"
        );
    }

    let result = dispatch(&session, &matches)?;
    emit_result(result, output)
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}

/// Defines the root `clap::Command` tree, including global flags and subcommands for
/// `status`, `spec`, `impl`, and `scratch`. Keeping the tree centralized ensures every
/// command advertises its help text per the CLI Invocation Model requirements.
fn build_cli() -> Command {
    Command::new(NAME)
        .about("SpecMan CLI")
        .arg(
            Arg::new("workspace")
                .long("workspace")
                .value_name("PATH")
                .help("Specify the workspace root. Defaults to the nearest ancestor with a .specman folder."),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .action(ArgAction::SetTrue)
                .help("Emit newline-delimited JSON instead of human-readable text."),
        )
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .action(ArgAction::SetTrue)
                .help("Emit additional logging about template locators, workspace paths, and adapters."),
        )
        .subcommand_required(true)
        .subcommand(commands::status::command())
        .subcommand(commands::spec::command())
        .subcommand(commands::implementation::command())
        .subcommand(commands::scratch::command())
}

/// Delegates parsed subcommands to their respective modules, ensuring the Lifecycle
/// Command Surface stays thin and predictable. Unknown subcommands map to `EX_USAGE` so
/// callers receive actionable feedback.
fn dispatch(
    session: &CliSession,
    matches: &ArgMatches,
) -> Result<commands::CommandResult, CliError> {
    match matches.subcommand() {
        Some(("status", sub)) => commands::status::run(session, sub),
        Some(("spec", sub)) => commands::spec::run(session, sub),
        Some(("impl", sub)) => commands::implementation::run(session, sub),
        Some(("scratch", sub)) => commands::scratch::run(session, sub),
        _ => Err(CliError::new("missing command", ExitStatus::Usage)),
    }
}
