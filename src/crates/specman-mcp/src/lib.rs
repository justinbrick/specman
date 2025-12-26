#![recursion_limit = "512"]

mod error;
mod prompts;
mod resources;
mod server;
mod tools;

pub use crate::error::McpError;
pub use crate::prompts::{
    ImplPromptArgs, MigrationPromptArgs, ScratchImplPromptArgs, ScratchSpecPromptArgs,
    SpecPromptArgs,
};
pub use crate::resources::{ArtifactInventory, ArtifactRecord};
pub use crate::server::{SpecmanMcpServer, run_stdio_server};
pub use crate::tools::{CreateArtifactResult, WorkspaceInfo};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    use rmcp::handler::server::ServerHandler;
    use rmcp::handler::server::wrapper::{Json, Parameters};
    use rmcp::model::PromptMessageContent;
    use rmcp::model::{PromptMessage, ResourceContents};
    use specman::{
        ArtifactId, ArtifactKind, ArtifactSummary, DependencyTree, WorkspaceLocator, WorkspacePaths,
    };
    use std::io::Write;
    use tempfile::TempDir;

    use crate::prompts::{ResolvedTarget, dependency_lines};
    use crate::resources::{
        artifact_handle, artifact_path, resource_templates, resources_from_inventory,
    };
    use crate::tools::{UpdateArtifactArgs, UpdateMode};

    #[tokio::test]
    async fn list_resources_includes_handles() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;
        let inventory = workspace.server.inventory().await?;
        let resources = resources_from_inventory(&inventory);

        let uris: Vec<String> = resources.into_iter().map(|r| r.raw.uri).collect();

        for expected in ["spec://testspec", "impl://testimpl"] {
            assert!(
                uris.contains(&expected.to_string()),
                "missing resource {expected}"
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn read_resource_returns_markdown_and_dependency_json()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let spec_body = workspace
            .server
            .read_resource_contents("spec://testspec")
            .await?;

        match spec_body {
            ResourceContents::TextResourceContents {
                mime_type, text, ..
            } => {
                assert_eq!(mime_type.as_deref(), Some("text/markdown"));
                assert!(text.contains("Spec Body"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }

        let deps = workspace
            .server
            .read_resource_contents("spec://testspec/dependencies")
            .await?;

        match deps {
            ResourceContents::TextResourceContents {
                mime_type, text, ..
            } => {
                assert_eq!(mime_type.as_deref(), Some("application/json"));
                let value: serde_json::Value = serde_json::from_str(&text)?;
                assert_eq!(value["root"]["id"]["name"], "testspec");
            }
            other => panic!("unexpected variant: {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn read_resource_constraints_index_returns_json() -> Result<(), Box<dyn std::error::Error>>
    {
        let workspace = TestWorkspace::create()?;

        let constraints = workspace
            .server
            .read_resource_contents("spec://testspec/constraints")
            .await?;

        match constraints {
            ResourceContents::TextResourceContents {
                mime_type, text, ..
            } => {
                assert_eq!(mime_type.as_deref(), Some("application/json"));
                let value: serde_json::Value = serde_json::from_str(&text)?;
                assert_eq!(value["artifact"], "spec://testspec");
                let items = value["constraints"]
                    .as_array()
                    .expect("constraints index should be a JSON array");

                assert_eq!(items.len(), 2, "expected two constraints");

                let first = &items[0];
                assert_eq!(first["constraint_id"], "concept-test.group");
                assert_eq!(first["identifier_line"], "!concept-test.group:");
                assert_eq!(
                    first["uri"],
                    "spec://testspec/constraints/concept-test.group"
                );

                let second = &items[1];
                assert_eq!(second["constraint_id"], "concept-test.other");
                assert_eq!(second["identifier_line"], "!concept-test.other:");
                assert_eq!(
                    second["uri"],
                    "spec://testspec/constraints/concept-test.other"
                );
            }
            other => panic!("unexpected variant: {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn read_resource_constraints_index_empty_spec_returns_empty_list()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let constraints = workspace
            .server
            .read_resource_contents("spec://empty/constraints")
            .await?;

        match constraints {
            ResourceContents::TextResourceContents {
                mime_type, text, ..
            } => {
                assert_eq!(mime_type.as_deref(), Some("application/json"));
                let value: serde_json::Value = serde_json::from_str(&text)?;
                assert_eq!(value["artifact"], "spec://empty");
                let items = value["constraints"]
                    .as_array()
                    .expect("constraints index should be a JSON array");
                assert!(items.is_empty(), "expected empty constraint list");
            }
            other => panic!("unexpected variant: {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn read_resource_constraints_index_missing_spec_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let err = match workspace
            .server
            .read_resource_contents("spec://does-not-exist/constraints")
            .await
        {
            Ok(_) => panic!("missing spec should fail"),
            Err(err) => err,
        };

        assert!(
            err.message.contains("not found") || err.message.contains("missing target"),
            "{err:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn read_resource_constraints_content_routes_and_validates()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let contents = workspace
            .server
            .read_resource_contents("spec://testspec/constraints/concept-test.group")
            .await?;

        match contents {
            ResourceContents::TextResourceContents {
                mime_type, text, ..
            } => {
                assert_eq!(mime_type.as_deref(), Some("text/markdown"));
                assert!(
                    text.contains("!concept-test.group:"),
                    "missing identifier line"
                );
                assert!(text.contains("MUST be indexable"));
                assert!(
                    !text.contains("!concept-test.other:"),
                    "must not include other constraint groups"
                );
                assert!(
                    !text.contains("MUST be discoverable"),
                    "must not include other constraint content"
                );
            }
            other => panic!("unexpected variant: {other:?}"),
        }
        Ok(())
    }

    #[tokio::test]
    async fn read_resource_constraints_content_missing_constraint_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let err = match workspace
            .server
            .read_resource_contents("spec://testspec/constraints/concept-test.missing")
            .await
        {
            Ok(_) => panic!("missing constraint should fail"),
            Err(err) => err,
        };

        assert!(err.message.contains("concept-test.missing"), "{err:?}");
        assert!(err.message.contains("spec://testspec"), "{err:?}");
        Ok(())
    }

    #[tokio::test]
    async fn read_resource_constraints_trailing_slash_routes_to_index()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let constraints = workspace
            .server
            .read_resource_contents("spec://testspec/constraints/")
            .await?;

        match constraints {
            ResourceContents::TextResourceContents { mime_type, .. } => {
                assert_eq!(mime_type.as_deref(), Some("application/json"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
        Ok(())
    }

    #[tokio::test]
    async fn read_resource_constraints_double_slash_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let err = match workspace
            .server
            .read_resource_contents("spec://testspec/constraints//")
            .await
        {
            Ok(_) => panic!("double slash should fail"),
            Err(err) => err,
        };

        assert!(err.message.contains("double slash"), "{err:?}");
        Ok(())
    }

    #[tokio::test]
    async fn read_resource_constraints_rejects_non_spec_scheme()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let err = match workspace
            .server
            .read_resource_contents("impl://testimpl/constraints")
            .await
        {
            Ok(_) => panic!("non-spec constraints should fail"),
            Err(err) => err,
        };

        assert!(
            err.message.contains("only available") || err.message.contains("spec://"),
            "{err:?}"
        );

        Ok(())
    }

    #[test]
    fn get_info_enables_prompts() {
        let server = SpecmanMcpServer::new().expect("server should start");
        let info = server.get_info();

        assert!(info.capabilities.prompts.is_some());
    }

    #[tokio::test]
    async fn scratch_feat_prompt_renders_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;
        let message = workspace
            .server
            .scratch_feat_prompt(Parameters(ScratchImplPromptArgs {
                target: "testimpl".to_string(),
            }))
            .await?
            .pop()
            .expect("prompt returns one message");

        let rendered = match message.content {
            PromptMessageContent::Text { text } => text,
            other => panic!("unexpected content: {other:?}"),
        };

        assert!(rendered.contains("git checkout -b"));
        assert!(rendered.contains("impl/testimpl/impl.md"));

        Ok(())
    }

    #[tokio::test]
    async fn scratch_ref_prompt_renders_dependencies() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;
        let rendered = workspace
            .server
            .scratch_ref_prompt(Parameters(ScratchImplPromptArgs {
                target: "testimpl".to_string(),
            }))
            .await?
            .pop()
            .expect("prompt returns one message")
            .content;

        let text = match rendered {
            PromptMessageContent::Text { text } => text,
            other => panic!("unexpected content: {other:?}"),
        };

        assert!(text.contains("impl/testimpl/impl.md"));
        assert!(text.contains("spec://testspec"));

        Ok(())
    }

    #[tokio::test]
    async fn scratch_revision_prompt_uses_spec_target() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;
        let rendered = workspace
            .server
            .scratch_revision_prompt(Parameters(ScratchSpecPromptArgs {
                target: "testspec".to_string(),
            }))
            .await?
            .pop()
            .expect("prompt returns one message")
            .content;

        let text = match rendered {
            PromptMessageContent::Text { text } => text,
            other => panic!("unexpected content: {other:?}"),
        };

        assert!(text.contains("spec/testspec/spec.md"));
        assert!(text.contains("git checkout -b"));

        Ok(())
    }

    #[tokio::test]
    async fn scratch_fix_prompt_defaults_branch() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;
        let rendered = workspace
            .server
            .scratch_fix_prompt(Parameters(ScratchImplPromptArgs {
                target: "testimpl".to_string(),
            }))
            .await?
            .pop()
            .expect("prompt returns one message")
            .content;

        let text = match rendered {
            PromptMessageContent::Text { text } => text,
            other => panic!("unexpected content: {other:?}"),
        };

        assert!(text.contains("impl/testimpl/impl.md"));
        assert!(text.contains("git checkout -b"));

        Ok(())
    }

    #[test]
    fn dependency_lines_deduplicates_handle_entries() -> Result<(), Box<dyn std::error::Error>> {
        use specman::{DependencyEdge, DependencyRelation};

        let temp = tempfile::tempdir()?;
        let root_path = temp.path();

        fs::create_dir_all(root_path.join(".specman"))?;
        fs::create_dir_all(root_path.join("impl/rootimpl"))?;
        fs::create_dir_all(root_path.join("spec/dep"))?;

        let workspace = WorkspacePaths::new(root_path.to_path_buf(), root_path.join(".specman"));

        let root_id = ArtifactId {
            kind: ArtifactKind::Implementation,
            name: "rootimpl".to_string(),
        };

        let dep_id = ArtifactId {
            kind: ArtifactKind::Specification,
            name: "dep".to_string(),
        };

        let root_summary = ArtifactSummary {
            id: root_id.clone(),
            ..Default::default()
        };

        let dep_summary = ArtifactSummary {
            id: dep_id.clone(),
            ..Default::default()
        };

        let edge = DependencyEdge {
            from: root_summary.clone(),
            to: dep_summary.clone(),
            relation: DependencyRelation::Upstream,
            optional: false,
        };

        let tree = DependencyTree {
            root: root_summary.clone(),
            upstream: vec![edge.clone(), edge],
            ..Default::default()
        };

        let resolved = ResolvedTarget {
            tree,
            workspace: workspace.clone(),
            handle: artifact_handle(&root_summary),
            path: artifact_path(&root_id, &workspace).display().to_string(),
        };

        let lines = dependency_lines(&resolved);

        assert!(
            lines
                .first()
                .expect("root line exists")
                .starts_with("- impl://rootimpl ("),
            "root entry must remain in list"
        );

        let dep_count = lines
            .iter()
            .filter(|line| line.contains("spec://dep"))
            .count();
        assert_eq!(dep_count, 1, "dependency handle should appear only once");

        Ok(())
    }

    #[tokio::test]
    async fn prompts_render_normalized_handles() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let feat_text = prompt_text(
            workspace
                .server
                .scratch_feat_prompt(Parameters(ScratchImplPromptArgs {
                    target: "testimpl".to_string(),
                }))
                .await?,
        );

        assert!(
            feat_text.contains("impl://testimpl"),
            "rendered prompt must include normalized impl locator: {feat_text}"
        );
        assert!(
            !feat_text.contains("{{target_path}}"),
            "rendered prompt must not leak target_path token: {feat_text}"
        );

        let revision_text = prompt_text(
            workspace
                .server
                .scratch_revision_prompt(Parameters(ScratchSpecPromptArgs {
                    target: "testspec".to_string(),
                }))
                .await?,
        );

        assert!(
            revision_text.contains("spec://testspec"),
            "rendered revision prompt must include normalized spec locator: {revision_text}"
        );
        assert!(
            !revision_text.contains("{{target_path}}"),
            "rendered revision prompt must not leak target_path token: {revision_text}"
        );

        let impl_text = prompt_text(
            workspace
                .server
                .impl_prompt(Parameters(ImplPromptArgs {
                    spec: "spec://testspec".to_string(),
                }))
                .await?,
        );

        assert!(
            impl_text.contains("spec://testspec"),
            "rendered impl prompt must include normalized spec locator: {impl_text}"
        );
        assert!(
            !impl_text.contains("{{target_path}}"),
            "rendered impl prompt must not leak target_path token: {impl_text}"
        );

        let migration_text = prompt_text(
            workspace
                .server
                .migration_prompt(Parameters(MigrationPromptArgs {
                    target: "testspec".to_string(),
                }))
                .await?,
        );

        assert!(
            migration_text.contains("spec://testspec"),
            "rendered migration prompt must include normalized spec locator: {migration_text}"
        );
        assert!(
            !migration_text.contains("{{target_path}}"),
            "rendered migration prompt must not leak target_path token: {migration_text}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn prompts_clear_template_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let renderings = vec![
            (
                "feat",
                workspace
                    .server
                    .scratch_feat_prompt(Parameters(ScratchImplPromptArgs {
                        target: "testimpl".to_string(),
                    }))
                    .await?,
            ),
            (
                "ref",
                workspace
                    .server
                    .scratch_ref_prompt(Parameters(ScratchImplPromptArgs {
                        target: "testimpl".to_string(),
                    }))
                    .await?,
            ),
            (
                "revision",
                workspace
                    .server
                    .scratch_revision_prompt(Parameters(ScratchSpecPromptArgs {
                        target: "testspec".to_string(),
                    }))
                    .await?,
            ),
            (
                "fix",
                workspace
                    .server
                    .scratch_fix_prompt(Parameters(ScratchImplPromptArgs {
                        target: "testimpl".to_string(),
                    }))
                    .await?,
            ),
            (
                "spec",
                workspace
                    .server
                    .spec_prompt(Parameters(SpecPromptArgs {}))
                    .await?,
            ),
            (
                "impl",
                workspace
                    .server
                    .impl_prompt(Parameters(ImplPromptArgs {
                        spec: "testspec".to_string(),
                    }))
                    .await?,
            ),
            (
                "migration",
                workspace
                    .server
                    .migration_prompt(Parameters(MigrationPromptArgs {
                        target: "testspec".to_string(),
                    }))
                    .await?,
            ),
        ];

        for (name, messages) in renderings {
            let text = prompt_text(messages);
            assert!(
                !text.contains("{{"),
                "prompt {name} retained unresolved tokens: {text}"
            );
        }

        Ok(())
    }

    #[test]
    fn prompt_router_lists_all_prompts() {
        let server = SpecmanMcpServer::new().expect("server should start");
        let prompts = server.prompt_router.list_all();
        let names: std::collections::HashSet<_> = prompts.iter().map(|p| p.name.as_str()).collect();

        for expected in [
            "feat",
            "ref",
            "revision",
            "fix",
            "spec",
            "impl",
            "migration",
        ] {
            assert!(names.contains(expected), "missing prompt {expected}");
        }
    }

    #[tokio::test]
    async fn migration_prompt_guides_spec_first_flow_and_phase_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;
        let text = prompt_text(
            workspace
                .server
                .migration_prompt(Parameters(MigrationPromptArgs {
                    target: "testspec".to_string(),
                }))
                .await?,
        );

        let spec_instruction = text
            .find("create_artifact` to create the specification artifact")
            .expect("migration prompt should instruct spec creation first");
        let scratch_instruction = text
            .find("revision scratch pad")
            .expect("migration prompt should instruct revision scratch pad creation");
        let impl_instruction = text
            .find("create an implementation targeting that specification")
            .expect("migration prompt should instruct implementation creation");
        let feat_instruction = text
            .find("feature scratch pad targeting the implementation")
            .expect("migration prompt should instruct feat scratch pad creation");
        assert!(
            spec_instruction < scratch_instruction,
            "spec creation instruction must precede scratch pad creation"
        );
        assert!(
            scratch_instruction < impl_instruction,
            "revision scratch pad creation must precede implementation creation"
        );
        assert!(
            impl_instruction < feat_instruction,
            "implementation creation must precede feature scratch pad creation"
        );

        let phase1 = text
            .find("Phase 1 - Enumerate sources")
            .expect("phase 1 must be present");
        let phase2 = text
            .find("Phase 2 - Extract findings")
            .expect("phase 2 must be present");
        let phase3 = text
            .find("Phase 3 - Draft/update specification")
            .expect("phase 3 must be present");
        let phase4 = text
            .find("Phase 4 - Generate implementation documentation")
            .expect("phase 4 must be present");

        assert!(phase1 < phase2 && phase2 < phase3 && phase3 < phase4);

        Ok(())
    }

    #[test]
    fn resource_templates_cover_expected_handles() {
        let templates = resource_templates();
        let uris: Vec<String> = templates.into_iter().map(|t| t.raw.uri_template).collect();

        for expected in [
            "spec://{artifact}",
            "impl://{artifact}",
            "scratch://{artifact}",
            "spec://{artifact}/dependencies",
            "spec://{artifact}/constraints",
            "spec://{artifact}/constraints/{constraint_id}",
            "impl://{artifact}/dependencies",
            "scratch://{artifact}/dependencies",
        ] {
            assert!(
                uris.contains(&expected.to_string()),
                "missing template {expected}"
            );
        }
    }

    #[test]
    fn create_artifact_schema_is_deterministic() {
        let schema_a = rmcp::schemars::schema_for!(crate::tools::CreateArtifactArgs);
        let schema_b = rmcp::schemars::schema_for!(crate::tools::CreateArtifactArgs);

        let a = serde_json::to_string(&schema_a).expect("schema serializes");
        let b = serde_json::to_string(&schema_b).expect("schema serializes");

        assert_eq!(a, b, "schema serialization must be deterministic");
    }

    #[test]
    fn tool_input_schemas_avoid_forbidden_json_schema_features() {
        fn assert_no_forbidden_keys(schema: &impl serde::Serialize, label: &str) {
            let json = serde_json::to_string(schema).expect("schema serializes");

            for forbidden in [
                "\"oneOf\"",
                "\"anyOf\"",
                "\"$ref\"",
                "\"patternProperties\"",
            ] {
                assert!(
                    !json.contains(forbidden),
                    "{label} schema must not contain forbidden key {forbidden}: {json}"
                );
            }
        }

        let create_schema = rmcp::schemars::schema_for!(crate::tools::CreateArtifactArgs);
        assert_no_forbidden_keys(&create_schema, "create_artifact");

        let update_schema = rmcp::schemars::schema_for!(crate::tools::UpdateArtifactArgs);
        assert_no_forbidden_keys(&update_schema, "update_artifact");
    }

    #[test]
    fn create_artifact_input_schema_is_object_type() {
        let schema = rmcp::schemars::schema_for!(crate::tools::CreateArtifactArgs);
        let value = serde_json::to_value(&schema).expect("schema serializes");
        let ty = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        assert_eq!(
            ty, "object",
            "MCP Inspector expects tool inputSchema.type to be exactly 'object'"
        );
    }

    #[test]
    fn create_artifact_input_schema_has_field_descriptions() {
        let schema = rmcp::schemars::schema_for!(crate::tools::CreateArtifactArgs);
        let value = serde_json::to_value(&schema).expect("schema serializes");

        fn collect_property_schemas<'a>(
            value: &'a serde_json::Value,
            key: &str,
            out: &mut Vec<&'a serde_json::Value>,
        ) {
            match value {
                serde_json::Value::Object(map) => {
                    if let Some(props) = map.get("properties").and_then(|v| v.as_object()) {
                        if let Some(schema) = props.get(key) {
                            out.push(schema);
                        }
                    }

                    for v in map.values() {
                        collect_property_schemas(v, key, out);
                    }
                }
                serde_json::Value::Array(arr) => {
                    for v in arr {
                        collect_property_schemas(v, key, out);
                    }
                }
                _ => {}
            }
        }

        fn assert_all_described(value: &serde_json::Value, key: &str) {
            let mut schemas = Vec::new();
            collect_property_schemas(value, key, &mut schemas);
            assert!(!schemas.is_empty(), "expected at least one '{key}' schema");

            for schema in schemas {
                let described = schema
                    .as_object()
                    .and_then(|m| m.get("description"))
                    .and_then(|d| d.as_str())
                    .is_some();

                assert!(
                    described,
                    "expected '{key}' schema to contain a description: {schema}"
                );
            }
        }

        // New externally-tagged schema: top-level variant keys plus inner common fields.
        for key in [
            "specification",
            "implementation",
            "scratch_pad",
            "target",
            "scratchKind",
            "intent",
            "name",
            "title",
        ] {
            assert_all_described(&value, key);
        }
    }

    #[test]
    fn list_tools_returns_object_input_schema() {
        let server = SpecmanMcpServer::new().expect("server should start");
        let tools = server.tool_router.list_all();

        assert!(!tools.is_empty(), "expected at least one tool");

        for tool in tools {
            let tool_value = serde_json::to_value(&tool).expect("tool serializes");
            let schema = tool_value.get("inputSchema").expect("tool has inputSchema");

            let ty = schema
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            assert_eq!(
                ty, "object",
                "tool '{}' inputSchema.type must be 'object' for MCP Inspector",
                tool.name
            );
        }
    }

    #[tokio::test]
    async fn create_artifact_normalizes_scratchpad_target() -> Result<(), Box<dyn std::error::Error>>
    {
        let workspace = TestWorkspace::create()?;

        let Json(result) = workspace
            .server
            .create_artifact_internal(
                None,
                crate::tools::CreateArtifactArgs::ScratchPad {
                    target: "impl://testimpl".to_string(),
                    scratch_kind: crate::tools::ScratchKind::Feat,
                    intent: "Create a scratch pad to plan a small feature".to_string(),
                },
            )
            .await
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, format!("{err:?}")))?;

        assert!(
            result.handle.starts_with("scratch://"),
            "expected scratch handle, got {}",
            result.handle
        );
        assert!(
            result.path.starts_with(".specman/scratchpad/"),
            "expected scratchpad path, got {}",
            result.path
        );
        assert!(
            result.path.ends_with("/scratch.md"),
            "expected scratch.md, got {}",
            result.path
        );

        let workspace_paths = workspace.server.workspace.workspace()?;
        let absolute = workspace_paths.root().join(&result.path);
        let content = fs::read_to_string(&absolute)?;

        assert!(
            content.contains("target: impl/testimpl/impl.md"),
            "scratchpad front matter must use normalized workspace-relative path"
        );
        assert!(
            !content.contains("target: impl://testimpl"),
            "scratchpad front matter must not retain the handle"
        );

        Ok(())
    }

    #[tokio::test]
    async fn create_artifact_normalizes_implementation_target()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let Json(result) = workspace
            .server
            .create_artifact_internal(
                None,
                crate::tools::CreateArtifactArgs::Implementation {
                    target: "spec://testspec".to_string(),
                    intent: None,
                    name: Some("mcpimpl".to_string()),
                },
            )
            .await
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, format!("{err:?}")))?;

        assert_eq!(result.handle, "impl://mcpimpl");
        assert_eq!(result.path, "impl/mcpimpl/impl.md");

        let workspace_paths = workspace.server.workspace.workspace()?;
        let absolute = workspace_paths.root().join(&result.path);
        let content = fs::read_to_string(&absolute)?;

        assert!(
            content.contains("spec: ../../spec/testspec/spec.md"),
            "impl front matter must use a normalized path (relative to the impl artifact)"
        );
        assert!(
            !content.contains("spec: spec://testspec"),
            "impl front matter must not retain the handle"
        );

        Ok(())
    }

    #[tokio::test]
    async fn create_artifact_rejects_url_locators() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let result = workspace
            .server
            .create_artifact_internal(
                None,
                crate::tools::CreateArtifactArgs::ScratchPad {
                    target: "https://example.com".to_string(),
                    scratch_kind: crate::tools::ScratchKind::Feat,
                    intent: "Try to create a scratch pad from an external URL".to_string(),
                },
            )
            .await;

        let err = result.err().expect("url targets must be rejected");

        assert!(
            err.message.contains("must not be URLs"),
            "unexpected error: {err:?}"
        );

        Ok(())
    }

    fn prompt_text(messages: Vec<PromptMessage>) -> String {
        let message = messages
            .into_iter()
            .next()
            .expect("prompt returns one message");

        match message.content {
            PromptMessageContent::Text { text } => text,
            other => panic!("unexpected content: {other:?}"),
        }
    }

    fn body_of(doc: &str) -> &str {
        specman::front_matter::split_front_matter(doc)
            .expect("front matter split")
            .body
    }

    struct TestWorkspace {
        _temp: TempDir,
        server: SpecmanMcpServer,
    }

    impl TestWorkspace {
        fn create() -> Result<Self, Box<dyn std::error::Error>> {
            let temp = tempfile::tempdir()?;

            create_workspace_files(temp.path())?;

            let server = SpecmanMcpServer::new_with_root(temp.path())?;

            Ok(Self {
                _temp: temp,
                server,
            })
        }
    }

    #[tokio::test]
    async fn update_artifact_preview_does_not_persist_and_preserves_body()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;
        let root = workspace._temp.path();

        let path = root.join("spec/testspec/spec.md");
        let before = fs::read_to_string(&path)?;

        let result = workspace
            .server
            .update_artifact(rmcp::handler::server::wrapper::Parameters(
                UpdateArtifactArgs::Spec(crate::tools::UpdateArtifactVariantArgs {
                    locator: "spec://testspec".to_string(),
                    mode: UpdateMode::Preview,
                    ops: vec![specman::FrontMatterUpdateOp::SetVersion {
                        version: "0.2.0".to_string(),
                    }],
                }),
            ))
            .await?;

        assert!(!result.0.persisted);
        assert_eq!(body_of(&before), body_of(&result.0.updated_document));

        let after_on_disk = fs::read_to_string(&path)?;
        assert_eq!(before, after_on_disk, "preview must not write to disk");

        Ok(())
    }

    #[tokio::test]
    async fn update_artifact_persist_writes_and_preserves_body()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;
        let root = workspace._temp.path();

        let path = root.join("impl/testimpl/impl.md");
        let before = fs::read_to_string(&path)?;

        let result = workspace
            .server
            .update_artifact(rmcp::handler::server::wrapper::Parameters(
                UpdateArtifactArgs::Impl(crate::tools::UpdateArtifactVariantArgs {
                    locator: "impl://testimpl".to_string(),
                    mode: UpdateMode::Persist,
                    ops: vec![specman::FrontMatterUpdateOp::AddTag {
                        tag: "mcp".to_string(),
                    }],
                }),
            ))
            .await?;

        assert!(result.0.persisted);
        assert_eq!(body_of(&before), body_of(&result.0.updated_document));

        let after_on_disk = fs::read_to_string(&path)?;
        assert!(after_on_disk.contains("mcp"));
        assert_eq!(body_of(&before), body_of(&after_on_disk));

        Ok(())
    }

    #[tokio::test]
    async fn update_artifact_rejects_kind_mismatch() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let err = match workspace
            .server
            .update_artifact(rmcp::handler::server::wrapper::Parameters(
                UpdateArtifactArgs::Impl(crate::tools::UpdateArtifactVariantArgs {
                    locator: "spec://testspec".to_string(),
                    mode: UpdateMode::Preview,
                    ops: vec![specman::FrontMatterUpdateOp::SetVersion {
                        version: "0.2.0".to_string(),
                    }],
                }),
            ))
            .await
        {
            Ok(_) => panic!("kind mismatch should fail"),
            Err(err) => err,
        };

        assert!(err.message.contains("kind mismatch"), "{err:?}");
        Ok(())
    }

    #[tokio::test]
    async fn update_artifact_rejects_scratch_target_mutation()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let err = match workspace
            .server
            .update_artifact(rmcp::handler::server::wrapper::Parameters(
                UpdateArtifactArgs::Scratch(crate::tools::UpdateArtifactVariantArgs {
                    locator: "scratch://testscratch".to_string(),
                    mode: UpdateMode::Preview,
                    ops: vec![specman::FrontMatterUpdateOp::SetTarget {
                        target: "spec://testspec".to_string(),
                    }],
                }),
            ))
            .await
        {
            Ok(_) => panic!("scratch target mutation should fail"),
            Err(err) => err,
        };

        assert!(
            err.message.contains("immutable"),
            "unexpected error: {err:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn update_artifact_rejects_https_persist() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let err = match workspace
            .server
            .update_artifact(rmcp::handler::server::wrapper::Parameters(
                UpdateArtifactArgs::Spec(crate::tools::UpdateArtifactVariantArgs {
                    locator: "https://example.com/spec.md".to_string(),
                    mode: UpdateMode::Persist,
                    ops: vec![specman::FrontMatterUpdateOp::SetVersion {
                        version: "0.2.0".to_string(),
                    }],
                }),
            ))
            .await
        {
            Ok(_) => panic!("https persist should fail"),
            Err(err) => err,
        };

        assert!(err.message.contains("persist is not supported"), "{err:?}");
        Ok(())
    }

    fn create_workspace_files(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let spec_dir = root.join("spec/testspec");
        let empty_spec_dir = root.join("spec/empty");
        let impl_dir = root.join("impl/testimpl");
        let scratch_dir = root.join(".specman/scratchpad/testscratch");
        let template_dir = root.join("templates");
        let dot_specman_templates = root.join(".specman/templates");

        fs::create_dir_all(&spec_dir)?;
        fs::create_dir_all(&empty_spec_dir)?;
        fs::create_dir_all(&impl_dir)?;
        fs::create_dir_all(&scratch_dir)?;
        fs::create_dir_all(&template_dir)?;
        fs::create_dir_all(&dot_specman_templates)?;

        fs::write(
            spec_dir.join("spec.md"),
            r"---
name: testspec
version: '0.1.0'
dependencies: []
---

# Spec Body

## Constraints

!concept-test.group:
- MUST be indexable

!concept-test.other:
- MUST be discoverable
",
        )?;

        fs::write(
            empty_spec_dir.join("spec.md"),
            r"---
name: empty
version: '0.1.0'
dependencies: []
---

# Empty Spec
",
        )?;

        fs::write(
            impl_dir.join("impl.md"),
            r"---
spec: spec://testspec
name: testimpl
version: '0.1.0'
primary_language:
  language: rust@1.0
---

# Impl Body
",
        )?;

        let scratch_content = r"---
target: impl://testimpl
branch: main
work_type:
  feat: {}
---

# Scratch Body
";

        let mut scratch_file = fs::File::create(scratch_dir.join("scratch.md"))?;
        scratch_file.write_all(scratch_content.as_bytes())?;

        fs::create_dir_all(root.join(".specman"))?;

        // Provide tokenized templates for core `Specman::create` so it doesn't fall back to
        // embedded placeholder templates that reference non-existent artifacts.
        fs::write(
            template_dir.join("impl.md"),
            r"---
spec: {{target_path}}
name: {{output_name}}
version: '0.1.0'
---

# Impl — {{output_name}}
",
        )?;

        fs::write(
            template_dir.join("scratch.md"),
            r"---
target: {{target_path}}
branch: main
work_type:
    {{work_type_kind}}: {}
---

# Scratch Pad — {{output_name}}
",
        )?;

        // Point the workspace template pointers at the tokenized templates.
        fs::write(dot_specman_templates.join("IMPL"), "templates/impl.md\n")?;
        fs::write(
            dot_specman_templates.join("SCRATCH"),
            "templates/scratch.md\n",
        )?;

        Ok(())
    }
}
