mod error;
mod prompts;
mod resources;
mod server;
mod tools;

pub use crate::error::McpError;
pub use crate::prompts::{ImplPromptArgs, ScratchPromptArgs, SpecPromptArgs};
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
    use specman::front_matter::{ScratchWorkType, ScratchWorkloadExtras};
    use specman::{
        ArtifactId, ArtifactKind, ArtifactSummary, CreateRequest, DependencyTree, WorkspaceLocator,
        WorkspacePaths,
    };
    use std::io::Write;
    use tempfile::TempDir;

    use crate::prompts::{ResolvedTarget, dependency_lines};
    use crate::resources::{
        artifact_handle, artifact_path, resource_templates, resources_from_inventory,
    };

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
            .scratch_feat_prompt(Parameters(ScratchPromptArgs {
                target: "impl://testimpl".to_string(),
                branch_name: None,
            }))
            .await?
            .pop()
            .expect("prompt returns one message");

        let rendered = match message.content {
            PromptMessageContent::Text { text } => text,
            other => panic!("unexpected content: {other:?}"),
        };

        assert!(rendered.contains("Create and check out a branch"));
        assert!(rendered.contains("impl/testimpl/impl.md"));

        Ok(())
    }

    #[tokio::test]
    async fn scratch_ref_prompt_renders_dependencies() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;
        let rendered = workspace
            .server
            .scratch_ref_prompt(Parameters(ScratchPromptArgs {
                target: "impl://testimpl".to_string(),
                branch_name: None,
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
            .scratch_revision_prompt(Parameters(ScratchPromptArgs {
                target: "spec://testspec".to_string(),
                branch_name: Some("custom-revision".to_string()),
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
        assert!(text.contains("Check out the provided branch \"custom-revision\""));

        Ok(())
    }

    #[tokio::test]
    async fn scratch_fix_prompt_defaults_branch() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;
        let rendered = workspace
            .server
            .scratch_fix_prompt(Parameters(ScratchPromptArgs {
                target: "impl://testimpl".to_string(),
                branch_name: None,
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
        assert!(text.contains("Create and check out a branch"));

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
                .scratch_feat_prompt(Parameters(ScratchPromptArgs {
                    target: "impl://testimpl".to_string(),
                    branch_name: None,
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
                .scratch_revision_prompt(Parameters(ScratchPromptArgs {
                    target: "spec://testspec".to_string(),
                    branch_name: None,
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
                    .scratch_feat_prompt(Parameters(ScratchPromptArgs {
                        target: "impl://testimpl".to_string(),
                        branch_name: None,
                    }))
                    .await?,
            ),
            (
                "ref",
                workspace
                    .server
                    .scratch_ref_prompt(Parameters(ScratchPromptArgs {
                        target: "impl://testimpl".to_string(),
                        branch_name: None,
                    }))
                    .await?,
            ),
            (
                "revision",
                workspace
                    .server
                    .scratch_revision_prompt(Parameters(ScratchPromptArgs {
                        target: "spec://testspec".to_string(),
                        branch_name: None,
                    }))
                    .await?,
            ),
            (
                "fix",
                workspace
                    .server
                    .scratch_fix_prompt(Parameters(ScratchPromptArgs {
                        target: "impl://testimpl".to_string(),
                        branch_name: None,
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
                        spec: "spec://testspec".to_string(),
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

        for expected in ["feat", "ref", "revision", "fix", "spec", "impl"] {
            assert!(names.contains(expected), "missing prompt {expected}");
        }
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
        let schema_a = rmcp::schemars::schema_for!(CreateRequest);
        let schema_b = rmcp::schemars::schema_for!(CreateRequest);

        let a = serde_json::to_string(&schema_a).expect("schema serializes");
        let b = serde_json::to_string(&schema_b).expect("schema serializes");

        assert_eq!(a, b, "schema serialization must be deterministic");
    }

    #[tokio::test]
    async fn create_artifact_normalizes_scratchpad_target() -> Result<(), Box<dyn std::error::Error>>
    {
        let workspace = TestWorkspace::create()?;

        let Json(result) = workspace
            .server
            .create_artifact(Parameters(CreateRequest::ScratchPad {
                context: specman::ScratchPadCreateContext {
                    name: "mcpscratch".to_string(),
                    target: "impl://testimpl".to_string(),
                    work_type: ScratchWorkType::Feat(ScratchWorkloadExtras::default()),
                },
            }))
            .await?;

        assert_eq!(result.handle, "scratch://mcpscratch");
        assert_eq!(result.path, ".specman/scratchpad/mcpscratch/scratch.md");

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
            .create_artifact(Parameters(CreateRequest::Implementation {
                context: specman::ImplContext {
                    name: "mcpimpl".to_string(),
                    target: "spec://testspec".to_string(),
                },
            }))
            .await?;

        assert_eq!(result.handle, "impl://mcpimpl");
        assert_eq!(result.path, "impl/mcpimpl/impl.md");

        let workspace_paths = workspace.server.workspace.workspace()?;
        let absolute = workspace_paths.root().join(&result.path);
        let content = fs::read_to_string(&absolute)?;

        assert!(
            content.contains("spec: spec://testspec"),
            "impl front matter must use a canonical spec handle"
        );
        assert!(
            !content.contains("spec: spec/testspec/spec.md"),
            "impl front matter must not embed a workspace path"
        );

        Ok(())
    }

    #[tokio::test]
    async fn create_artifact_rejects_url_locators() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;

        let result = workspace
            .server
            .create_artifact(Parameters(CreateRequest::ScratchPad {
                context: specman::ScratchPadCreateContext {
                    name: "bad".to_string(),
                    target: "https://example.com".to_string(),
                    work_type: ScratchWorkType::Feat(ScratchWorkloadExtras::default()),
                },
            }))
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

    fn create_workspace_files(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let spec_dir = root.join("spec/testspec");
        let impl_dir = root.join("impl/testimpl");
        let scratch_dir = root.join(".specman/scratchpad/testscratch");
        let template_dir = root.join("templates");
        let dot_specman_templates = root.join(".specman/templates");

        fs::create_dir_all(&spec_dir)?;
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
spec: {{target}}
name: {{name}}
version: '0.1.0'
---

# Impl — {{name}}
",
        )?;

        fs::write(
            template_dir.join("scratch.md"),
            r"---
target: {{target}}
branch: main
work_type:
    {{work_type_kind}}: {}
---

# Scratch Pad — {{name}}
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
