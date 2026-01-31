use std::collections::BTreeMap;
use std::process::ExitCode;

use serde_json::json;
use specman::{ArtifactKind, ArtifactSummary, DependencyEdge, DependencyTree, TemplateLocator};

use crate::commands::CommandResult;
use crate::commands::dependencies::{self, DependencyScope, DependencyView};
use crate::commands::templates::PointerAction;
use crate::error::CliError;

pub enum OutputFormat {
    Text,
    Json,
}

/// Renders a `CommandResult` as either human-readable text or newline-delimited JSON,
/// satisfying the Observability & Error Surfacing concept while converting outcomes into
/// deterministic exit codes.
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
                } else {
                    if !report.errors.is_empty() {
                        println!("  [ERR] {} ({}):", report.name, report.kind);
                        for error in &report.errors {
                            let loc = error
                                .location
                                .as_deref()
                                .map(|l| format!(" [{}]", l))
                                .unwrap_or_default();
                            println!("    - {}{}", error.message, loc);
                        }
                    } else if let Some(message) = &report.message {
                        println!("  [ERR] {} ({}): {message}", report.name, report.kind);
                    } else {
                        println!("  [ERR] {} ({}): unknown error", report.name, report.kind);
                    }
                }
            }
        }
        CommandResult::WorkspaceInitialized {
            root,
            dot_specman,
            created,
            dry_run,
        } => {
            if *dry_run {
                if *created {
                    println!(
                        "Dry run: would initialize workspace at {} (.specman at {})",
                        root, dot_specman
                    );
                } else {
                    println!(
                        "Dry run: workspace already initialized at {} (.specman at {})",
                        root, dot_specman
                    );
                }
            } else if *created {
                println!(
                    "Initialized workspace at {} (.specman: {})",
                    root, dot_specman
                );
            } else {
                println!(
                    "Workspace already initialized at {} (.specman: {})",
                    root, dot_specman
                );
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
                let target_spec = imp
                    .spec_identifier
                    .as_deref()
                    .or(imp.spec_locator.as_deref())
                    .unwrap_or("unknown");
                println!(
                    "  - {} (target: {}, version: {version})",
                    imp.name, target_spec
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
        CommandResult::DependencyTree { scope, view, tree } => {
            print_dependency_view(*scope, *view, tree);
        }
        CommandResult::TemplatePointer { report } => {
            let action_label = match report.action {
                PointerAction::Set => "Updated",
                PointerAction::Remove => "Removed",
            };
            println!(
                "{} pointer for {} (tier: {:?})",
                action_label, report.kind, report.provenance.tier
            );
            println!(
                "  Locator: {}",
                describe_template_locator(&report.descriptor.locator)
            );
            println!("  Provenance locator: {}", report.provenance.locator);
            if let Some(pointer) = &report.provenance.pointer {
                println!("  Pointer file: {pointer}");
            }
            if let Some(cache_path) = &report.provenance.cache_path {
                println!("  Cache: {cache_path}");
            }
            if let Some(last_modified) = &report.provenance.last_modified {
                println!("  Last-Modified: {last_modified}");
            }
        }
    }
}

fn print_json(result: &CommandResult) -> Result<(), CliError> {
    let payload = json!(result);
    println!("{payload}");
    Ok(())
}

fn print_dependency_tree(tree: &DependencyTree) {
    render_direction_section(
        "Downstream",
        tree,
        DependencyView::Downstream,
        /*indent_root=*/ true,
    );
}

fn print_dependency_view(scope: DependencyScope, view: DependencyView, tree: &DependencyTree) {
    println!(
        "Dependency tree ({}) for {} '{}'",
        dependencies::view_label(view),
        dependencies::scope_label(scope),
        tree.root.id.name
    );

    match view {
        DependencyView::All => {
            render_direction_section("Downstream", tree, DependencyView::Downstream, false);
            render_direction_section("Upstream", tree, DependencyView::Upstream, false);
        }
        other => {
            let title = capitalize(dependencies::view_label(other));
            render_direction_section(&title, tree, other, false);
        }
    }
}

fn render_direction_section(
    title: &str,
    tree: &DependencyTree,
    view: DependencyView,
    indent_root: bool,
) {
    let edges = match view {
        DependencyView::Downstream => &tree.downstream,
        DependencyView::Upstream => &tree.upstream,
        DependencyView::All => &tree.aggregate,
    };

    if view == DependencyView::All {
        // The caller renders both sections separately when `--all` is selected, so this
        // branch should never be hit in practice.
        return;
    }

    println!("  {}: {} edge(s)", title, edges.len());
    let prefix = if indent_root { "    " } else { "  " };
    println!("{}{}", prefix, artifact_label(&tree.root));
    if edges.is_empty() {
        println!("{prefix}  (none)");
        return;
    }

    let children = build_children_map(edges, view);
    let mut stack = Vec::new();
    render_children(&tree.root, &children, prefix.to_string(), &mut stack);
}

fn build_children_map(
    edges: &[DependencyEdge],
    view: DependencyView,
) -> BTreeMap<String, Vec<ArtifactSummary>> {
    let mut map: BTreeMap<String, Vec<ArtifactSummary>> = BTreeMap::new();

    for edge in edges {
        let (parent, child) = match view {
            DependencyView::Downstream => (&edge.to, &edge.from),
            DependencyView::Upstream => (&edge.from, &edge.to),
            DependencyView::All => unreachable!("all view handled earlier"),
        };
        map.entry(parent.id.name.clone())
            .or_default()
            .push(child.clone());
    }

    for children in map.values_mut() {
        children.sort_by(|a, b| a.id.name.cmp(&b.id.name));
    }

    map
}

fn render_children(
    parent: &ArtifactSummary,
    children_map: &BTreeMap<String, Vec<ArtifactSummary>>,
    prefix: String,
    stack: &mut Vec<String>,
) {
    if let Some(children) = children_map.get(&parent.id.name) {
        stack.push(parent.id.name.clone());
        for (index, child) in children.iter().enumerate() {
            let connector = if index + 1 == children.len() {
                "└──"
            } else {
                "├──"
            };
            let cycle = stack.contains(&child.id.name);
            if cycle {
                println!(
                    "{}{} {} (cycle detected)",
                    prefix,
                    connector,
                    artifact_label(child)
                );
                continue;
            }
            println!("{}{} {}", prefix, connector, artifact_label(child));
            let next_prefix = format!(
                "{}{}",
                prefix,
                if index + 1 == children.len() {
                    "    "
                } else {
                    "│   "
                }
            );
            render_children(child, children_map, next_prefix, stack);
        }
        stack.pop();
    }
}

fn artifact_label(summary: &ArtifactSummary) -> String {
    let kind = match summary.id.kind {
        ArtifactKind::Specification => "spec",
        ArtifactKind::Implementation => "impl",
        ArtifactKind::ScratchPad => "scratch",
    };
    match summary.version.as_ref() {
        Some(version) => format!("{} {}@{}", kind, summary.id.name, version),
        None => format!("{} {}", kind, summary.id.name),
    }
}

fn capitalize(input: &str) -> String {
    let mut chars = input.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

fn describe_template_locator(locator: &TemplateLocator) -> String {
    match locator {
        TemplateLocator::FilePath(path) => path.display().to_string(),
        TemplateLocator::Url(url) => url.clone(),
    }
}
