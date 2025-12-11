use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::handler::server::{
    ServerHandler, router::Router, router::prompt::PromptRouter, tool::ToolRouter, wrapper::Json,
    wrapper::Parameters,
};
use rmcp::schemars::JsonSchema;
use rmcp::service::{RequestContext, RoleServer, ServerInitializeError};
use rmcp::{
    model::{
        ErrorData, GetPromptRequestParam, GetPromptResult, ListPromptsResult,
        ListResourceTemplatesResult, ListResourcesResult, PaginatedRequestParam, PromptMessage,
        PromptMessageRole, RawResource, RawResourceTemplate, ReadResourceRequestParam,
        ReadResourceResult, Resource, ResourceContents, ResourceTemplate, ServerCapabilities,
        ServerInfo,
    },
    prompt, prompt_handler, prompt_router,
    service::ServiceExt,
    tool, tool_router, transport,
};
use serde::{Deserialize, Serialize};
use specman::{
    ArtifactId, ArtifactKind, ArtifactSummary, DependencyTree, FilesystemDependencyMapper,
    FilesystemWorkspaceLocator, SemVer, SpecmanError, WorkspaceLocator, WorkspacePaths,
};

const SCRATCH_FEAT_TEMPLATE: &str = include_str!("../templates/prompts/scratch-feat.md");
const SCRATCH_FIX_TEMPLATE: &str = include_str!("../templates/prompts/scratch-fix.md");
const SCRATCH_REF_TEMPLATE: &str = include_str!("../templates/prompts/scratch-ref.md");
const SCRATCH_REVISION_TEMPLATE: &str = include_str!("../templates/prompts/scratch-revision.md");

/// Structured workspace data exposed over MCP tools.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceInfo {
    pub root: String,
    pub dot_specman: String,
    pub spec_dir: String,
    pub impl_dir: String,
    pub scratchpad_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactRecord {
    pub id: ArtifactId,
    pub handle: String,
    pub path: String,
    pub version: Option<SemVer>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ArtifactInventory {
    #[serde(default)]
    pub specifications: Vec<ArtifactRecord>,
    #[serde(default)]
    pub implementations: Vec<ArtifactRecord>,
    #[serde(default)]
    pub scratchpads: Vec<ArtifactRecord>,
}

#[derive(Clone)]
pub struct SpecmanMcpServer {
    workspace: Arc<FilesystemWorkspaceLocator>,
    dependency_mapper: Arc<FilesystemDependencyMapper<Arc<FilesystemWorkspaceLocator>>>,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

#[tool_router]
#[prompt_router]
impl SpecmanMcpServer {
    pub fn new() -> Result<Self, SpecmanError> {
        let cwd = std::env::current_dir()?;
        Self::new_with_root(cwd)
    }

    pub fn new_with_root(root: impl Into<PathBuf>) -> Result<Self, SpecmanError> {
        let workspace = Arc::new(FilesystemWorkspaceLocator::new(root));
        let dependency_mapper = Arc::new(FilesystemDependencyMapper::new(workspace.clone()));

        Ok(Self {
            workspace,
            dependency_mapper,
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        })
    }

    #[tool(
        name = "workspace_discovery",
        description = "Return canonical workspace directories discovered via nearest .specman ancestor"
    )]
    async fn workspace_discovery(&self) -> Result<Json<WorkspaceInfo>, McpError> {
        let workspace = self.workspace.workspace().map_err(to_mcp_error)?;

        Ok(Json(WorkspaceInfo {
            root: workspace.root().display().to_string(),
            dot_specman: workspace.dot_specman().display().to_string(),
            spec_dir: workspace.spec_dir().display().to_string(),
            impl_dir: workspace.impl_dir().display().to_string(),
            scratchpad_dir: workspace.scratchpad_dir().display().to_string(),
        }))
    }

    #[tool(
        name = "workspace_inventory",
        description = "List specifications, implementations, and scratch pads as SpecMan resource handles"
    )]
    async fn workspace_inventory(&self) -> Result<Json<ArtifactInventory>, McpError> {
        Ok(Json(self.inventory().await?))
    }

    #[prompt(
        name = "feat",
        description = "Generate a SpecMan scratch pad for feature execution using the standard template"
    )]
    pub async fn scratch_feat_prompt(
        &self,
        Parameters(args): Parameters<ScratchFeatArgs>,
    ) -> Result<Vec<PromptMessage>, McpError> {
        self.render_scratch_prompt(
            SCRATCH_FEAT_TEMPLATE,
            &args.base.target,
            args.base.branch_name,
            args.base.scratch_name,
            args.base.arguments,
            ScratchPromptExtras {
                work_type: "feat",
                target_token: "{{target_impl_path}}",
                extra_tokens: vec![
                    (
                        "{{objectives}}",
                        value_or_default(args.objectives, "(add objectives)"),
                    ),
                    (
                        "{{task_outline}}",
                        value_or_default(args.task_outline, "(add a task outline)"),
                    ),
                ],
            },
        )
    }

    #[prompt(
        name = "ref",
        description = "Generate a SpecMan scratch pad for refactor discovery using the standard template"
    )]
    pub async fn scratch_ref_prompt(
        &self,
        Parameters(args): Parameters<ScratchRefArgs>,
    ) -> Result<Vec<PromptMessage>, McpError> {
        self.render_scratch_prompt(
            SCRATCH_REF_TEMPLATE,
            &args.base.target,
            args.base.branch_name,
            args.base.scratch_name,
            args.base.arguments,
            ScratchPromptExtras {
                work_type: "ref",
                target_token: "{{target_impl_path}}",
                extra_tokens: vec![
                    (
                        "{{refactor_focus}}",
                        value_or_default(args.refactor_focus, "(state refactor focus)"),
                    ),
                    (
                        "{{investigation_notes}}",
                        value_or_default(args.investigation_notes, "(capture investigation notes)"),
                    ),
                ],
            },
        )
    }

    #[prompt(
        name = "revision",
        description = "Generate a SpecMan scratch pad for specification revision using the standard template"
    )]
    pub async fn scratch_revision_prompt(
        &self,
        Parameters(args): Parameters<ScratchRevisionArgs>,
    ) -> Result<Vec<PromptMessage>, McpError> {
        self.render_scratch_prompt(
            SCRATCH_REVISION_TEMPLATE,
            &args.base.target,
            args.base.branch_name,
            args.base.scratch_name,
            args.base.arguments,
            ScratchPromptExtras {
                work_type: "revision",
                target_token: "{{target_spec_path}}",
                extra_tokens: vec![
                    (
                        "{{revised_headings}}",
                        value_or_default(args.revised_headings, "(list revised headings)"),
                    ),
                    (
                        "{{change_summary}}",
                        value_or_default(args.change_summary, "(summarize change scope)"),
                    ),
                ],
            },
        )
    }

    #[prompt(
        name = "fix",
        description = "Generate a SpecMan scratch pad for implementation fixes using the standard template"
    )]
    pub async fn scratch_fix_prompt(
        &self,
        Parameters(args): Parameters<ScratchFixArgs>,
    ) -> Result<Vec<PromptMessage>, McpError> {
        self.render_scratch_prompt(
            SCRATCH_FIX_TEMPLATE,
            &args.base.target,
            args.base.branch_name,
            args.base.scratch_name,
            args.base.arguments,
            ScratchPromptExtras {
                work_type: "fix",
                target_token: "{{target_impl_path}}",
                extra_tokens: Vec::new(),
            },
        )
    }
}

impl SpecmanMcpServer {
    /// Start a stdio-based MCP server and wait until the transport closes.
    pub async fn run_stdio(self) -> Result<(), ServerInitializeError> {
        let tools = self.tool_router.clone();
        let prompts = self.prompt_router.clone();
        let router = Router::new(self).with_tools(tools).with_prompts(prompts);
        let service = router.serve(transport::io::stdio()).await?;

        // Hold the service open until the peer closes the transport.
        let _ = service.waiting().await;
        Ok(())
    }
}

/// Convenience entry point that builds the server and runs it over stdio.
pub async fn run_stdio_server() -> Result<(), ServerInitializeError> {
    let server = SpecmanMcpServer::new()
        .map_err(|err| ServerInitializeError::InitializeFailed(to_mcp_error(err)))?;
    server.run_stdio().await
}

fn to_mcp_error(err: SpecmanError) -> McpError {
    ErrorData::internal_error(err.to_string(), None)
}

fn artifact_record(summary: &ArtifactSummary, workspace: &WorkspacePaths) -> ArtifactRecord {
    let handle = match summary.id.kind {
        ArtifactKind::Specification => format!("spec://{}", summary.id.name),
        ArtifactKind::Implementation => format!("impl://{}", summary.id.name),
        ArtifactKind::ScratchPad => format!("scratch://{}", summary.id.name),
    };

    let path = artifact_path(&summary.id, workspace).display().to_string();

    ArtifactRecord {
        id: summary.id.clone(),
        handle,
        path,
        version: summary.version.clone(),
        metadata: summary.metadata.clone(),
    }
}

fn artifact_path(id: &ArtifactId, workspace: &WorkspacePaths) -> PathBuf {
    match id.kind {
        ArtifactKind::Specification => workspace.spec_dir().join(&id.name).join("spec.md"),
        ArtifactKind::Implementation => workspace.impl_dir().join(&id.name).join("impl.md"),
        ArtifactKind::ScratchPad => workspace.scratchpad_dir().join(&id.name).join("scratch.md"),
    }
}

fn list_dir_entries(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root)
        .ok()
        .into_iter()
        .flat_map(|iter| iter.filter_map(Result::ok))
        .map(|entry| entry.path())
        .collect()
}

impl SpecmanMcpServer {
    async fn collect_artifacts(
        &self,
        kind: ArtifactKind,
        root: PathBuf,
        file_name: &str,
    ) -> Result<Vec<ArtifactRecord>, McpError> {
        let mut records = Vec::new();
        let workspace = self.workspace.workspace().map_err(to_mcp_error)?;

        for entry in list_dir_entries(&root) {
            if !entry.is_dir() {
                continue;
            }

            let target = entry.join(file_name);
            if !target.exists() {
                continue;
            }

            let tree = self
                .dependency_mapper
                .dependency_tree_from_path(&target)
                .map_err(to_mcp_error)?;

            if tree.root.id.kind == kind {
                records.push(artifact_record(&tree.root, &workspace));
            }
        }

        Ok(records)
    }

    async fn inventory(&self) -> Result<ArtifactInventory, McpError> {
        let workspace = self.workspace.workspace().map_err(to_mcp_error)?;

        let specs = self
            .collect_artifacts(ArtifactKind::Specification, workspace.spec_dir(), "spec.md")
            .await?;
        let impls = self
            .collect_artifacts(
                ArtifactKind::Implementation,
                workspace.impl_dir(),
                "impl.md",
            )
            .await?;
        let scratches = self
            .collect_artifacts(
                ArtifactKind::ScratchPad,
                workspace.scratchpad_dir(),
                "scratch.md",
            )
            .await?;

        Ok(ArtifactInventory {
            specifications: specs,
            implementations: impls,
            scratchpads: scratches,
        })
    }
}

#[prompt_handler]
impl ServerHandler for SpecmanMcpServer {
    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        async move {
            let inventory = self.inventory().await?;
            Ok(ListResourcesResult::with_all_items(
                resources_from_inventory(&inventory),
            ))
        }
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourceTemplatesResult, McpError>> + Send + '_
    {
        std::future::ready(Ok(ListResourceTemplatesResult::with_all_items(
            resource_templates(),
        )))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
        async move {
            let contents = self.read_resource_contents(&request.uri).await?;
            Ok(ReadResourceResult {
                contents: vec![contents],
            })
        }
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
            ..ServerInfo::default()
        }
    }
}

fn resource_for_artifact(
    record: &ArtifactRecord,
    description: String,
    mime_type: &'static str,
) -> Resource {
    Resource {
        raw: RawResource {
            uri: record.handle.clone(),
            name: record.id.name.clone(),
            title: Some(record.handle.clone()),
            description: Some(description),
            mime_type: Some(mime_type.to_string()),
            size: None,
            icons: None,
            meta: None,
        },
        annotations: None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ScratchPromptArgs {
    pub target: String,
    pub branch_name: Option<String>,
    pub scratch_name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ScratchFeatArgs {
    #[serde(flatten)]
    pub base: ScratchPromptArgs,
    #[serde(default)]
    pub objectives: Option<String>,
    #[serde(default)]
    pub task_outline: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ScratchRefArgs {
    #[serde(flatten)]
    pub base: ScratchPromptArgs,
    #[serde(default)]
    pub refactor_focus: Option<String>,
    #[serde(default)]
    pub investigation_notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ScratchRevisionArgs {
    #[serde(flatten)]
    pub base: ScratchPromptArgs,
    #[serde(default)]
    pub revised_headings: Option<String>,
    #[serde(default)]
    pub change_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ScratchFixArgs {
    #[serde(flatten)]
    pub base: ScratchPromptArgs,
}

struct ResolvedTarget {
    tree: DependencyTree,
    workspace: WorkspacePaths,
    handle: String,
    path: String,
}

struct ScratchPromptExtras {
    work_type: &'static str,
    target_token: &'static str,
    extra_tokens: Vec<(&'static str, String)>,
}

impl SpecmanMcpServer {
    fn resolve_target(&self, locator: &str) -> Result<ResolvedTarget, McpError> {
        let workspace = self.workspace.workspace().map_err(to_mcp_error)?;
        let tree = self
            .dependency_mapper
            .dependency_tree_from_locator(locator)
            .map_err(to_mcp_error)?;

        let path = artifact_path(&tree.root.id, &workspace)
            .display()
            .to_string();
        let handle = artifact_handle(&tree.root);

        Ok(ResolvedTarget {
            tree,
            workspace,
            handle,
            path,
        })
    }

    fn render_scratch_prompt(
        &self,
        template: &str,
        locator: &str,
        branch_name: Option<String>,
        scratch_name: Option<String>,
        arguments: Option<String>,
        extras: ScratchPromptExtras,
    ) -> Result<Vec<PromptMessage>, McpError> {
        let resolved = self.resolve_target(locator)?;

        let inferred_scratch = format!("{}-{}", resolved.tree.root.id.name, extras.work_type);
        let scratch_name = sanitize_slug(scratch_name.as_deref().unwrap_or(&inferred_scratch));
        let default_branch = format!(
            "{}/{}/{}",
            resolved.tree.root.id.name, extras.work_type, scratch_name
        );
        let branch_name = branch_name
            .and_then(|value| {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            })
            .unwrap_or(default_branch);

        let context = bullet_list(&dependency_lines(&resolved));
        let dependencies = context.clone();
        let argument_text = value_or_default(arguments, "(no additional arguments provided)");

        let mut replacements = vec![
            ("{{output_name}}", scratch_name.clone()),
            ("{{scratch_name}}", scratch_name.clone()),
            ("{{branch_name}}", branch_name.clone()),
            (extras.target_token, resolved.path.clone()),
            ("{{context}}", context),
            ("{{dependencies}}", dependencies),
            ("{{arguments}}", argument_text),
        ];

        replacements.extend(extras.extra_tokens);

        let rendered = apply_tokens(template, &replacements);
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            rendered,
        )])
    }
}

fn apply_tokens(template: &str, replacements: &[(&str, String)]) -> String {
    let mut rendered = template.to_owned();
    for (needle, value) in replacements {
        rendered = rendered.replace(needle, value);
    }
    rendered
}

fn artifact_handle(summary: &ArtifactSummary) -> String {
    match summary.id.kind {
        ArtifactKind::Specification => format!("spec://{}", summary.id.name),
        ArtifactKind::Implementation => format!("impl://{}", summary.id.name),
        ArtifactKind::ScratchPad => format!("scratch://{}", summary.id.name),
    }
}

fn dependency_lines(resolved: &ResolvedTarget) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("- {} ({})", resolved.handle, resolved.path));

    for edge in &resolved.tree.upstream {
        let handle = artifact_handle(&edge.to);
        let path = artifact_path(&edge.to.id, &resolved.workspace)
            .display()
            .to_string();
        lines.push(format!("- {} ({})", handle, path));
    }

    lines
}

fn bullet_list(items: &[String]) -> String {
    if items.is_empty() {
        "- (no dependencies discovered)".to_string()
    } else {
        items.join("\n")
    }
}

fn sanitize_slug(input: &str) -> String {
    let mut slug: String = input
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            '-' => '-',
            ch if ch.is_ascii_alphanumeric() => ch,
            ch if ch.is_whitespace() => '-',
            _ => '-',
        })
        .collect();

    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }

    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "scratch".to_string()
    } else {
        trimmed
    }
}

fn value_or_default(value: Option<String>, default_value: &str) -> String {
    value
        .and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .unwrap_or_else(|| default_value.to_string())
}

fn kind_label(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Specification => "specification",
        ArtifactKind::Implementation => "implementation",
        ArtifactKind::ScratchPad => "scratch pad",
    }
}

fn resources_from_inventory(inventory: &ArtifactInventory) -> Vec<Resource> {
    let mut resources = Vec::new();

    for record in &inventory.specifications {
        resources.push(resource_for_artifact(
            record,
            format!("SpecMan {} {}", kind_label(record.id.kind), record.handle),
            "text/markdown",
        ));
    }

    for record in &inventory.implementations {
        resources.push(resource_for_artifact(
            record,
            format!("SpecMan {} {}", kind_label(record.id.kind), record.handle),
            "text/markdown",
        ));
    }

    for record in &inventory.scratchpads {
        resources.push(resource_for_artifact(
            record,
            format!("SpecMan {} {}", kind_label(record.id.kind), record.handle),
            "text/markdown",
        ));
    }

    resources
}

fn resource_templates() -> Vec<ResourceTemplate> {
    let mut templates = Vec::new();

    templates.push(ResourceTemplate {
        raw: RawResourceTemplate {
            uri_template: "spec://{artifact}".to_string(),
            name: "spec-resource".to_string(),
            title: Some("Specification content".to_string()),
            description: Some("Read a SpecMan specification as a resource".to_string()),
            mime_type: Some("text/markdown".to_string()),
        },
        annotations: None,
    });

    templates.push(ResourceTemplate {
        raw: RawResourceTemplate {
            uri_template: "impl://{artifact}".to_string(),
            name: "impl-resource".to_string(),
            title: Some("Implementation content".to_string()),
            description: Some("Read a SpecMan implementation as a resource".to_string()),
            mime_type: Some("text/markdown".to_string()),
        },
        annotations: None,
    });

    templates.push(ResourceTemplate {
        raw: RawResourceTemplate {
            uri_template: "scratch://{artifact}".to_string(),
            name: "scratch-resource".to_string(),
            title: Some("Scratch pad content".to_string()),
            description: Some("Read a SpecMan scratch pad as a resource".to_string()),
            mime_type: Some("text/markdown".to_string()),
        },
        annotations: None,
    });

    templates.push(ResourceTemplate {
        raw: RawResourceTemplate {
            uri_template: "spec://{artifact}/dependencies".to_string(),
            name: "spec-dependencies".to_string(),
            title: Some("Specification dependency tree".to_string()),
            description: Some("Return dependency tree JSON for a specification".to_string()),
            mime_type: Some("application/json".to_string()),
        },
        annotations: None,
    });

    templates.push(ResourceTemplate {
        raw: RawResourceTemplate {
            uri_template: "impl://{artifact}/dependencies".to_string(),
            name: "impl-dependencies".to_string(),
            title: Some("Implementation dependency tree".to_string()),
            description: Some("Return dependency tree JSON for an implementation".to_string()),
            mime_type: Some("application/json".to_string()),
        },
        annotations: None,
    });

    templates.push(ResourceTemplate {
        raw: RawResourceTemplate {
            uri_template: "scratch://{artifact}/dependencies".to_string(),
            name: "scratch-dependencies".to_string(),
            title: Some("Scratch pad dependency tree".to_string()),
            description: Some(
                "Return dependency tree JSON for a scratch pad (if dependencies are tracked)"
                    .to_string(),
            ),
            mime_type: Some("application/json".to_string()),
        },
        annotations: None,
    });

    templates
}

pub type McpError = ErrorData;

impl SpecmanMcpServer {
    async fn read_resource_contents(&self, uri: &str) -> Result<ResourceContents, McpError> {
        // Dependency resources use the same locator minus the /dependencies suffix for tree construction.
        let (is_dependencies, base_locator) = uri
            .strip_suffix("/dependencies")
            .map(|base| (true, base))
            .unwrap_or((false, uri));

        let workspace = self.workspace.workspace().map_err(to_mcp_error)?;
        let tree = self
            .dependency_mapper
            .dependency_tree_from_locator(base_locator)
            .map_err(to_mcp_error)?;

        if is_dependencies {
            let json_tree = serde_json::to_string(&tree)
                .map_err(|err| to_mcp_error(SpecmanError::Serialization(err.to_string())))?;
            return Ok(ResourceContents::TextResourceContents {
                uri: uri.to_string(),
                mime_type: Some("application/json".to_string()),
                text: json_tree,
                meta: None,
            });
        }

        let path = artifact_path(&tree.root.id, &workspace);
        let body = fs::read_to_string(&path).map_err(|err| to_mcp_error(err.into()))?;
        Ok(ResourceContents::TextResourceContents {
            uri: uri.to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: body,
            meta: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::PromptMessageContent;
    use std::io::Write;
    use tempfile::TempDir;

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
            other => panic!("unexpected variant: {:?}", other),
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
            other => panic!("unexpected variant: {:?}", other),
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
            .scratch_feat_prompt(Parameters(ScratchFeatArgs {
                base: ScratchPromptArgs {
                    target: "impl://testimpl".to_string(),
                    branch_name: None,
                    scratch_name: None,
                    arguments: Some("user guidance".to_string()),
                },
                objectives: Some("ship a demo".to_string()),
                task_outline: None,
            }))
            .await?
            .pop()
            .expect("prompt returns one message");

        let rendered = match message.content {
            PromptMessageContent::Text { text } => text,
            other => panic!("unexpected content: {:?}", other),
        };

        assert!(rendered.contains("branch: testimpl/feat/testimpl-feat"));
        assert!(rendered.contains("user guidance"));
        assert!(rendered.contains("ship a demo"));
        assert!(rendered.contains("impl/testimpl/impl.md"));

        Ok(())
    }

    #[tokio::test]
    async fn scratch_ref_prompt_renders_dependencies() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;
        let rendered = workspace
            .server
            .scratch_ref_prompt(Parameters(ScratchRefArgs {
                base: ScratchPromptArgs {
                    target: "impl://testimpl".to_string(),
                    branch_name: None,
                    scratch_name: Some("impl-refactor".to_string()),
                    arguments: None,
                },
                refactor_focus: Some("restructure adapters".to_string()),
                investigation_notes: Some("check dependency graph".to_string()),
            }))
            .await?
            .pop()
            .expect("prompt returns one message")
            .content;

        let text = match rendered {
            PromptMessageContent::Text { text } => text,
            other => panic!("unexpected content: {:?}", other),
        };

        assert!(text.contains("impl/testimpl/impl.md"));
        assert!(text.contains("spec://testspec"));
        assert!(text.contains("impl-refactor"));
        assert!(text.contains("restructure adapters"));
        assert!(text.contains("check dependency graph"));

        Ok(())
    }

    #[tokio::test]
    async fn scratch_revision_prompt_uses_spec_target() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;
        let rendered = workspace
            .server
            .scratch_revision_prompt(Parameters(ScratchRevisionArgs {
                base: ScratchPromptArgs {
                    target: "spec://testspec".to_string(),
                    branch_name: Some("custom-revision".to_string()),
                    scratch_name: None,
                    arguments: Some("user adds notes".to_string()),
                },
                revised_headings: Some("intro, scope".to_string()),
                change_summary: Some("tighten wording".to_string()),
            }))
            .await?
            .pop()
            .expect("prompt returns one message")
            .content;

        let text = match rendered {
            PromptMessageContent::Text { text } => text,
            other => panic!("unexpected content: {:?}", other),
        };

        assert!(text.contains("spec/testspec/spec.md"));
        assert!(text.contains("custom-revision"));
        assert!(text.contains("intro, scope"));
        assert!(text.contains("tighten wording"));
        assert!(text.contains("user adds notes"));

        Ok(())
    }

    #[tokio::test]
    async fn scratch_fix_prompt_defaults_branch() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = TestWorkspace::create()?;
        let rendered = workspace
            .server
            .scratch_fix_prompt(Parameters(ScratchFixArgs {
                base: ScratchPromptArgs {
                    target: "impl://testimpl".to_string(),
                    branch_name: None,
                    scratch_name: None,
                    arguments: Some("fix bug".to_string()),
                },
            }))
            .await?
            .pop()
            .expect("prompt returns one message")
            .content;

        let text = match rendered {
            PromptMessageContent::Text { text } => text,
            other => panic!("unexpected content: {:?}", other),
        };

        assert!(text.contains("impl/testimpl/impl.md"));
        assert!(text.contains("testimpl/fix/testimpl-fix"));
        assert!(text.contains("fix bug"));

        Ok(())
    }

    #[test]
    fn prompt_router_lists_all_prompts() {
        let server = SpecmanMcpServer::new().expect("server should start");
        let prompts = server.prompt_router.list_all();
        let names: std::collections::HashSet<_> = prompts.iter().map(|p| p.name.as_str()).collect();

        for expected in [
            "specman.scratch.feat",
            "specman.scratch.ref",
            "specman.scratch.revision",
            "specman.scratch.fix",
        ] {
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

        fs::create_dir_all(&spec_dir)?;
        fs::create_dir_all(&impl_dir)?;
        fs::create_dir_all(&scratch_dir)?;

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

        Ok(())
    }
}
