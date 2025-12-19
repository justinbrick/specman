use std::path::PathBuf;

use rmcp::RoleServer;
use rmcp::elicit_safe;
use rmcp::model::{
    Content, ContextInclusion, CreateMessageRequestParam, Role as SamplingRole, SamplingMessage,
};
use rmcp::service::Peer;
use serde_yaml::{Mapping, Value as YamlValue};

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use std::borrow::Cow;

use rmcp::schemars::JsonSchema;
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};

use specman::{
    ArtifactId, ArtifactKind, CreateRequest, DefaultLifecycleController,
    FilesystemDependencyMapper, FilesystemWorkspaceLocator, MarkdownTemplateEngine,
    PersistedArtifact, Specman, TemplateCatalog, WorkspaceLocator, WorkspacePersistence,
};

use crate::error::{McpError, invalid_params, to_mcp_error};
use crate::resources::{artifact_handle, resolved_path_or_artifact_path, workspace_relative_path};
use crate::server::SpecmanMcpServer;

type SpecmanInstance = Specman<
    FilesystemDependencyMapper<std::sync::Arc<FilesystemWorkspaceLocator>>,
    MarkdownTemplateEngine,
    std::sync::Arc<FilesystemWorkspaceLocator>,
>;

// NOTE: The MCP tool schema is derived from `CreateArtifactArgs` below.

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScratchKind {
    #[schemars(description = "Draft scratch pad work type.")]
    Draft,
    #[schemars(description = "Revision scratch pad work type.")]
    Revision,
    #[schemars(description = "Feature scratch pad work type.")]
    Feat,
    #[schemars(description = "Refactor scratch pad work type.")]
    Ref,
    #[schemars(description = "Fix scratch pad work type.")]
    Fix,
}

impl ScratchKind {
    fn as_work_type_key(&self) -> &'static str {
        match self {
            ScratchKind::Draft => "draft",
            ScratchKind::Revision => "revision",
            ScratchKind::Feat => "feat",
            ScratchKind::Ref => "ref",
            ScratchKind::Fix => "fix",
        }
    }
}

/// `create_artifact` input schema for MCP.
///
/// This uses an internally-tagged enum (`kind`) so that each artifact type can have a
/// clear, typed, and schema-enforced set of inputs.
///
/// The server intentionally does NOT accept arbitrary template substitutions. It is responsible
/// for gathering any missing details via MCP sampling + elicitation and then mapping
/// the result into a SpecMan core `CreateRequest`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CreateArtifactArgs {
    /// Create a specification under `spec/`.
    Specification {
        /// Optional natural-language intent provided by the caller.
        /// Used to guide sampling (e.g. title/name suggestions, affected headings).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,

        /// Optional name hint. The server MAY still request explicit confirmation via elicitation.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,

        /// Optional title hint.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },

    /// Create an implementation under `impl/`.
    Implementation {
        /// Target locator for implementation creation.
        ///
        /// MUST resolve to a specification (e.g. `spec://...`).
        target: String,

        /// Optional natural-language intent provided by the caller.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,

        /// Optional name hint.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },

    /// Create a scratch pad under `templates/scratch/` (or project scratchpad dir).
    ScratchPad {
        /// Target locator for scratch pad creation.
        ///
        /// MUST resolve within the workspace and MUST NOT be an HTTP(S) URL.
        target: String,

        /// Scratch work type selector.
        #[serde(rename = "scratchKind", alias = "scratch_kind")]
        scratch_kind: ScratchKind,

        /// Required natural-language intent provided by the caller.
        /// Used to guide sampling (especially scratch pad name inference).
        intent: String,
    },
}

// NOTE:
// The MCP Inspector currently expects tool `inputSchema.type` to be exactly
// the string "object".
//
// For an internally-tagged enum, schemars can emit a schema whose top-level
// object does not include a concrete `type: "object"` (e.g. only `oneOf`).
// That’s valid JSON Schema, but it fails MCP Inspector validation.
//
// To keep MCP Inspector happy, we handcraft the schema in `CreateArtifactArgs::json_schema`.

impl JsonSchema for CreateArtifactArgs {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("CreateArtifactArgs")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let scratch_kind_schema = generator.subschema_for::<ScratchKind>();

        schemars::json_schema!({
            "type": "object",
            "properties": {
                "kind": {
                    "type": "string",
                    "description": "Discriminator selecting which artifact shape is being requested ('specification', 'implementation', or 'scratch_pad')."
                }
            },
            "required": ["kind"],
            "oneOf": [
                {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "kind": {
                            "const": "specification",
                            "type": "string",
                            "description": "Selects the specification creation request shape."
                        },
                        "intent": {
                            "type": "string",
                            "description": "Optional natural-language intent to guide sampling and prompt generation."
                        },
                        "name": {
                            "type": "string",
                            "description": "Optional slug/name hint for the new specification (may still require confirmation)."
                        },
                        "title": {
                            "type": "string",
                            "description": "Optional human-readable title hint for the new specification."
                        }
                    },
                    "required": ["kind"]
                },
                {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "kind": {
                            "const": "implementation",
                            "type": "string",
                            "description": "Selects the implementation creation request shape."
                        },
                        "target": {
                            "type": "string",
                            "description": "Target locator that MUST resolve to a specification (e.g. 'spec://...')."
                        },
                        "intent": {
                            "type": "string",
                            "description": "Optional natural-language intent to guide sampling and prompt generation."
                        },
                        "name": {
                            "type": "string",
                            "description": "Optional slug/name hint for the new implementation (may still require confirmation)."
                        }
                    },
                    "required": ["kind", "target"]
                },
                {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "kind": {
                            "const": "scratch_pad",
                            "type": "string",
                            "description": "Selects the scratch pad creation request shape."
                        },
                        "target": {
                            "type": "string",
                            "description": "Target locator for scratch pad creation. MUST resolve within the workspace and MUST NOT be an HTTP(S) URL."
                        },
                        "scratchKind": {
                            "allOf": [scratch_kind_schema],
                            "description": "Scratch pad work type selector ('feat', 'ref', 'revision', 'fix', or 'draft')."
                        },
                        "intent": {
                            "type": "string",
                            "description": "Required natural-language intent to guide sampling and prompt generation."
                        }
                    },
                    "required": ["kind", "target", "scratchKind", "intent"]
                }
            ]
        })
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct NameSuggestion {
    #[schemars(description = "Suggested slug/name.")]
    name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct SpecSuggestion {
    #[schemars(description = "Suggested specification slug/name.")]
    name: String,
    #[schemars(description = "Suggested specification title.")]
    title: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct NameOverride {
    /// Leave empty to accept the proposed name.
    #[serde(default)]
    #[schemars(description = "Name override input. Leave empty to accept the proposed name.")]
    name: String,
}

elicit_safe!(NameOverride);

pub(crate) fn build_tool_router() -> ToolRouter<SpecmanMcpServer> {
    SpecmanMcpServer::tool_router()
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

/// Deterministic result payload returned by the `create_artifact` MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateArtifactResult {
    #[schemars(description = "Stable artifact identifier (kind + name).")]
    pub id: ArtifactId,
    #[schemars(
        description = "Canonical artifact handle (e.g. 'spec://name', 'impl://name', 'scratch://name')."
    )]
    pub handle: String,
    /// Canonical workspace-relative path to the created artifact.
    #[schemars(description = "Workspace-relative path to the created artifact markdown file.")]
    pub path: String,
}

#[tool_router]
impl SpecmanMcpServer {
    #[tool(
        name = "create_artifact",
        description = "Create a SpecMan artifact (spec, impl, or scratch pad). Accepts minimal inputs and uses MCP sampling + elicitation to fill gaps."
    )]
    pub(crate) async fn create_artifact(
        &self,
        peer: Peer<RoleServer>,
        Parameters(args): Parameters<CreateArtifactArgs>,
    ) -> Result<Json<CreateArtifactResult>, McpError> {
        self.create_artifact_internal(Some(&peer), args).await
    }
}

impl SpecmanMcpServer {
    pub(crate) async fn create_artifact_internal(
        &self,
        peer: Option<&Peer<RoleServer>>,
        args: CreateArtifactArgs,
    ) -> Result<Json<CreateArtifactResult>, McpError> {
        let specman = self.build_specman()?;

        let request = self
            .build_create_request_via_sampling_and_elicitation(peer, &args)
            .await?;
        let normalized = self.normalize_create_request(request)?;

        let persisted = match &normalized {
            CreateRequest::ScratchPad { context } => {
                self.create_scratchpad_with_front_matter(&specman, context)?
            }
            _ => specman.create(normalized).map_err(to_mcp_error)?,
        };

        Ok(Json(create_artifact_result(&persisted)))
    }

    async fn build_create_request_via_sampling_and_elicitation(
        &self,
        peer: Option<&Peer<RoleServer>>,
        args: &CreateArtifactArgs,
    ) -> Result<CreateRequest, McpError> {
        match args {
            CreateArtifactArgs::Specification {
                name,
                title,
                intent,
            } => {
                let suggestion = if let (Some(name), Some(title)) = (name.as_ref(), title.as_ref())
                {
                    SpecSuggestion {
                        name: name.clone(),
                        title: title.clone(),
                    }
                } else {
                    self.sample_json::<SpecSuggestion>(
                        peer,
                        "Propose a SpecMan specification name + title",
                        &format!(
                            "Return JSON matching this schema (and ONLY JSON):\n\n{}\n\n\
Intent (optional): {}\n\n\
Constraints:\n\
- name must be a slug (lowercase, digits, hyphens).\n\
- title should be human readable.\n",
                            schema_json_for::<SpecSuggestion>(),
                            intent.clone().unwrap_or_default()
                        ),
                    )
                    .await?
                };

                let name = self
                    .confirm_name(peer, suggestion.name, "specification")
                    .await?;
                validate_slug(&name, "specification")?;

                Ok(CreateRequest::Specification {
                    context: specman::SpecContext {
                        name,
                        title: suggestion.title,
                    },
                })
            }
            CreateArtifactArgs::Implementation {
                target,
                intent,
                name,
            } => {
                let target = target.clone();

                let suggested = match name.as_ref() {
                    Some(name) => NameSuggestion { name: name.clone() },
                    None => {
                        self.sample_json::<NameSuggestion>(
                            peer,
                            "Propose a SpecMan implementation name",
                            &format!(
                                "Return JSON matching this schema (and ONLY JSON):\n\n{}\n\n\
Target: {}\n\
Intent (optional): {}\n\n\
Constraints: name must be a slug (lowercase, digits, hyphens).\n",
                                schema_json_for::<NameSuggestion>(),
                                target,
                                intent.clone().unwrap_or_default()
                            ),
                        )
                        .await?
                    }
                };

                let name = self
                    .confirm_name(peer, suggested.name, "implementation")
                    .await?;
                validate_slug(&name, "implementation")?;

                Ok(CreateRequest::Implementation {
                    context: specman::ImplContext { name, target },
                })
            }
            CreateArtifactArgs::ScratchPad {
                target,
                scratch_kind,
                intent,
            } => {
                let target = target.clone();
                let kind = scratch_kind.clone();

                // Reject URL targets early so we don't require sampling just to fail later.
                let trimmed_target = target.trim();
                if trimmed_target.starts_with("https://") || trimmed_target.starts_with("http://") {
                    return Err(invalid_params(
                        "workspace target locators must not be URLs; use spec://, impl://, scratch://, or a workspace-relative path",
                    ));
                }

                let intent = intent.trim().to_string();
                if intent.is_empty() {
                    return Err(invalid_params(
                        "scratch pad intent is required and must not be empty",
                    ));
                }

                // Scratch pads should be fast to scaffold.
                // Always infer name via sampling (derived from the required intent).
                let proposed_name = if peer.is_some() {
                    self.sample_json::<NameSuggestion>(
                        peer,
                        "Propose a SpecMan scratch pad name",
                        &format!(
                            "Return JSON matching this schema (and ONLY JSON):\n\n{}\n\n\
Target: {}\n\
Work type: {}\n\
Intent (required): {}\n\n\
Constraints:\n\
- name must be all lowercase, digits, hyphen-separated, <=4 words.\n\
- prefer action verbs.\n",
                            schema_json_for::<NameSuggestion>(),
                            target,
                            kind.as_work_type_key(),
                            intent
                        ),
                    )
                    .await?
                    .name
                } else {
                    infer_scratch_pad_name_from_intent(&intent)
                };

                let name = proposed_name.trim().to_string();
                if name.is_empty() {
                    return Err(invalid_params("scratch pad name must not be empty"));
                }
                validate_slug_max_words(&name, "scratch pad", 4)?;

                let work_type = self.build_scratch_work_type(&kind);

                Ok(CreateRequest::ScratchPad {
                    context: specman::ScratchPadCreateContext {
                        name,
                        target,
                        work_type,
                    },
                })
            }
        }
    }

    async fn confirm_name(
        &self,
        peer: Option<&Peer<RoleServer>>,
        proposed: String,
        artifact_kind: &str,
    ) -> Result<String, McpError> {
        let proposed = proposed.trim().to_string();
        if proposed.is_empty() {
            return Err(invalid_params("sampled name must not be empty"));
        }

        // If the client supports elicitation, confirm explicitly.
        if let Some(peer) = peer {
            if peer.supports_elicitation() {
                let message = format!(
                    "Proposed {artifact_kind} name: '{proposed}'.\n\
Enter an alternate name, or leave blank to accept."
                );
                // Per prompt contract: blank OR no response means "accept proposed".
                // Elicitation is a best-effort UX improvement; if it fails (timeout, cancel,
                // client quirks), fall back to the proposed value.
                if let Ok(Some(override_name)) = peer
                    .elicit_with_timeout::<NameOverride>(message, None)
                    .await
                {
                    let trimmed = override_name.name.trim();
                    if !trimmed.is_empty() {
                        return Ok(trimmed.to_string());
                    }
                }
            }
        }

        Ok(proposed)
    }

    fn build_scratch_work_type(
        &self,
        kind: &ScratchKind,
    ) -> specman::front_matter::ScratchWorkType {
        use specman::front_matter::{
            ScratchFixMetadata, ScratchRefactorMetadata, ScratchRevisionMetadata, ScratchWorkType,
            ScratchWorkloadExtras,
        };

        match kind {
            ScratchKind::Draft => ScratchWorkType::Draft(ScratchWorkloadExtras::default()),
            ScratchKind::Feat => ScratchWorkType::Feat(ScratchWorkloadExtras::default()),
            ScratchKind::Revision => ScratchWorkType::Revision(ScratchRevisionMetadata {
                revised_headings: Vec::new(),
                ..Default::default()
            }),
            ScratchKind::Ref => ScratchWorkType::Refactor(ScratchRefactorMetadata {
                refactored_headings: Vec::new(),
                ..Default::default()
            }),
            ScratchKind::Fix => ScratchWorkType::Fix(ScratchFixMetadata {
                fixed_headings: Vec::new(),
                ..Default::default()
            }),
        }
    }

    async fn sample_json<T: for<'de> Deserialize<'de> + JsonSchema>(
        &self,
        peer: Option<&Peer<RoleServer>>,
        purpose: &str,
        prompt: &str,
    ) -> Result<T, McpError> {
        let peer = peer.ok_or_else(|| {
            invalid_params("sampling is required to infer missing inputs, but no peer is available")
        })?;
        let request = CreateMessageRequestParam {
            messages: vec![SamplingMessage {
                role: SamplingRole::User,
                content: Content::text(prompt.to_string()),
            }],
            model_preferences: None,
            system_prompt: Some(format!(
                "You are a deterministic assistant. Purpose: {purpose}. Output MUST be valid JSON only."
            )),
            include_context: Some(ContextInclusion::ThisServer),
            temperature: Some(0.0),
            max_tokens: 1200,
            stop_sequences: None,
            metadata: None,
        };

        let response = peer
            .create_message(request)
            .await
            .map_err(|err| invalid_params(err.to_string()))?;

        let text = response
            .message
            .content
            .as_text()
            .map(|t| t.text.clone())
            .unwrap_or_default();

        let normalized = Self::normalize_sampled_json(&text);
        serde_json::from_str::<T>(normalized.trim()).map_err(|err| {
            invalid_params(format!(
                "sampling response did not match expected JSON schema: {err}; raw={text}"
            ))
        })
    }

    fn normalize_sampled_json(text: &str) -> String {
        let trimmed = text.trim();
        if !trimmed.starts_with("```") {
            return trimmed.to_string();
        }

        let mut lines = trimmed.lines();
        let first = lines.next().unwrap_or_default();
        if !first.starts_with("```") {
            return trimmed.to_string();
        }

        let mut out = String::new();
        for line in lines {
            if line.trim_start().starts_with("```") {
                break;
            }
            out.push_str(line);
            out.push('\n');
        }

        out.trim().to_string()
    }
    fn create_scratchpad_with_front_matter(
        &self,
        specman: &SpecmanInstance,
        context: &specman::ScratchPadCreateContext,
    ) -> Result<PersistedArtifact, McpError> {
        let persisted = specman
            .create(CreateRequest::ScratchPad {
                context: context.clone(),
            })
            .map_err(to_mcp_error)?;

        let branch = default_branch_from_target(
            &context.target,
            scratch_work_type_key(&context.work_type),
            &context.name,
        );

        let content = std::fs::read_to_string(&persisted.path)
            .map_err(|err| invalid_params(format!("failed to read scratch pad: {err}")))?;
        let rewritten =
            rewrite_scratch_front_matter(&content, &context.target, &branch, &context.work_type)?;
        std::fs::write(&persisted.path, rewritten)
            .map_err(|err| invalid_params(format!("failed to write scratch pad: {err}")))?;

        Ok(persisted)
    }
}

fn schema_json_for<T: JsonSchema>() -> String {
    serde_json::to_string_pretty(&schemars::schema_for!(T)).unwrap_or_else(|_| "{}".to_string())
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

fn infer_scratch_pad_name_from_intent(intent: &str) -> String {
    // Best-effort, deterministic slug inference for environments without an MCP sampling peer.
    // Keep at most 4 hyphenated words; prefer action-y first words in the user's intent.
    let mut out = String::new();
    let mut word = String::new();
    let mut words = 0usize;

    for ch in intent.chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            word.push(ch);
            continue;
        }

        if !word.is_empty() {
            if words > 0 {
                out.push('-');
            }
            out.push_str(&word);
            word.clear();
            words += 1;
            if words >= 4 {
                break;
            }
        }
    }

    if words < 4 && !word.is_empty() {
        if words > 0 {
            out.push('-');
        }
        out.push_str(&word);
    }

    if out.is_empty() {
        // Safe, deterministic fallback.
        "scratch-pad".to_string()
    } else {
        out
    }
}

fn default_branch_from_target(target: &str, work_type: &str, scratch_name: &str) -> String {
    let target_slug = if let Some(rest) = target.strip_prefix("impl/") {
        rest.split('/').next().unwrap_or(rest)
    } else if let Some(rest) = target.strip_prefix("spec/") {
        rest.split('/').next().unwrap_or(rest)
    } else if let Some(rest) = target.strip_prefix(".specman/scratchpad/") {
        rest.split('/').next().unwrap_or(rest)
    } else {
        target
            .split('/')
            .next_back()
            .and_then(|segment| segment.split('.').next())
            .unwrap_or(target)
    };
    format!("{target_slug}/{work_type}/{scratch_name}")
}

fn scratch_work_type_key(work_type: &specman::front_matter::ScratchWorkType) -> &'static str {
    use specman::front_matter::ScratchWorkType;
    match work_type {
        ScratchWorkType::Draft(_) => "draft",
        ScratchWorkType::Revision(_) => "revision",
        ScratchWorkType::Feat(_) => "feat",
        ScratchWorkType::Refactor(_) => "ref",
        ScratchWorkType::Fix(_) => "fix",
    }
}

fn rewrite_scratch_front_matter(
    content: &str,
    target: &str,
    branch: &str,
    work_type: &specman::front_matter::ScratchWorkType,
) -> Result<String, McpError> {
    let split = specman::front_matter::split_front_matter(content).map_err(to_mcp_error)?;
    let mut doc: Mapping = serde_yaml::from_str(split.yaml)
        .map_err(|err| invalid_params(format!("invalid scratch front matter: {err}")))?;

    doc.insert(YamlValue::from("target"), YamlValue::from(target));
    doc.insert(YamlValue::from("branch"), YamlValue::from(branch));

    // Build `work_type` YAML from the strongly typed model.
    let work_type_yaml: YamlValue = serde_yaml::to_value(work_type)
        .map_err(|err| invalid_params(format!("failed to encode work_type yaml: {err}")))?;
    doc.insert(YamlValue::from("work_type"), work_type_yaml);

    let yaml = serde_yaml::to_string(&doc)
        .map_err(|err| invalid_params(format!("failed to write front matter: {err}")))?;
    Ok(format!("---\n{}---\n{}", yaml, split.body))
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

impl SpecmanMcpServer {
    fn build_specman(&self) -> Result<SpecmanInstance, McpError> {
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
}
