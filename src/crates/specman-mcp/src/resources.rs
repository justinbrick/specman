use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    GetPromptRequestParam, GetPromptResult, ListPromptsResult, ListResourceTemplatesResult,
    ListResourcesResult, PaginatedRequestParam, RawResource, RawResourceTemplate,
    ReadResourceRequestParam, ReadResourceResult, Resource, ResourceContents, ResourceTemplate,
    ServerCapabilities, ServerInfo,
};
use rmcp::prompt_handler;
use rmcp::schemars::JsonSchema;
use rmcp::service::{RequestContext, RoleServer};
use serde::{Deserialize, Serialize};

use specman::{
    ArtifactId, ArtifactKind, ArtifactSummary, SemVer, SpecmanError,
    WorkspaceLocator, WorkspacePaths,
};

use crate::error::{McpError, to_mcp_error};
use crate::server::SpecmanMcpServer;

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

pub(crate) fn workspace_relative_path(root: &Path, absolute: &Path) -> Option<String> {
    let relative = absolute.strip_prefix(root).ok()?;
    Some(relative.to_string_lossy().replace('\\', "/"))
}

pub(crate) fn artifact_path(id: &ArtifactId, workspace: &WorkspacePaths) -> PathBuf {
    match id.kind {
        ArtifactKind::Specification => workspace.spec_dir().join(&id.name).join("spec.md"),
        ArtifactKind::Implementation => workspace.impl_dir().join(&id.name).join("impl.md"),
        ArtifactKind::ScratchPad => workspace.scratchpad_dir().join(&id.name).join("scratch.md"),
    }
}

pub(crate) fn resolved_path_or_artifact_path(
    summary: &ArtifactSummary,
    workspace: &WorkspacePaths,
) -> String {
    summary
        .resolved_path
        .clone()
        .unwrap_or_else(|| artifact_path(&summary.id, workspace).display().to_string())
}

pub(crate) fn artifact_handle(summary: &ArtifactSummary) -> String {
    match summary.id.kind {
        ArtifactKind::Specification => format!("spec://{}", summary.id.name),
        ArtifactKind::Implementation => format!("impl://{}", summary.id.name),
        ArtifactKind::ScratchPad => format!("scratch://{}", summary.id.name),
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

fn artifact_record(summary: &ArtifactSummary, workspace: &WorkspacePaths) -> ArtifactRecord {
    ArtifactRecord {
        id: summary.id.clone(),
        handle: artifact_handle(summary),
        path: resolved_path_or_artifact_path(summary, workspace),
        version: summary.version.clone(),
        metadata: summary.metadata.clone(),
    }
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

    pub(crate) async fn inventory(&self) -> Result<ArtifactInventory, McpError> {
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

    pub(crate) async fn read_resource_contents(
        &self,
        uri: &str,
    ) -> Result<ResourceContents, McpError> {
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

fn kind_label(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Specification => "specification",
        ArtifactKind::Implementation => "implementation",
        ArtifactKind::ScratchPad => "scratch pad",
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

pub(crate) fn resources_from_inventory(inventory: &ArtifactInventory) -> Vec<Resource> {
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

pub(crate) fn resource_templates() -> Vec<ResourceTemplate> {
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
