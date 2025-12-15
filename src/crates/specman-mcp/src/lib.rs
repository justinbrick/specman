use std::collections::{BTreeMap, HashSet};
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
    ArtifactId, ArtifactKind, ArtifactSummary, CreateRequest, DefaultLifecycleController,
    DependencyTree, FilesystemDependencyMapper, FilesystemWorkspaceLocator, MarkdownTemplateEngine,
    PersistedArtifact, SemVer, Specman, SpecmanError, TemplateCatalog, WorkspaceLocator,
    WorkspacePaths, WorkspacePersistence,
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

/// Deterministic result payload returned by the `create_artifact` MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateArtifactResult {
    pub id: ArtifactId,
    pub handle: String,
    /// Canonical workspace-relative path to the created artifact.
    pub path: String,
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
        name = "create_artifact",
        description = "Create a SpecMan artifact (spec, impl, or scratch pad) from a core CreateRequest"
    )]
    async fn create_artifact(
        &self,
        Parameters(request): Parameters<CreateRequest>,
    ) -> Result<Json<CreateArtifactResult>, McpError> {
        let normalized = self.normalize_create_request(request)?;
        let specman = self.build_specman()?;
        let persisted = specman.create(normalized).map_err(to_mcp_error)?;

        Ok(Json(create_artifact_result(&persisted)))
    }

    #[prompt(
        name = "feat",
        description = "Generate a SpecMan scratch pad for feature execution using the standard template"
    )]
    pub async fn scratch_feat_prompt(
        &self,
        Parameters(args): Parameters<ScratchPromptArgs>,
    ) -> Result<Vec<PromptMessage>, McpError> {
        self.render_scratch_prompt(
            SCRATCH_FEAT_TEMPLATE,
            &args.target,
            args.branch_name,
            "feat",
        )
    }

    #[prompt(
        name = "ref",
        description = "Generate a SpecMan scratch pad for refactor discovery using the standard template"
    )]
    pub async fn scratch_ref_prompt(
        &self,
        Parameters(args): Parameters<ScratchPromptArgs>,
    ) -> Result<Vec<PromptMessage>, McpError> {
        self.render_scratch_prompt(SCRATCH_REF_TEMPLATE, &args.target, args.branch_name, "ref")
    }

    #[prompt(
        name = "revision",
        description = "Generate a SpecMan scratch pad for specification revision using the standard template"
    )]
    pub async fn scratch_revision_prompt(
        &self,
        Parameters(args): Parameters<ScratchPromptArgs>,
    ) -> Result<Vec<PromptMessage>, McpError> {
        self.render_scratch_prompt(
            SCRATCH_REVISION_TEMPLATE,
            &args.target,
            args.branch_name,
            "revision",
        )
    }

    #[prompt(
        name = "fix",
        description = "Generate a SpecMan scratch pad for implementation fixes using the standard template"
    )]
    pub async fn scratch_fix_prompt(
        &self,
        Parameters(args): Parameters<ScratchPromptArgs>,
    ) -> Result<Vec<PromptMessage>, McpError> {
        self.render_scratch_prompt(SCRATCH_FIX_TEMPLATE, &args.target, args.branch_name, "fix")
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

fn invalid_params(message: impl Into<String>) -> McpError {
    ErrorData::invalid_params(message.into(), None)
}

fn create_artifact_result(persisted: &PersistedArtifact) -> CreateArtifactResult {
    let relative = workspace_relative_path(persisted.workspace.root(), &persisted.path)
        .unwrap_or_else(|| persisted.path.display().to_string());
    let handle = match persisted.artifact.kind {
        ArtifactKind::Specification => format!("spec://{}", persisted.artifact.name),
        ArtifactKind::Implementation => format!("impl://{}", persisted.artifact.name),
        ArtifactKind::ScratchPad => format!("scratch://{}", persisted.artifact.name),
    };
    CreateArtifactResult {
        id: persisted.artifact.clone(),
        handle,
        path: relative,
    }
}

fn workspace_relative_path(root: &Path, absolute: &Path) -> Option<String> {
    let relative = absolute.strip_prefix(root).ok()?;
    Some(relative.to_string_lossy().replace('\\', "/"))
}

fn artifact_record(summary: &ArtifactSummary, workspace: &WorkspacePaths) -> ArtifactRecord {
    let handle = match summary.id.kind {
        ArtifactKind::Specification => format!("spec://{}", summary.id.name),
        ArtifactKind::Implementation => format!("impl://{}", summary.id.name),
        ArtifactKind::ScratchPad => format!("scratch://{}", summary.id.name),
    };

    let path = resolved_path_or_artifact_path(summary, workspace);

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

fn resolved_path_or_artifact_path(summary: &ArtifactSummary, workspace: &WorkspacePaths) -> String {
    summary
        .resolved_path
        .clone()
        .unwrap_or_else(|| artifact_path(&summary.id, workspace).display().to_string())
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
    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let inventory = self.inventory().await?;
        Ok(ListResourcesResult::with_all_items(
            resources_from_inventory(&inventory),
        ))
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

    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let contents = self.read_resource_contents(&request.uri).await?;
        Ok(ReadResourceResult {
            contents: vec![contents],
        })
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
}

struct ResolvedTarget {
    tree: DependencyTree,
    workspace: WorkspacePaths,
    handle: String,
    path: String,
}

impl SpecmanMcpServer {
    fn resolve_target(&self, locator: &str) -> Result<ResolvedTarget, McpError> {
        let workspace = self.workspace.workspace().map_err(to_mcp_error)?;
        let tree = self
            .dependency_mapper
            .dependency_tree_from_locator_best_effort(locator)
            .map_err(to_mcp_error)?;

        let path = resolved_path_or_artifact_path(&tree.root, &workspace);
        let handle = artifact_handle(&tree.root);

        Ok(ResolvedTarget {
            tree,
            workspace,
            handle,
            path,
        })
    }

    /// Render a scratch-pad prompt using only template-spec tokens to honor the Template Token Contract.
    fn render_scratch_prompt(
        &self,
        template: &str,
        locator: &str,
        branch_name: Option<String>,
        work_type: &str,
    ) -> Result<Vec<PromptMessage>, McpError> {
        let resolved = self.resolve_target(locator)?;

        let target_name = resolved.tree.root.id.name.clone();

        let provided_branch = branch_name.and_then(|value| normalize_input(Some(value)));

        let branch_instruction = match provided_branch {
            Some(branch) => format!(
                "Check out the provided branch \"{branch}\" and keep it active while working on this {work_type} scratch pad."
            ),
            None => format!(
                "Create and check out a branch that follows {target_name}/{work_type}/{{scratch_pad_name}}; for this work, an example is {target_name}/{work_type}/action-being-done."
            ),
        };

        let artifact_instruction = format!(
            "Provide a scratch pad name (lowercase, hyphenated, ≤4 words) that satisfies spec/specman-data-model naming rules. Example: action-being-done."
        );

        let context = bullet_list(&dependency_lines(&resolved));
        let dependencies = context.clone();

        let replacements = vec![
            ("{{branch_name_or_request}}", branch_instruction.clone()),
            ("{{branch_name}}", branch_instruction.clone()),
            ("{{target_path}}", resolved.handle.clone()),
            ("{{context}}", context),
            ("{{dependencies}}", dependencies),
            ("{{artifact_name_or_request}}", artifact_instruction),
        ];

        let rendered = apply_tokens(template, &replacements)?;
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            rendered,
        )])
    }
}

fn apply_tokens(template: &str, replacements: &[(&str, String)]) -> Result<String, McpError> {
    let mut rendered = template.to_owned();
    for (needle, value) in replacements {
        rendered = rendered.replace(needle, value);
    }

    if rendered.contains("{{") {
        return Err(to_mcp_error(SpecmanError::Template(
            "unresolved template tokens remain after rendering".to_string(),
        )));
    }

    Ok(rendered)
}

fn artifact_handle(summary: &ArtifactSummary) -> String {
    match summary.id.kind {
        ArtifactKind::Specification => format!("spec://{}", summary.id.name),
        ArtifactKind::Implementation => format!("impl://{}", summary.id.name),
        ArtifactKind::ScratchPad => format!("scratch://{}", summary.id.name),
    }
}

fn dependency_lines(resolved: &ResolvedTarget) -> Vec<String> {
    // Deduplicate on artifact handle while preserving first-seen order; always keep the root entry.
    // If downstream context is added later, apply the same handle-level dedup there as well.
    let mut lines = Vec::new();
    let mut seen = HashSet::new();

    let root_handle = resolved.handle.clone();
    let root_line = format!("- {} ({})", root_handle, resolved.path);
    lines.push(root_line);
    seen.insert(root_handle);

    for edge in &resolved.tree.upstream {
        let handle = artifact_handle(&edge.to);
        if !seen.insert(handle.clone()) {
            continue;
        }

        let path = resolved_path_or_artifact_path(&edge.to, &resolved.workspace);
        lines.push(format!("- {handle} ({path})"));
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

fn normalize_input(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
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
    vec![
        ResourceTemplate {
            raw: RawResourceTemplate {
                uri_template: "spec://{artifact}".to_string(),
                name: "spec-resource".to_string(),
                title: Some("Specification content".to_string()),
                description: Some("Read a SpecMan specification as a resource".to_string()),
                mime_type: Some("text/markdown".to_string()),
            },
            annotations: None,
        },
        ResourceTemplate {
            raw: RawResourceTemplate {
                uri_template: "impl://{artifact}".to_string(),
                name: "impl-resource".to_string(),
                title: Some("Implementation content".to_string()),
                description: Some("Read a SpecMan implementation as a resource".to_string()),
                mime_type: Some("text/markdown".to_string()),
            },
            annotations: None,
        },
        ResourceTemplate {
            raw: RawResourceTemplate {
                uri_template: "scratch://{artifact}".to_string(),
                name: "scratch-resource".to_string(),
                title: Some("Scratch pad content".to_string()),
                description: Some("Read a SpecMan scratch pad as a resource".to_string()),
                mime_type: Some("text/markdown".to_string()),
            },
            annotations: None,
        },
        ResourceTemplate {
            raw: RawResourceTemplate {
                uri_template: "spec://{artifact}/dependencies".to_string(),
                name: "spec-dependencies".to_string(),
                title: Some("Specification dependency tree".to_string()),
                description: Some("Return dependency tree JSON for a specification".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            annotations: None,
        },
        ResourceTemplate {
            raw: RawResourceTemplate {
                uri_template: "impl://{artifact}/dependencies".to_string(),
                name: "impl-dependencies".to_string(),
                title: Some("Implementation dependency tree".to_string()),
                description: Some("Return dependency tree JSON for an implementation".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            annotations: None,
        },
        ResourceTemplate {
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
        },
    ]
}

pub type McpError = ErrorData;

impl SpecmanMcpServer {
    fn build_specman(
        &self,
    ) -> Result<
        Specman<
            FilesystemDependencyMapper<Arc<FilesystemWorkspaceLocator>>,
            MarkdownTemplateEngine,
            Arc<FilesystemWorkspaceLocator>,
        >,
        McpError,
    > {
        let locator = self.workspace.clone();
        let workspace = locator.workspace().map_err(to_mcp_error)?;

        let mapper = FilesystemDependencyMapper::new(locator.clone());
        let inventory = mapper.inventory_handle();
        let templates = MarkdownTemplateEngine::new();
        let controller = DefaultLifecycleController::new(mapper, templates);
        let catalog = TemplateCatalog::new(workspace);
        let persistence = WorkspacePersistence::with_inventory(locator, inventory);

        Ok(Specman::new(controller, catalog, persistence))
    }

    fn normalize_locator_to_workspace_path(&self, locator: &str) -> Result<String, McpError> {
        let trimmed = locator.trim();
        if trimmed.is_empty() {
            return Err(invalid_params("locator must not be empty"));
        }

        if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
            return Err(invalid_params(
                "workspace target locators must not be URLs; use spec://, impl://, scratch://, or a workspace-relative path",
            ));
        }

        let workspace = self.workspace.workspace().map_err(to_mcp_error)?;
        let tree = self
            .dependency_mapper
            .dependency_tree_from_locator(trimmed)
            .map_err(to_mcp_error)?;

        let resolved = resolved_path_or_artifact_path(&tree.root, &workspace);
        let absolute = PathBuf::from(resolved);
        workspace_relative_path(workspace.root(), &absolute)
            .ok_or_else(|| invalid_params("locator must resolve within the workspace"))
    }

    fn normalize_locator_to_handle(&self, locator: &str) -> Result<ArtifactSummary, McpError> {
        let trimmed = locator.trim();
        if trimmed.is_empty() {
            return Err(invalid_params("locator must not be empty"));
        }

        if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
            return Err(invalid_params(
                "workspace target locators must not be URLs; use spec://, impl://, scratch://, or a workspace-relative path",
            ));
        }

        let tree = self
            .dependency_mapper
            .dependency_tree_from_locator(trimmed)
            .map_err(to_mcp_error)?;
        Ok(tree.root)
    }

    fn normalize_create_request(&self, request: CreateRequest) -> Result<CreateRequest, McpError> {
        match request {
            CreateRequest::Custom { .. } => Err(invalid_params(
                "CreateRequest::Custom is not supported via MCP; use Specification, Implementation, or ScratchPad",
            )),
            CreateRequest::Implementation { context } => {
                let target_summary = self.normalize_locator_to_handle(&context.target)?;
                if target_summary.id.kind != ArtifactKind::Specification {
                    return Err(invalid_params(
                        "implementation targets must resolve to a specification (spec://... or a spec path)",
                    ));
                }
                let target = artifact_handle(&target_summary);
                Ok(CreateRequest::Implementation {
                    context: specman::ImplContext {
                        name: context.name.trim().to_string(),
                        target,
                    },
                })
            }
            CreateRequest::ScratchPad { context } => {
                let target = self.normalize_locator_to_workspace_path(&context.target)?;
                Ok(CreateRequest::ScratchPad {
                    context: specman::ScratchPadCreateContext {
                        name: context.name.trim().to_string(),
                        target,
                        work_type: context.work_type,
                    },
                })
            }
            CreateRequest::Specification { context } => Ok(CreateRequest::Specification {
                context: specman::SpecContext {
                    name: context.name.trim().to_string(),
                    title: context.title.trim().to_string(),
                },
            }),
        }
    }

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
    use specman::front_matter::{ScratchWorkType, ScratchWorkloadExtras};
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
    async fn scratch_prompts_render_normalized_handles() -> Result<(), Box<dyn std::error::Error>> {
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

        Ok(())
    }

    #[tokio::test]
    async fn scratch_prompts_clear_template_tokens() -> Result<(), Box<dyn std::error::Error>> {
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

        for expected in ["feat", "ref", "revision", "fix"] {
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
