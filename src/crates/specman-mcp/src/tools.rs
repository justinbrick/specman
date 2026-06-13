use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::schemars::JsonSchema;
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};

use specman::{
    ArtifactId, ArtifactKind, FrontMatterUpdate, IdentityUpdate, ImplementationUpdate,
    PersistedArtifact, ScratchFixMetadata, ScratchRefactorMetadata, ScratchRevisionMetadata,
    ScratchUpdate, ScratchWorkType, ScratchWorkloadExtras, SpecificationUpdate, SpecmanEnv,
    WorkspaceLocator, apply_front_matter_update,
};

use crate::error::{McpError, invalid_params, to_mcp_error};
use crate::resources::{
    artifact_handle, artifact_path, resolved_path_or_artifact_path, workspace_relative_path,
};
use crate::server::SpecmanMcpServer;
use tracing::{debug, info, instrument};

// ── Create tool input types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateSpecificationArgs {
    #[schemars(description = "Specification slug/name (lowercase, digits, hyphens).")]
    pub name: String,
    #[schemars(description = "Human-readable specification title.")]
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateImplementationArgs {
    #[schemars(description = "Implementation slug/name (lowercase, digits, hyphens).")]
    pub name: String,
    #[schemars(
        description = "Target specification locator (spec://... or workspace-relative path)."
    )]
    pub target: String,
}

/// Shared args for all scratch pad create tools.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateScratchPadArgs {
    #[schemars(description = "Scratch pad name (lowercase, digits, hyphens, <=4 words).")]
    pub name: String,
    #[schemars(
        description = "Target artifact locator. For revisions: a spec. For features/refactors/fixes: an impl."
    )]
    pub target: String,
}

// ── Update tool input types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceMode {
    #[schemars(description = "Persist the updated artifact to disk.")]
    Persist,
    #[schemars(description = "Preview-only: return the updated document without writing to disk.")]
    Preview,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateSpecificationArgs {
    #[schemars(
        description = "Artifact locator: workspace-relative path, spec://... handle, or HTTPS URL."
    )]
    pub locator: String,
    pub mode: PersistenceMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_implementation: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<Vec<specman::DependencyEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateImplementationArgs {
    #[schemars(
        description = "Artifact locator: workspace-relative path, impl://... handle, or HTTPS URL."
    )]
    pub locator: String,
    pub mode: PersistenceMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub references: Option<Vec<specman::ReferenceEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<Vec<specman::DependencyEntry>>,
}

/// Shared args for all scratch pad update tools.
/// No `target` (immutable) and no `work_type` (determined by tool name).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateScratchPadArgs {
    #[schemars(
        description = "Artifact locator: workspace-relative path or scratch://... handle. HTTPS not accepted."
    )]
    pub locator: String,
    pub mode: PersistenceMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<Vec<specman::DependencyEntry>>,
}

// ── Result types ─────────────────────────────────────────────────────

/// Result payload returned by all create tools.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateArtifactResult {
    #[schemars(description = "Stable artifact identifier (kind + name).")]
    pub id: ArtifactId,
    #[schemars(
        description = "Canonical artifact handle (e.g. 'spec://name', 'impl://name', 'scratch://name')."
    )]
    pub handle: String,
    #[schemars(description = "Workspace-relative path to the created artifact markdown file.")]
    pub path: String,
}

/// Result payload returned by all update tools.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateArtifactResult {
    pub id: ArtifactId,
    pub handle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub updated_document: String,
    pub persisted: bool,
}

/// Structured workspace data exposed over MCP tools.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceInfo {
    #[schemars(description = "Workspace root directory (absolute path string).")]
    pub root: String,
    #[schemars(description = "Path to the workspace '.specman' directory (absolute path string).")]
    pub dot_specman: String,
    #[schemars(description = "Path to the workspace 'spec/' directory (absolute path string).")]
    pub spec_dir: String,
    #[schemars(description = "Path to the workspace 'impl/' directory (absolute path string).")]
    pub impl_dir: String,
    #[schemars(description = "Path to the workspace scratchpad directory (absolute path string).")]
    pub scratchpad_dir: String,
}

// ── Router ───────────────────────────────────────────────────────────

pub(crate) fn build_tool_router() -> ToolRouter<SpecmanMcpServer> {
    SpecmanMcpServer::tool_router()
}

// ── Tool methods ─────────────────────────────────────────────────────

#[tool_router]
impl SpecmanMcpServer {
    // ── Create tools ─────────────────────────────────────────────

    #[tool(
        name = "create_specification",
        description = "Create a specification artifact (spec/{name}/spec.md)."
    )]
    pub(crate) async fn create_specification(
        &self,
        Parameters(args): Parameters<CreateSpecificationArgs>,
    ) -> Result<Json<CreateArtifactResult>, McpError> {
        info!("create_specification request received");
        validate_slug(&args.name, "specification")?;
        let env = self.build_env()?;
        let result = specman::create_specification(
            &env,
            specman::CreateSpecOptions {
                name: args.name.clone(),
                title: args.title.clone(),
                dry_run: false,
                front_matter: None,
            },
        )
        .map_err(to_mcp_error)?;

        let persisted = match result {
            specman::CreateResult::Persisted(p) => p,
            specman::CreateResult::DryRun(_) => unreachable!(),
        };
        self.invalidate_dependency_inventory();
        let r = create_artifact_result(&persisted);
        info!(handle = %r.handle, path = %r.path, "create_specification completed");
        Ok(Json(r))
    }

    #[tool(
        name = "create_implementation",
        description = "Create an implementation artifact (impl/{name}/impl.md)."
    )]
    pub(crate) async fn create_implementation(
        &self,
        Parameters(args): Parameters<CreateImplementationArgs>,
    ) -> Result<Json<CreateArtifactResult>, McpError> {
        info!("create_implementation request received");
        validate_slug(&args.name, "implementation")?;

        let target_summary = self.normalize_locator_to_handle(&args.target)?;
        if target_summary.id.kind != ArtifactKind::Specification {
            return Err(invalid_params(
                "implementation targets must resolve to a specification (spec://... or a spec path)",
            ));
        }
        let target_handle = artifact_handle(&target_summary);
        let env = self.build_env()?;
        let result = specman::create_implementation(
            &env,
            specman::CreateImplOptions {
                name: args.name.clone(),
                target: target_handle,
                dry_run: false,
                front_matter: None,
            },
        )
        .map_err(to_mcp_error)?;

        let persisted = match result {
            specman::CreateResult::Persisted(p) => p,
            specman::CreateResult::DryRun(_) => unreachable!(),
        };
        self.invalidate_dependency_inventory();
        let r = create_artifact_result(&persisted);
        info!(handle = %r.handle, path = %r.path, "create_implementation completed");
        Ok(Json(r))
    }

    #[tool(
        name = "create_revision",
        description = "Create a revision scratch pad (a planning document for a specification revision). This does not modify the specification itself."
    )]
    pub(crate) async fn create_revision(
        &self,
        Parameters(args): Parameters<CreateScratchPadArgs>,
    ) -> Result<Json<CreateArtifactResult>, McpError> {
        self.create_scratch_pad(
            &args.name,
            &args.target,
            ScratchWorkType::Revision(ScratchRevisionMetadata::default()),
        )
        .await
    }

    #[tool(
        name = "create_feature",
        description = "Create a feature scratch pad (a planning document for introducing a feature). This does not create the feature implementation itself."
    )]
    pub(crate) async fn create_feature(
        &self,
        Parameters(args): Parameters<CreateScratchPadArgs>,
    ) -> Result<Json<CreateArtifactResult>, McpError> {
        self.create_scratch_pad(
            &args.name,
            &args.target,
            ScratchWorkType::Feat(ScratchWorkloadExtras::default()),
        )
        .await
    }

    #[tool(
        name = "create_refactor",
        description = "Create a refactor scratch pad (a planning document for refactoring an implementation)."
    )]
    pub(crate) async fn create_refactor(
        &self,
        Parameters(args): Parameters<CreateScratchPadArgs>,
    ) -> Result<Json<CreateArtifactResult>, McpError> {
        self.create_scratch_pad(
            &args.name,
            &args.target,
            ScratchWorkType::Refactor(ScratchRefactorMetadata::default()),
        )
        .await
    }

    #[tool(
        name = "create_fix",
        description = "Create a fix scratch pad (a planning document for applying a fix to an implementation)."
    )]
    pub(crate) async fn create_fix(
        &self,
        Parameters(args): Parameters<CreateScratchPadArgs>,
    ) -> Result<Json<CreateArtifactResult>, McpError> {
        self.create_scratch_pad(
            &args.name,
            &args.target,
            ScratchWorkType::Fix(ScratchFixMetadata::default()),
        )
        .await
    }

    // ── Update tools ─────────────────────────────────────────────

    #[tool(
        name = "update_specification",
        description = "Update YAML front matter metadata for a specification artifact. Supports preview and persist modes."
    )]
    pub(crate) async fn update_specification(
        &self,
        Parameters(args): Parameters<UpdateSpecificationArgs>,
    ) -> Result<Json<UpdateArtifactResult>, McpError> {
        let update = FrontMatterUpdate::Specification(SpecificationUpdate {
            identity: identity_from_args(
                &args.name,
                &args.title,
                &args.description,
                &args.version,
                &args.tags,
            ),
            requires_implementation: args.requires_implementation,
            dependencies: args.dependencies.clone(),
        });
        self.do_update(
            ArtifactKind::Specification,
            &args.locator,
            args.mode,
            &update,
        )
        .await
    }

    #[tool(
        name = "update_implementation",
        description = "Update YAML front matter metadata for an implementation artifact. Supports preview and persist modes."
    )]
    pub(crate) async fn update_implementation(
        &self,
        Parameters(args): Parameters<UpdateImplementationArgs>,
    ) -> Result<Json<UpdateArtifactResult>, McpError> {
        let update = FrontMatterUpdate::Implementation(ImplementationUpdate {
            identity: identity_from_args(
                &args.name,
                &args.title,
                &args.description,
                &args.version,
                &args.tags,
            ),
            spec: args.spec.clone(),
            location: args.location.clone(),
            references: args.references.clone(),
            dependencies: args.dependencies.clone(),
        });
        self.do_update(
            ArtifactKind::Implementation,
            &args.locator,
            args.mode,
            &update,
        )
        .await
    }

    #[tool(
        name = "update_revision",
        description = "Update YAML front matter metadata for a revision scratch pad (a planning document). Supports preview and persist modes."
    )]
    pub(crate) async fn update_revision(
        &self,
        Parameters(args): Parameters<UpdateScratchPadArgs>,
    ) -> Result<Json<UpdateArtifactResult>, McpError> {
        self.update_scratch_pad(args).await
    }

    #[tool(
        name = "update_feature",
        description = "Update YAML front matter metadata for a feature scratch pad (a planning document). Supports preview and persist modes."
    )]
    pub(crate) async fn update_feature(
        &self,
        Parameters(args): Parameters<UpdateScratchPadArgs>,
    ) -> Result<Json<UpdateArtifactResult>, McpError> {
        self.update_scratch_pad(args).await
    }

    #[tool(
        name = "update_refactor",
        description = "Update YAML front matter metadata for a refactor scratch pad (a planning document). Supports preview and persist modes."
    )]
    pub(crate) async fn update_refactor(
        &self,
        Parameters(args): Parameters<UpdateScratchPadArgs>,
    ) -> Result<Json<UpdateArtifactResult>, McpError> {
        self.update_scratch_pad(args).await
    }

    #[tool(
        name = "update_fix",
        description = "Update YAML front matter metadata for a fix scratch pad (a planning document). Supports preview and persist modes."
    )]
    pub(crate) async fn update_fix(
        &self,
        Parameters(args): Parameters<UpdateScratchPadArgs>,
    ) -> Result<Json<UpdateArtifactResult>, McpError> {
        self.update_scratch_pad(args).await
    }
}

// ── Internal helpers ─────────────────────────────────────────────────

impl SpecmanMcpServer {
    #[instrument(level = "info", skip(self))]
    async fn create_scratch_pad(
        &self,
        name: &str,
        target: &str,
        work_type: ScratchWorkType,
    ) -> Result<Json<CreateArtifactResult>, McpError> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err(invalid_params("scratch pad name must not be empty"));
        }
        validate_slug_max_words(trimmed_name, "scratch pad", 4)?;

        let trimmed_target = target.trim();
        if trimmed_target.starts_with("https://") || trimmed_target.starts_with("http://") {
            return Err(invalid_params(
                "workspace target locators must not be URLs; use spec://, impl://, scratch://, or a workspace-relative path",
            ));
        }
        let resolved_target = self.normalize_locator_to_workspace_path(trimmed_target)?;

        let env = self.build_env()?;
        let result = specman::create_scratch_pad(
            &env,
            specman::CreateScratchOptions {
                name: trimmed_name.to_string(),
                target: resolved_target,
                work_type,
                dry_run: false,
                front_matter: None,
            },
        )
        .map_err(to_mcp_error)?;

        let persisted = match result {
            specman::CreateResult::Persisted(p) => p,
            specman::CreateResult::DryRun(_) => unreachable!(),
        };
        self.invalidate_dependency_inventory();
        let r = create_artifact_result(&persisted);
        info!(handle = %r.handle, path = %r.path, "scratch pad created");
        Ok(Json(r))
    }

    #[instrument(level = "info", skip(self, update))]
    async fn do_update(
        &self,
        expected_kind: ArtifactKind,
        locator: &str,
        mode: PersistenceMode,
        update: &FrontMatterUpdate,
    ) -> Result<Json<UpdateArtifactResult>, McpError> {
        debug!(?expected_kind, %locator, "update start");

        let locator = locator.trim();
        if locator.is_empty() {
            return Err(invalid_params("locator must not be empty"));
        }
        if locator.ends_with("/dependencies") || locator.contains("/dependencies/") {
            return Err(invalid_params(
                "mutation is not supported for '/dependencies' derived locators",
            ));
        }
        if locator.ends_with("/constraints") || locator.contains("/constraints/") {
            return Err(invalid_params(
                "mutation is not supported for '/constraints' derived locators",
            ));
        }
        if locator.starts_with("http://") {
            return Err(invalid_params(
                "unsupported url scheme in locator; use https",
            ));
        }

        let persist = matches!(mode, PersistenceMode::Persist);

        if locator.starts_with("https://") {
            if persist {
                return Err(invalid_params(
                    "persist is not supported for HTTPS locators; use mode=preview",
                ));
            }
            info!(%locator, "fetching https artifact for preview update");
            let raw = fetch_https_document(locator).await?;
            let id = ArtifactId {
                kind: expected_kind,
                name: derive_name_from_https(locator),
            };
            let workspace = self.workspace.workspace().map_err(to_mcp_error)?;
            let fake_path = workspace.root().join("remote.md");
            let (updated_document, _) =
                apply_front_matter_update(&id, &fake_path, &workspace, &raw, update, false)
                    .map_err(to_mcp_error)?;
            return Ok(Json(UpdateArtifactResult {
                id,
                handle: locator.to_string(),
                path: None,
                updated_document,
                persisted: false,
            }));
        }

        let workspace = self.workspace.workspace().map_err(to_mcp_error)?;
        let tree = self
            .dependency_mapper
            .dependency_tree_from_locator(locator)
            .map_err(to_mcp_error)?;

        if tree.root.id.kind != expected_kind {
            return Err(invalid_params(format!(
                "artifact kind mismatch: expected {:?} but locator resolved to {:?}",
                expected_kind, tree.root.id.kind
            )));
        }

        let relative = self.workspace_relative_artifact_path(&tree.root, &workspace)?;
        let artifact_path = artifact_path(&tree.root.id, &workspace);
        let content = std::fs::read_to_string(&artifact_path)
            .map_err(|err| to_mcp_error(specman::SpecmanError::Workspace(err.to_string())))?;

        let (updated_document, _) = specman::apply_front_matter_update(
            &tree.root.id,
            &artifact_path,
            &workspace,
            &content,
            update,
            persist,
        )
        .map_err(to_mcp_error)?;

        if persist {
            self.invalidate_dependency_inventory();
        }

        let handle = artifact_handle(&tree.root);
        Ok(Json(UpdateArtifactResult {
            id: tree.root.id.clone(),
            handle,
            path: Some(relative),
            updated_document,
            persisted: persist,
        }))
    }

    async fn update_scratch_pad(
        &self,
        args: UpdateScratchPadArgs,
    ) -> Result<Json<UpdateArtifactResult>, McpError> {
        let locator = args.locator.trim();
        if locator.starts_with("https://") {
            return Err(invalid_params(
                "HTTPS locators are not supported for scratch pads",
            ));
        }
        let update = FrontMatterUpdate::Scratch(ScratchUpdate {
            identity: identity_from_args(
                &args.name,
                &args.title,
                &args.description,
                &args.version,
                &args.tags,
            ),
            work_type: None,
            dependencies: args.dependencies.clone(),
        });
        self.do_update(ArtifactKind::ScratchPad, locator, args.mode, &update)
            .await
    }
}

// ── Shared helpers ───────────────────────────────────────────────────

fn identity_from_args(
    name: &Option<String>,
    title: &Option<String>,
    description: &Option<String>,
    version: &Option<String>,
    tags: &Option<Vec<String>>,
) -> IdentityUpdate {
    IdentityUpdate {
        name: name.clone(),
        title: title.clone(),
        description: description.clone(),
        version: version.clone(),
        tags: tags.clone(),
    }
}

fn create_artifact_result(persisted: &PersistedArtifact) -> CreateArtifactResult {
    let handle = match persisted.artifact.kind {
        ArtifactKind::Specification => format!("spec://{}", persisted.artifact.name),
        ArtifactKind::Implementation => format!("impl://{}", persisted.artifact.name),
        ArtifactKind::ScratchPad => format!("scratch://{}", persisted.artifact.name),
    };
    CreateArtifactResult {
        id: persisted.artifact.clone(),
        handle,
        path: persisted
            .path
            .strip_prefix(persisted.workspace.root())
            .unwrap_or(&persisted.path)
            .to_string_lossy()
            .to_string(),
    }
}

async fn fetch_https_document(url: &str) -> Result<String, McpError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|err| invalid_params(format!("invalid https url: {err}")))?;
    if parsed.scheme() != "https" {
        return Err(invalid_params(
            "unsupported url scheme in locator; use https",
        ));
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|err| rmcp::model::ErrorData::internal_error(err.to_string(), None))?;
    let response = client
        .get(parsed)
        .send()
        .await
        .map_err(|err| rmcp::model::ErrorData::internal_error(err.to_string(), None))?;
    let status = response.status();
    if !status.is_success() {
        return Err(invalid_params(format!(
            "failed to fetch https locator (status={status})"
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| rmcp::model::ErrorData::internal_error(err.to_string(), None))?;
    const MAX_BYTES: usize = 2 * 1024 * 1024;
    if bytes.len() > MAX_BYTES {
        return Err(invalid_params(format!(
            "https document too large ({} bytes; max {MAX_BYTES})",
            bytes.len()
        )));
    }
    String::from_utf8(bytes.to_vec())
        .map_err(|err| invalid_params(format!("https response was not utf-8: {err}")))
}

fn derive_name_from_https(url: &str) -> String {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return "remote".to_string();
    };
    let candidate = parsed
        .path_segments()
        .and_then(|mut segs| segs.next_back())
        .unwrap_or("remote");
    let candidate = candidate.split('.').next().unwrap_or(candidate).trim();
    let mut out = String::new();
    let mut last_dash = false;
    for ch in candidate.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "remote".to_string()
    } else {
        out
    }
}

fn validate_slug(value: &str, kind: &str) -> Result<(), McpError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(invalid_params(format!("{kind} name must not be empty")));
    }
    if trimmed
        .chars()
        .any(|ch| !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'))
    {
        return Err(invalid_params(format!(
            "{kind} name '{trimmed}' must be lowercase alphanumeric with hyphen separators"
        )));
    }
    Ok(())
}

fn validate_slug_max_words(value: &str, kind: &str, max_words: usize) -> Result<(), McpError> {
    validate_slug(value, kind)?;
    let segments: Vec<_> = value.split('-').filter(|seg| !seg.is_empty()).collect();
    if segments.len() > max_words {
        return Err(invalid_params(format!(
            "{kind} name '{value}' must contain no more than {max_words} hyphenated words"
        )));
    }
    Ok(())
}

// ── SpecmanMcpServer internal helpers ────────────────────────────────

impl SpecmanMcpServer {
    fn build_env(&self) -> Result<SpecmanEnv, McpError> {
        SpecmanEnv::new(self.workspace.clone(), None).map_err(to_mcp_error)
    }

    fn invalidate_dependency_inventory(&self) {
        self.dependency_mapper
            .dependency_graph()
            .invalidate_inventory();
    }

    fn workspace_relative_artifact_path(
        &self,
        summary: &specman::ArtifactSummary,
        workspace: &specman::WorkspacePaths,
    ) -> Result<String, McpError> {
        let resolved = resolved_path_or_artifact_path(summary, workspace);
        let mut absolute = std::path::PathBuf::from(&resolved);
        if absolute.is_relative() {
            absolute = workspace.root().join(&absolute);
        }

        workspace_relative_path(workspace.root(), &absolute)
            .or_else(|| {
                let fallback = artifact_path(&summary.id, workspace);
                workspace_relative_path(workspace.root(), &fallback)
            })
            .ok_or_else(|| invalid_params("locator must resolve within the workspace"))
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

        self.workspace_relative_artifact_path(&tree.root, &workspace)
    }

    fn normalize_locator_to_handle(
        &self,
        locator: &str,
    ) -> Result<specman::ArtifactSummary, McpError> {
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
}
