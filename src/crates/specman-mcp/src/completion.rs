use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use rmcp::model::{ArgumentInfo, CompleteRequestParams, CompletionContext, Reference};
use serde_json::{Map, Value, json};

use specman::WorkspacePaths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArtifactClass {
    Spec,
    Impl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandleScope {
    SpecOnly,
    ImplOnly,
}

#[derive(Debug, Default)]
struct CompletionIndex {
    specs: Vec<String>,
    impls: Vec<String>,
    warnings: Vec<String>,
}

pub(crate) struct CompletionOutcome {
    pub(crate) values: Vec<String>,
    pub(crate) warnings: Vec<String>,
}

pub(crate) fn capability_descriptor_metadata() -> Map<String, Value> {
    let value = json!({
        "specmanCapabilityDescriptor": {
            "entity": "SpecManCapabilityDescriptor",
            "completion": {
                "warningTransport": "notifications/message",
                "ordering": "artifact-class-first-then-lexical-handle",
                "scratchSuggestions": false,
                "surfaces": [
                    { "surface": "prompt.revision.target", "acceptedKinds": ["spec"] },
                    { "surface": "prompt.migration.target", "acceptedKinds": ["spec"] },
                    { "surface": "prompt.impl.spec", "acceptedKinds": ["spec"] },
                    { "surface": "prompt.feat.target", "acceptedKinds": ["impl"] },
                    { "surface": "prompt.ref.target", "acceptedKinds": ["impl"] },
                    { "surface": "prompt.fix.target", "acceptedKinds": ["impl"] },
                    { "surface": "prompt.compliance.implementation", "acceptedKinds": ["impl"] },
                    { "surface": "resource.spec://{artifact}", "acceptedKinds": ["spec"] },
                    { "surface": "resource.impl://{artifact}", "acceptedKinds": ["impl"] },
                    { "surface": "resource.spec://{artifact}/constraints", "acceptedKinds": ["spec"] },
                    { "surface": "resource.spec://{artifact}/constraints/{constraint_id}", "acceptedKinds": ["spec"] },
                    { "surface": "resource.impl://{artifact}/compliance", "acceptedKinds": ["impl"] }
                ]
            }
        }
    });

    value.as_object().cloned().unwrap_or_default()
}

pub(crate) fn complete_request(
    workspace: &WorkspacePaths,
    request: &CompleteRequestParams,
) -> CompletionOutcome {
    let mut index = build_index(workspace);

    let values = match &request.r#ref {
        Reference::Prompt(prompt_ref) => complete_prompt(
            &index,
            &prompt_ref.name,
            &request.argument,
            request.context.as_ref(),
        ),
        Reference::Resource(resource_ref) => complete_resource(
            &index,
            workspace,
            &resource_ref.uri,
            &request.argument,
            request.context.as_ref(),
        ),
    };

    CompletionOutcome {
        values,
        warnings: std::mem::take(&mut index.warnings),
    }
}

fn complete_prompt(
    index: &CompletionIndex,
    prompt_name: &str,
    argument: &ArgumentInfo,
    _context: Option<&CompletionContext>,
) -> Vec<String> {
    let scope = match (prompt_name, argument.name.as_str()) {
        ("revision", "target") => Some(HandleScope::SpecOnly),
        ("migration", "target") => Some(HandleScope::SpecOnly),
        ("impl", "spec") => Some(HandleScope::SpecOnly),
        ("feat", "target") => Some(HandleScope::ImplOnly),
        ("ref", "target") => Some(HandleScope::ImplOnly),
        ("fix", "target") => Some(HandleScope::ImplOnly),
        ("compliance", "implementation") => Some(HandleScope::ImplOnly),
        _ => None,
    };

    match scope {
        Some(HandleScope::SpecOnly) => filter_handles(&index.specs, &argument.value),
        Some(HandleScope::ImplOnly) => filter_handles(&index.impls, &argument.value),
        None => Vec::new(),
    }
}

fn complete_resource(
    index: &CompletionIndex,
    workspace: &WorkspacePaths,
    uri_template: &str,
    argument: &ArgumentInfo,
    context: Option<&CompletionContext>,
) -> Vec<String> {
    let normalized = normalize_uri_template(uri_template);

    if argument.name == "artifact" {
        let scope = if normalized.starts_with("spec://{artifact}") {
            Some(HandleScope::SpecOnly)
        } else if normalized.starts_with("impl://{artifact}") {
            Some(HandleScope::ImplOnly)
        } else {
            None
        };

        return match scope {
            Some(HandleScope::SpecOnly) => filter_slugs(&index.specs, &argument.value),
            Some(HandleScope::ImplOnly) => filter_slugs(&index.impls, &argument.value),
            None => Vec::new(),
        };
    }

    if normalized == "spec://{artifact}/constraints/{constraint_id}"
        && argument.name == "constraint_id"
    {
        let artifact = context
            .and_then(|ctx| ctx.get_argument("artifact"))
            .map(String::as_str)
            .unwrap_or_default();
        return complete_constraint_ids(workspace, artifact, &argument.value);
    }

    Vec::new()
}

fn complete_constraint_ids(
    workspace: &WorkspacePaths,
    artifact_name: &str,
    current: &str,
) -> Vec<String> {
    if artifact_name.trim().is_empty() {
        return Vec::new();
    }

    let path = workspace
        .spec_dir()
        .join(artifact_name.trim())
        .join("spec.md");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return Vec::new(),
    };

    let mut ids = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('!') {
            continue;
        }

        let Some((raw, _)) = trimmed[1..].split_once(':') else {
            continue;
        };
        let id = raw.trim();
        if id.is_empty() || id.contains('/') || id.chars().any(char::is_whitespace) {
            continue;
        }
        ids.insert(id.to_string());
    }

    ids.into_iter()
        .filter(|id| id.starts_with(current))
        .collect()
}

fn build_index(workspace: &WorkspacePaths) -> CompletionIndex {
    let mut index = CompletionIndex::default();
    index.specs = collect_handles(
        workspace.spec_dir().as_path(),
        "spec.md",
        ArtifactClass::Spec,
        &mut index.warnings,
    );
    index.impls = collect_handles(
        workspace.impl_dir().as_path(),
        "impl.md",
        ArtifactClass::Impl,
        &mut index.warnings,
    );
    index
}

fn collect_handles(
    root: &Path,
    artifact_file: &str,
    class: ArtifactClass,
    warnings: &mut Vec<String>,
) -> Vec<String> {
    let mut out = Vec::new();

    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) => {
            warnings.push(format!(
                "completion index degraded: failed to read '{}': {err}",
                root.display()
            ));
            return out;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if !is_slug_like(&name) {
            warnings.push(format!(
                "completion index degraded: skipped invalid artifact directory '{}'; expected lowercase slug",
                path.display()
            ));
            continue;
        }

        let marker = path.join(artifact_file);
        if !marker.exists() {
            warnings.push(format!(
                "completion index degraded: skipped '{}' because '{}' is missing",
                path.display(),
                artifact_file
            ));
            continue;
        }

        let handle = match class {
            ArtifactClass::Spec => format!("spec://{name}"),
            ArtifactClass::Impl => format!("impl://{name}"),
        };
        out.push(handle);
    }

    out.sort_unstable();
    out.dedup();
    out
}

fn filter_handles(handles: &[String], current: &str) -> Vec<String> {
    if current.trim().is_empty() {
        return handles.to_vec();
    }

    let typed_handle = current.contains("://");
    handles
        .iter()
        .filter(|h| {
            if typed_handle {
                h.starts_with(current)
            } else {
                h.split_once("://")
                    .map(|(_, name)| name.starts_with(current))
                    .unwrap_or(false)
            }
        })
        .cloned()
        .collect()
}

fn filter_slugs(handles: &[String], current: &str) -> Vec<String> {
    handles
        .iter()
        .filter_map(|handle| handle.split_once("://").map(|(_, name)| name.to_string()))
        .filter(|name| name.starts_with(current))
        .collect()
}

fn normalize_uri_template(uri_template: &str) -> String {
    uri_template.trim().trim_end_matches('/').to_string()
}

fn is_slug_like(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{
        CompleteRequestParams, CompletionContext, PromptReference, ResourceReference,
    };
    use specman::WorkspacePaths;
    use std::collections::HashMap;
    use std::io::Write;

    fn write_file(path: &Path, contents: &str) {
        let parent = path.parent().expect("parent");
        fs::create_dir_all(parent).expect("create parent");
        let mut file = fs::File::create(path).expect("create file");
        file.write_all(contents.as_bytes()).expect("write file");
    }

    fn fixture_workspace() -> (tempfile::TempDir, WorkspacePaths) {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        fs::create_dir_all(root.join(".specman")).expect(".specman");

        write_file(
            &root.join("spec/alpha/spec.md"),
            "---\nname: alpha\n---\n\n!concept-a.group:\n- MUST a\n",
        );
        write_file(
            &root.join("spec/omega/spec.md"),
            "---\nname: omega\n---\n\n!concept-z.group:\n- MUST z\n",
        );
        write_file(&root.join("impl/beta/impl.md"), "---\nname: beta\n---\n");
        write_file(&root.join("impl/gamma/impl.md"), "---\nname: gamma\n---\n");

        let ws = WorkspacePaths::new(root.to_path_buf(), root.join(".specman"));
        (temp, ws)
    }

    #[test]
    fn prompt_scope_enforces_revision_vs_feat() {
        let (_temp, ws) = fixture_workspace();

        let revision = CompleteRequestParams {
            meta: None,
            r#ref: Reference::Prompt(PromptReference {
                name: "revision".to_string(),
                title: None,
            }),
            argument: ArgumentInfo {
                name: "target".to_string(),
                value: "".to_string(),
            },
            context: None,
        };

        let feat = CompleteRequestParams {
            meta: None,
            r#ref: Reference::Prompt(PromptReference {
                name: "feat".to_string(),
                title: None,
            }),
            argument: ArgumentInfo {
                name: "target".to_string(),
                value: "".to_string(),
            },
            context: None,
        };

        let revision_values = complete_request(&ws, &revision).values;
        let feat_values = complete_request(&ws, &feat).values;

        assert_eq!(revision_values, vec!["spec://alpha", "spec://omega"]);
        assert_eq!(feat_values, vec!["impl://beta", "impl://gamma"]);
    }

    #[test]
    fn parity_prompt_and_resource_surfaces_share_scope() {
        let (_temp, ws) = fixture_workspace();

        let prompt = CompleteRequestParams {
            meta: None,
            r#ref: Reference::Prompt(PromptReference {
                name: "revision".to_string(),
                title: None,
            }),
            argument: ArgumentInfo {
                name: "target".to_string(),
                value: "".to_string(),
            },
            context: None,
        };
        let resource = CompleteRequestParams {
            meta: None,
            r#ref: Reference::Resource(ResourceReference {
                uri: "spec://{artifact}/constraints".to_string(),
            }),
            argument: ArgumentInfo {
                name: "artifact".to_string(),
                value: "".to_string(),
            },
            context: None,
        };

        let prompt_handles = complete_request(&ws, &prompt).values;
        let resource_names = complete_request(&ws, &resource).values;
        let from_resource: Vec<String> = resource_names
            .into_iter()
            .map(|name| format!("spec://{name}"))
            .collect();

        assert_eq!(prompt_handles, from_resource);
    }

    #[test]
    fn resource_constraint_id_completion_uses_context_artifact() {
        let (_temp, ws) = fixture_workspace();
        let mut args = HashMap::new();
        args.insert("artifact".to_string(), "alpha".to_string());

        let request = CompleteRequestParams {
            meta: None,
            r#ref: Reference::Resource(ResourceReference {
                uri: "spec://{artifact}/constraints/{constraint_id}".to_string(),
            }),
            argument: ArgumentInfo {
                name: "constraint_id".to_string(),
                value: "concept".to_string(),
            },
            context: Some(CompletionContext::with_arguments(args)),
        };

        let values = complete_request(&ws, &request).values;
        assert_eq!(values, vec!["concept-a.group"]);
    }

    #[test]
    fn malformed_inventory_returns_partial_results_and_warnings() {
        let (_temp, ws) = fixture_workspace();
        fs::create_dir_all(ws.spec_dir().join("BROKEN")).expect("create invalid spec dir");
        fs::create_dir_all(ws.impl_dir().join("delta")).expect("create impl dir without impl.md");

        let request = CompleteRequestParams {
            meta: None,
            r#ref: Reference::Prompt(PromptReference {
                name: "revision".to_string(),
                title: None,
            }),
            argument: ArgumentInfo {
                name: "target".to_string(),
                value: "".to_string(),
            },
            context: None,
        };

        let outcome = complete_request(&ws, &request);
        assert_eq!(outcome.values, vec!["spec://alpha", "spec://omega"]);
        assert!(!outcome.warnings.is_empty(), "expected degraded warnings");
    }
}
