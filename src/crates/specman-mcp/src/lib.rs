use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::handler::server::{
    ServerHandler, router::Router, tool::ToolRouter, wrapper::Json, wrapper::Parameters,
};
use rmcp::schemars::JsonSchema;
use rmcp::service::{RequestContext, RoleServer, ServerInitializeError};
use rmcp::{
    model::{
        ErrorData, ListResourceTemplatesResult, ListResourcesResult, PaginatedRequestParam,
        RawResource, RawResourceTemplate, ReadResourceRequestParam, ReadResourceResult, Resource,
        ResourceContents, ResourceTemplate, ServerCapabilities, ServerInfo,
    },
    service::ServiceExt,
    tool, tool_router, transport,
};
use serde::{Deserialize, Serialize};
use specman::{
    ArtifactId, ArtifactKind, ArtifactSummary, DependencyTree, FilesystemDependencyMapper,
    FilesystemWorkspaceLocator, SemVer, SpecmanError, WorkspaceLocator, WorkspacePaths,
};

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
pub struct DependencyTreeRequest {
    pub locator: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DescribeArtifactRequest {
    pub locator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactDescription {
    pub artifact: ArtifactRecord,
    pub tree: DependencyTree,
}

#[derive(Clone)]
pub struct SpecmanMcpServer {
    workspace: Arc<FilesystemWorkspaceLocator>,
    dependency_mapper: Arc<FilesystemDependencyMapper<Arc<FilesystemWorkspaceLocator>>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl SpecmanMcpServer {
    pub fn new() -> Result<Self, SpecmanError> {
        let workspace = Arc::new(FilesystemWorkspaceLocator::from_current_dir()?);
        let dependency_mapper = Arc::new(FilesystemDependencyMapper::new(workspace.clone()));

        Ok(Self {
            workspace,
            dependency_mapper,
            tool_router: Self::tool_router(),
        })
    }

    #[tool(
        name = "specman.core.workspace_discovery",
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
        name = "specman.core.dependency_mapping",
        description = "Build dependency tree for a locator (path, URL, or spec:// handle)"
    )]
    async fn dependency_mapping(
        &self,
        params: Parameters<DependencyTreeRequest>,
    ) -> Result<Json<DependencyTree>, McpError> {
        let locator = params.0.locator;
        let tree = self
            .dependency_mapper
            .dependency_tree_from_locator(&locator)
            .map_err(to_mcp_error)?;
        Ok(Json(tree))
    }

    #[tool(
        name = "specman.core.workspace_inventory",
        description = "List specifications, implementations, and scratch pads as SpecMan resource handles"
    )]
    async fn workspace_inventory(&self) -> Result<Json<ArtifactInventory>, McpError> {
        Ok(Json(self.inventory().await?))
    }

    #[tool(
        name = "specman.core.artifact_describe",
        description = "Describe an artifact by locator or resource handle and include its dependency tree"
    )]
    async fn artifact_describe(
        &self,
        params: Parameters<DescribeArtifactRequest>,
    ) -> Result<Json<ArtifactDescription>, McpError> {
        let workspace = self.workspace.workspace().map_err(to_mcp_error)?;
        let locator = params.0.locator;

        let tree = self
            .dependency_mapper
            .dependency_tree_from_locator(&locator)
            .map_err(to_mcp_error)?;

        let record = artifact_record(&tree.root, &workspace);

        Ok(Json(ArtifactDescription {
            artifact: record,
            tree,
        }))
    }
}

impl SpecmanMcpServer {
    /// Start a stdio-based MCP server and wait until the transport closes.
    pub async fn run_stdio(self) -> Result<(), ServerInitializeError> {
        let tools = self.tool_router.clone();
        let router = Router::new(self).with_tools(tools);
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
        _dir_guard: DirGuard,
        server: SpecmanMcpServer,
    }

    impl TestWorkspace {
        fn create() -> Result<Self, Box<dyn std::error::Error>> {
            let temp = tempfile::tempdir()?;
            let dir_guard = DirGuard::change_to(temp.path())?;

            create_workspace_files(temp.path())?;

            let server = SpecmanMcpServer::new()?;

            Ok(Self {
                _temp: temp,
                _dir_guard: dir_guard,
                server,
            })
        }
    }

    struct DirGuard {
        previous: PathBuf,
    }

    impl DirGuard {
        fn change_to(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
            let previous = std::env::current_dir()?;
            std::env::set_current_dir(path)?;
            Ok(Self { previous })
        }
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.previous);
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
