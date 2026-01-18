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
    ArtifactId, ArtifactKey, ArtifactKind, ArtifactSummary, ConstraintIdentifier,
    FilesystemStructureIndexer, SemVer, SpecmanError, WorkspaceLocator, WorkspacePaths,
};

use crate::error::{McpError, invalid_params, to_mcp_error};
use crate::server::SpecmanMcpServer;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConstraintIndexEntry {
    #[schemars(description = "Constraint group set without leading '!' or trailing ':'.")]
    pub constraint_id: String,
    #[schemars(description = "Literal identifier line as it appears in the spec artifact.")]
    pub identifier_line: String,
    #[schemars(description = "Canonical resource URI for reading this constraint group.")]
    pub uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConstraintIndex {
    #[schemars(
        description = "Artifact handle (e.g. 'spec://name') the constraints were read from."
    )]
    pub artifact: String,
    #[schemars(description = "Deterministic list of constraint groups in the specification.")]
    pub constraints: Vec<ConstraintIndexEntry>,
}

fn ensure_specification_artifact(kind: ArtifactKind) -> Result<(), McpError> {
    if kind == ArtifactKind::Specification {
        Ok(())
    } else {
        Err(invalid_params(
            "'/constraints' resources are only available for spec:// artifacts",
        ))
    }
}

fn validate_constraint_id(constraint_id: &str) -> Result<(), McpError> {
    if constraint_id.is_empty() {
        return Err(invalid_params("constraint_id must not be empty"));
    }
    if constraint_id.starts_with('!') || constraint_id.ends_with(':') {
        return Err(invalid_params(
            "constraint_id must not include leading '!' or trailing ':'",
        ));
    }
    if constraint_id.contains('/') {
        return Err(invalid_params("constraint_id must not include '/'"));
    }
    if constraint_id.contains(' ') || constraint_id.contains('\t') {
        return Err(invalid_params("constraint_id must not include whitespace"));
    }
    Ok(())
}

fn read_artifact_file(path: &Path) -> Result<String, McpError> {
    fs::read_to_string(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            invalid_params(format!("Artifact not found: {}", path.display()))
        } else {
            to_mcp_error(err.into())
        }
    })
}

#[derive(Clone, Debug)]
struct FenceState {
    ch: char,
    len: usize,
}

fn fence_update(current: Option<&FenceState>, line: &str) -> Option<Option<FenceState>> {
    let trimmed = line.strip_prefix("   ").unwrap_or(line);
    let (ch, run) = if trimmed.starts_with("```") {
        ('`', trimmed.chars().take_while(|c| *c == '`').count())
    } else if trimmed.starts_with("~~~") {
        ('~', trimmed.chars().take_while(|c| *c == '~').count())
    } else {
        return None;
    };

    if current.is_none() {
        return Some(Some(FenceState {
            ch,
            len: run.max(3),
        }));
    }

    let cur = current.unwrap();
    if cur.ch == ch && trimmed.chars().take_while(|c| *c == ch).count() >= cur.len {
        return Some(None);
    }

    Some(Some(cur.clone()))
}

fn is_atx_heading_line(line: &str) -> bool {
    let trimmed = line.strip_prefix("   ").unwrap_or(line);
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&hashes) {
        return false;
    }
    let Some(after) = trimmed.get(hashes..) else {
        return false;
    };
    after.starts_with(' ')
}

fn is_constraint_identifier_line(trimmed: &str) -> bool {
    if !trimmed.starts_with('!') || !trimmed.ends_with(':') {
        return false;
    }
    if trimmed.contains(' ') || trimmed.contains('\t') {
        return false;
    }
    let core = trimmed.trim_start_matches('!').trim_end_matches(':');
    let groups: Vec<_> = core.split('.').filter(|s| !s.is_empty()).collect();
    groups.len() >= 2
}

fn extract_constraint_block(body: &str, constraint_id: &str) -> Option<String> {
    let lines: Vec<&str> = body.lines().collect();

    let mut fence: Option<FenceState> = None;
    let mut start: Option<usize> = None;

    for (idx, raw) in lines.iter().enumerate() {
        if let Some(updated) = fence_update(fence.as_ref(), raw) {
            fence = updated;
        }

        if fence.is_some() {
            continue;
        }

        let trimmed = raw.trim();
        if !is_constraint_identifier_line(trimmed) {
            continue;
        }

        let group = trimmed.trim_start_matches('!').trim_end_matches(':');

        if group == constraint_id {
            start = Some(idx);
            break;
        }
    }

    let start = start?;

    // Scan forward until the next constraint identifier or next heading.
    let mut fence: Option<FenceState> = None;
    let mut end = lines.len();
    for raw in lines.iter().skip(start + 1).enumerate() {
        let (offset, line) = raw;
        if let Some(updated) = fence_update(fence.as_ref(), line) {
            fence = updated;
        }
        if fence.is_some() {
            continue;
        }
        let trimmed = line.trim();
        if is_atx_heading_line(line) || is_constraint_identifier_line(trimmed) {
            end = start + 1 + offset;
            break;
        }
    }

    let mut out = lines[start..end].join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Some(out)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactRecord {
    #[schemars(description = "Stable artifact identifier (kind + name).")]
    pub id: ArtifactId,
    #[schemars(
        description = "Canonical artifact handle (e.g. 'spec://name', 'impl://name', 'scratch://name')."
    )]
    pub handle: String,
    #[schemars(description = "Workspace-relative path to the artifact markdown file.")]
    pub path: String,
    #[schemars(description = "Optional semantic version if the artifact declares one.")]
    pub version: Option<SemVer>,
    #[serde(default)]
    #[schemars(description = "Optional artifact metadata map (string keys/values).")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ArtifactInventory {
    #[serde(default)]
    #[schemars(description = "All discovered specifications in the workspace.")]
    pub specifications: Vec<ArtifactRecord>,
    #[serde(default)]
    #[schemars(description = "All discovered implementations in the workspace.")]
    pub implementations: Vec<ArtifactRecord>,
    #[serde(default)]
    #[schemars(description = "All discovered scratch pads in the workspace.")]
    pub scratchpads: Vec<ArtifactRecord>,
}

// Re-export for crate-local callers.
pub(crate) use specman::workspace_relative_path;

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

#[derive(Debug, PartialEq)]
enum ParsedResourceRequest {
    ArtifactContent(String),
    Dependencies(String),
    ComplianceReport(String),
    ConstraintsIndex(String),
    ConstraintContent(String, String),
}

impl std::str::FromStr for ParsedResourceRequest {
    type Err = McpError;

    fn from_str(uri: &str) -> Result<Self, Self::Err> {
        // Dependencies
        if let Some(base) = uri.strip_suffix("/dependencies") {
            return Ok(ParsedResourceRequest::Dependencies(base.to_string()));
        }

        // Compliance
        if let Some(base) = uri.strip_suffix("/compliance") {
            return Ok(ParsedResourceRequest::ComplianceReport(base.to_string()));
        }

        // Constraints Logic
        if uri.contains("/constraints//") {
            return Err(invalid_params(
                "malformed constraints locator (double slash)",
            ));
        }

        if let Some(base) = uri.strip_suffix("/constraints") {
            return Ok(ParsedResourceRequest::ConstraintsIndex(base.to_string()));
        }
        if let Some(base) = uri.strip_suffix("/constraints/") {
            return Ok(ParsedResourceRequest::ConstraintsIndex(base.to_string()));
        }
        if let Some((base, params)) = uri.split_once("/constraints/") {
            if params.is_empty() {
                return Ok(ParsedResourceRequest::ConstraintsIndex(base.to_string()));
            }
            return Ok(ParsedResourceRequest::ConstraintContent(
                base.to_string(),
                params.to_string(),
            ));
        }

        // Default to Artifact Content
        Ok(ParsedResourceRequest::ArtifactContent(uri.to_string()))
    }
}

impl ParsedResourceRequest {
    fn artifact_handle(&self) -> &str {
        match self {
            ParsedResourceRequest::ArtifactContent(h) => h,
            ParsedResourceRequest::Dependencies(h) => h,
            ParsedResourceRequest::ComplianceReport(h) => h,
            ParsedResourceRequest::ConstraintsIndex(h) => h,
            ParsedResourceRequest::ConstraintContent(h, _) => h,
        }
    }

    fn resolve_artifact_id(&self) -> Result<ArtifactId, McpError> {
        let handle = self.artifact_handle();

        let (kind, slug) = if let Some(rest) = handle.strip_prefix("spec://") {
            (ArtifactKind::Specification, rest)
        } else if let Some(rest) = handle.strip_prefix("impl://") {
            (ArtifactKind::Implementation, rest)
        } else if let Some(rest) = handle.strip_prefix("scratch://") {
            (ArtifactKind::ScratchPad, rest)
        } else {
            return Err(invalid_params(format!(
                "Artifact handle '{handle}' must use spec://, impl://, or scratch:// scheme"
            )));
        };

        if slug.contains('/') || slug.contains('\\') || slug.trim().is_empty() {
            return Err(invalid_params(format!(
                "Invalid artifact name in handle: '{handle}'"
            )));
        }

        Ok(ArtifactId {
            kind,
            name: slug.to_string(),
        })
    }
}

impl SpecmanMcpServer {
    async fn read_constraints_index(
        &self,
        uri: &str,
        base_locator: &str,
        artifact_id: &ArtifactId,
    ) -> Result<ResourceContents, McpError> {
        ensure_specification_artifact(artifact_id.kind)?;
        let workspace = self.workspace.workspace().map_err(to_mcp_error)?;

        let path = artifact_path(artifact_id, &workspace);
        let body = read_artifact_file(&path)?;
        let body = specman::front_matter::split_front_matter(&body).map_err(to_mcp_error)?;
        let lines: Vec<&str> = body.body.lines().collect();

        // Map constraint group -> (document-order line number, literal line)
        // by scanning the artifact body. This ensures `identifier_line` is truly literal.
        let mut identifier_lines: BTreeMap<String, (usize, String)> = BTreeMap::new();
        for (idx, raw) in lines.iter().enumerate() {
            let trimmed = raw.trim();
            if !trimmed.starts_with('!') || !trimmed.ends_with(':') {
                continue;
            }
            if trimmed.contains(' ') || trimmed.contains('\t') {
                continue;
            }

            let group = trimmed
                .trim_start_matches('!')
                .trim_end_matches(':')
                .to_string();
            identifier_lines
                .entry(group)
                .or_insert((idx + 1, (*raw).to_string()));
        }

        let workspace_path = workspace_relative_path(workspace.root(), &path).ok_or_else(|| {
            to_mcp_error(SpecmanError::Workspace(format!(
                "failed to resolve workspace-relative path for '{}'",
                path.display()
            )))
        })?;

        let indexer = FilesystemStructureIndexer::new(self.workspace.clone());
        let index = indexer
            .build_once_with_workspace(&workspace)
            .map_err(to_mcp_error)?;

        let mut entries: Vec<(usize, String, ConstraintIndexEntry)> = index
            .constraints
            .values()
            .filter(|record| {
                record.id.artifact.kind == ArtifactKind::Specification
                    && record.id.artifact.workspace_path == workspace_path
            })
            .map(|record| {
                let (doc_line, identifier_line) = identifier_lines
                    .get(&record.id.group)
                    .cloned()
                    .unwrap_or_else(|| (usize::MAX, format!("!{}:", record.id.group)));

                (
                    doc_line,
                    record.id.group.clone(),
                    ConstraintIndexEntry {
                        constraint_id: record.id.group.clone(),
                        identifier_line,
                        uri: format!("{}/constraints/{}", base_locator, record.id.group),
                    },
                )
            })
            .collect();

        // Deterministic: document order first, then stable group id.
        entries.sort_by(|(a_line, a_id, _), (b_line, b_id, _)| {
            a_line.cmp(b_line).then_with(|| a_id.cmp(b_id))
        });

        let entries: Vec<ConstraintIndexEntry> = entries.into_iter().map(|(_, _, e)| e).collect();

        let index = ConstraintIndex {
            artifact: base_locator.to_string(),
            constraints: entries,
        };

        let json = serde_json::to_string(&index)
            .map_err(|err| to_mcp_error(SpecmanError::Serialization(err.to_string())))?;

        Ok(ResourceContents::TextResourceContents {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: json,
            meta: None,
        })
    }

    async fn read_constraint_content(
        &self,
        uri: &str,
        base_locator: &str,
        artifact_id: &ArtifactId,
        constraint_id: &str,
    ) -> Result<ResourceContents, McpError> {
        ensure_specification_artifact(artifact_id.kind)?;
        validate_constraint_id(constraint_id)?;
        let workspace = self.workspace.workspace().map_err(to_mcp_error)?;

        // Validate the constraint exists via the structure index for exact matching.
        let path = artifact_path(artifact_id, &workspace);
        let workspace_path = workspace_relative_path(workspace.root(), &path).ok_or_else(|| {
            to_mcp_error(SpecmanError::Workspace(format!(
                "failed to resolve workspace-relative path for '{}'",
                path.display()
            )))
        })?;

        let indexer = FilesystemStructureIndexer::new(self.workspace.clone());
        let index = indexer
            .build_once_with_workspace(&workspace)
            .map_err(to_mcp_error)?;

        let key = ConstraintIdentifier {
            artifact: ArtifactKey {
                kind: ArtifactKind::Specification,
                workspace_path,
            },
            group: constraint_id.to_string(),
        };

        if !index.constraints.contains_key(&key) {
            return Err(invalid_params(format!(
                "Constraint '{constraint_id}' not found in specification {base_locator}"
            )));
        }
        let body = read_artifact_file(&path)?;
        let body = specman::front_matter::split_front_matter(&body).map_err(to_mcp_error)?;

        let text = extract_constraint_block(body.body, constraint_id).ok_or_else(|| {
            to_mcp_error(SpecmanError::Workspace(format!(
                "failed to extract constraint '{constraint_id}' from {base_locator}"
            )))
        })?;

        Ok(ResourceContents::TextResourceContents {
            uri: uri.to_string(),
            mime_type: Some("text/markdown".to_string()),
            text,
            meta: None,
        })
    }

    async fn read_compliance_report(
        &self,
        uri: &str,
        artifact_id: &ArtifactId,
    ) -> Result<ResourceContents, McpError> {
        let workspace = self.workspace.workspace().map_err(to_mcp_error)?;
        let root = workspace.root().to_path_buf();
        let impl_id = artifact_id.clone();

        let report = tokio::task::spawn_blocking(move || {
            specman::validation::validate_compliance(&root, &impl_id)
        })
        .await
        .map_err(|e| {
            to_mcp_error(SpecmanError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e,
            )))
        })?
        .map_err(to_mcp_error)?;

        let json = serde_json::to_string(&report)
            .map_err(|err| to_mcp_error(SpecmanError::Serialization(err.to_string())))?;

        Ok(ResourceContents::TextResourceContents {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: json,
            meta: None,
        })
    }

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
        let request: ParsedResourceRequest = uri.parse()?;

        // Validate constraint requests are only for specifications
        if let ParsedResourceRequest::ConstraintsIndex(base)
        | ParsedResourceRequest::ConstraintContent(base, _) = &request
        {
            if !base.starts_with("spec://") {
                return Err(invalid_params(
                    "'/constraints' resources are only available for spec:// artifacts",
                ));
            }
        }

        match &request {
            ParsedResourceRequest::Dependencies(handle) => {
                let tree = self
                    .dependency_mapper
                    .dependency_tree_from_locator(handle)
                    .map_err(to_mcp_error)?;
                let json = serde_json::to_string(&tree)
                    .map_err(|err| to_mcp_error(SpecmanError::Serialization(err.to_string())))?;
                Ok(ResourceContents::TextResourceContents {
                    uri: uri.to_string(),
                    mime_type: Some("application/json".to_string()),
                    text: json,
                    meta: None,
                })
            }
            ParsedResourceRequest::ComplianceReport(_) => {
                let artifact_id = request.resolve_artifact_id()?;
                self.read_compliance_report(uri, &artifact_id).await
            }
            ParsedResourceRequest::ConstraintsIndex(base) => {
                let artifact_id = request.resolve_artifact_id()?;
                self.read_constraints_index(uri, base, &artifact_id).await
            }
            ParsedResourceRequest::ConstraintContent(base, id) => {
                let artifact_id = request.resolve_artifact_id()?;
                self.read_constraint_content(uri, base, &artifact_id, id)
                    .await
            }
            ParsedResourceRequest::ArtifactContent(_) => {
                let artifact_id = request.resolve_artifact_id()?;
                let workspace = self.workspace.workspace().map_err(to_mcp_error)?;
                let path = artifact_path(&artifact_id, &workspace);
                let body = read_artifact_file(&path)?;
                Ok(ResourceContents::TextResourceContents {
                    uri: uri.to_string(),
                    mime_type: Some("text/markdown".to_string()),
                    text: body,
                    meta: None,
                })
            }
        }
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
                uri_template: "spec://{artifact}/constraints".to_string(),
                name: "spec-constraints-index".to_string(),
                title: Some("Specification constraints index".to_string()),
                description: Some("Return constraint index JSON for a specification".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            annotations: None,
        },
        ResourceTemplate {
            raw: RawResourceTemplate {
                uri_template: "spec://{artifact}/constraints/{constraint_id}".to_string(),
                name: "spec-constraint-content".to_string(),
                title: Some("Specification constraint content".to_string()),
                description: Some("Read a specific constraint group as Markdown".to_string()),
                mime_type: Some("text/markdown".to_string()),
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
        ResourceTemplate {
            raw: RawResourceTemplate {
                uri_template: "impl://{artifact}/compliance".to_string(),
                name: "impl-compliance".to_string(),
                title: Some("Implementation compliance report".to_string()),
                description: Some(
                    "Return compliance coverage JSON for an implementation".to_string(),
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
