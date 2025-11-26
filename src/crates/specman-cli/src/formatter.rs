use std::process::ExitCode;

use serde_json::json;
use specman::dependency_tree::DependencyTree;

use crate::commands::CommandResult;
use crate::error::CliError;

pub enum OutputFormat {
    Text,
    Json,
}

pub fn emit_result(result: CommandResult, format: OutputFormat) -> Result<ExitCode, CliError> {
    match format {
        OutputFormat::Text => print_text(&result),
        OutputFormat::Json => print_json(&result)?,
    };
    Ok(ExitCode::from(result.exit_status().code()))
}

fn print_text(result: &CommandResult) {
    match result {
        CommandResult::Status { reports, healthy } => {
            if *healthy {
                println!("Workspace status: OK ({} artifacts)", reports.len());
            } else {
                println!("Workspace status: FAIL ({} artifacts)", reports.len());
            }
            for report in reports {
                if report.ok {
                    println!("  [OK] {} ({})", report.name, report.kind);
                } else if let Some(message) = &report.message {
                    println!("  [ERR] {} ({}): {message}", report.name, report.kind);
                }
            }
        }
        CommandResult::SpecList { specs } => {
            println!("Specifications ({}):", specs.len());
            for spec in specs {
                let version = spec.version.as_deref().unwrap_or("unknown");
                println!("  - {} (version: {version})", spec.name);
            }
        }
        CommandResult::SpecCreated { summary, path } => {
            println!("Created specification '{}' at {}", summary.name, path);
        }
        CommandResult::SpecDeleted {
            summary,
            forced,
            tree,
            removed_path,
        } => {
            if *forced {
                println!(
                    "Deleted specification '{}' with --force (removed: {})",
                    summary.name, removed_path
                );
            } else {
                println!(
                    "Deleted specification '{}' (removed: {})",
                    summary.name, removed_path
                );
            }
            print_dependency_tree(tree);
        }
        CommandResult::ImplList { implementations } => {
            println!("Implementations ({}):", implementations.len());
            for imp in implementations {
                let version = imp.version.as_deref().unwrap_or("unknown");
                let language = imp.language.as_deref().unwrap_or("unknown");
                let target_spec = imp
                    .spec_identifier
                    .as_deref()
                    .or_else(|| imp.spec_locator.as_deref())
                    .unwrap_or("unknown");
                println!(
                    "  - {} (target: {}, lang: {}, version: {version})",
                    imp.name, target_spec, language
                );
            }
        }
        CommandResult::ImplCreated { summary, path } => {
            println!("Created implementation '{}' at {}", summary.name, path);
        }
        CommandResult::ImplDeleted {
            summary,
            forced,
            tree,
            removed_path,
        } => {
            if *forced {
                println!(
                    "Deleted implementation '{}' with --force (removed: {})",
                    summary.name, removed_path
                );
            } else {
                println!(
                    "Deleted implementation '{}' (removed: {})",
                    summary.name, removed_path
                );
            }
            print_dependency_tree(tree);
        }
        CommandResult::ScratchList {
            pads,
            missing_metadata,
        } => {
            if *missing_metadata {
                println!("Scratch pads ({}): metadata issues detected", pads.len());
            } else {
                println!("Scratch pads ({}):", pads.len());
            }
            for pad in pads {
                println!(
                    "  - {} (target: {}, work_type: {})",
                    pad.name,
                    pad.target.as_deref().unwrap_or("unknown"),
                    pad.work_type.as_deref().unwrap_or("unknown")
                );
                let mut issues = Vec::new();
                if pad.target.is_none() {
                    issues.push("missing target");
                }
                if pad.work_type.is_none() {
                    issues.push("missing work_type");
                }
                if !issues.is_empty() {
                    println!("    ! {}", issues.join(", "));
                }
            }
        }
        CommandResult::ScratchCreated { summary, path } => {
            println!("Created scratch pad '{}' at {}", summary.name, path);
        }
        CommandResult::ScratchDeleted {
            summary,
            forced,
            tree,
            removed_path,
        } => {
            if *forced {
                println!(
                    "Deleted scratch pad '{}' with --force (removed: {})",
                    summary.name, removed_path
                );
            } else {
                println!(
                    "Deleted scratch pad '{}' (removed: {})",
                    summary.name, removed_path
                );
            }
            print_dependency_tree(tree);
        }
    }
}

fn print_json(result: &CommandResult) -> Result<(), CliError> {
    let payload = json!(result);
    println!("{}", payload);
    Ok(())
}

fn print_dependency_tree(tree: &DependencyTree) {
    println!("    downstream dependents: {}", tree.downstream.len());
    for edge in &tree.downstream {
        let consumer = &edge.from.id.name;
        println!("      - {}", consumer);
    }
}
