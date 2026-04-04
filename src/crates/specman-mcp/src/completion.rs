use rmcp::model::{CompleteRequestParams, Reference};

use specman::WorkspacePaths;

mod contracts;
mod index;
mod matching;
mod serialization;

use contracts::HandleScope;
use index::{build_index, complete_constraint_ids};
use matching::{filter_handles_fuzzy, filter_slugs_fuzzy};

pub(crate) use serialization::capability_descriptor_metadata;

pub(crate) struct CompletionOutcome {
    pub(crate) values: Vec<String>,
    pub(crate) warnings: Vec<String>,
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
    index: &index::CompletionIndex,
    prompt_name: &str,
    argument: &rmcp::model::ArgumentInfo,
    _context: Option<&rmcp::model::CompletionContext>,
) -> Vec<String> {
    let scope = contracts::prompt_scope(prompt_name, &argument.name);

    match scope {
        Some(HandleScope::SpecOnly) => filter_handles_fuzzy(&index.specs, &argument.value),
        Some(HandleScope::ImplOnly) => filter_handles_fuzzy(&index.impls, &argument.value),
        None => Vec::new(),
    }
}

fn complete_resource(
    index: &index::CompletionIndex,
    workspace: &WorkspacePaths,
    uri_template: &str,
    argument: &rmcp::model::ArgumentInfo,
    context: Option<&rmcp::model::CompletionContext>,
) -> Vec<String> {
    let normalized = normalize_uri_template(uri_template);

    if argument.name == "artifact" {
        let scope = contracts::resource_scope_for_artifact(&normalized);

        return match scope {
            Some(HandleScope::SpecOnly) => filter_slugs_fuzzy(&index.specs, &argument.value),
            Some(HandleScope::ImplOnly) => filter_slugs_fuzzy(&index.impls, &argument.value),
            None => Vec::new(),
        };
    }

    if contracts::is_constraint_id_surface(&normalized, &argument.name) {
        let artifact = context
            .and_then(|ctx| ctx.get_argument("artifact"))
            .map(String::as_str)
            .unwrap_or_default();
        return complete_constraint_ids(workspace, artifact, &argument.value);
    }

    Vec::new()
}
fn normalize_uri_template(uri_template: &str) -> String {
    uri_template.trim().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{
        ArgumentInfo, CompleteRequestParams, CompletionContext, PromptReference, ResourceReference,
    };
    use specman::WorkspacePaths;
    use std::collections::HashMap;
    use std::fs;
    use std::io::Write;
    use std::path::Path;

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
    fn prompt_completion_uses_fuzzy_matching() {
        let (_temp, ws) = fixture_workspace();

        let request = CompleteRequestParams {
            meta: None,
            r#ref: Reference::Prompt(PromptReference {
                name: "revision".to_string(),
                title: None,
            }),
            argument: ArgumentInfo {
                name: "target".to_string(),
                value: "oa".to_string(),
            },
            context: None,
        };

        let values = complete_request(&ws, &request).values;
        assert_eq!(values, vec!["spec://omega"]);
    }

    #[test]
    fn resource_slug_completion_uses_fuzzy_matching() {
        let (_temp, ws) = fixture_workspace();

        let request = CompleteRequestParams {
            meta: None,
            r#ref: Reference::Resource(ResourceReference {
                uri: "impl://{artifact}".to_string(),
            }),
            argument: ArgumentInfo {
                name: "artifact".to_string(),
                value: "gm".to_string(),
            },
            context: None,
        };

        let values = complete_request(&ws, &request).values;
        assert_eq!(values, vec!["gamma"]);
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
