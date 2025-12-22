use std::path::PathBuf;

use rmcp::RoleServer;
use rmcp::elicit_safe;
use rmcp::model::{
    Content, ContextInclusion, CreateMessageRequestParam, Role as SamplingRole, SamplingMessage,
};
use rmcp::service::Peer;
use specman::{FrontMatterUpdateOp, FrontMatterUpdateRequest, apply_front_matter_update};

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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UpdateMode {
    #[schemars(description = "Persist the updated artifact to disk.")]
    Persist,
    #[schemars(description = "Preview-only: return the updated document without writing to disk.")]
    Preview,
}

/// Callers must declare which artifact kind they expect.
///
/// This is intentionally a simple string enum ("spec" | "impl" | "scratch")
/// rather than a tagged object, to keep tool schemas compatible with stricter
/// validators.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExpectedArtifactKind {
    #[schemars(description = "Expect a specification artifact.")]
    Spec,
    #[schemars(description = "Expect an implementation artifact.")]
    Impl,
    #[schemars(description = "Expect a scratch pad artifact.")]
    Scratch,
}

impl ExpectedArtifactKind {
    fn as_artifact_kind(&self) -> ArtifactKind {
        match self {
            ExpectedArtifactKind::Spec => ArtifactKind::Specification,
            ExpectedArtifactKind::Impl => ArtifactKind::Implementation,
            ExpectedArtifactKind::Scratch => ArtifactKind::ScratchPad,
        }
    }
}

/// `update_artifact` input schema for MCP.
///
/// NOTE: We handcraft this JSON Schema instead of relying on schemars' derived
/// output.
///
/// Some MCP clients (notably OpenAI tool schema validators) are significantly
/// stricter than general JSON Schema, and may reject otherwise-valid schemas
/// that contain `$defs/$ref`, `oneOf/anyOf`, or OpenAPI-style `nullable`.
///
/// The server still deserializes/validates using the concrete Rust types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateArtifactArgs {
    pub locator: String,

    pub expected_kind: ExpectedArtifactKind,

    pub mode: UpdateMode,

    pub ops: Vec<FrontMatterUpdateOp>,
}

impl JsonSchema for UpdateArtifactArgs {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("UpdateArtifactArgs")
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        // Keep this schema deliberately simple (draft-07-ish) and inline.
        // Validation of detailed op shapes happens server-side.
        schemars::json_schema!({
            "type": "object",
            "description": "`update_artifact` input schema for MCP.",
            "additionalProperties": false,
            "properties": {
                "locator": {
                    "type": "string",
                    "description": "Artifact locator: workspace-relative path, spec://.../impl://.../scratch://... handle, or an HTTPS URL."
                },
                "expectedKind": {
                    "type": "string",
                    "description": "Callers must declare which artifact kind they expect.",
                    "enum": ["spec", "impl", "scratch"]
                },
                "mode": {
                    "type": "string",
                    "description": "Persistence mode: 'persist' writes to disk; 'preview' returns updated content without writing.",
                    "enum": ["persist", "preview"]
                },
                "ops": {
                    "type": "array",
                    "description": "Ops-based front matter mutation list. MUST contain at least one op.",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "description": "Front matter update operation. The server validates required fields for each op.",
                        "additionalProperties": true,
                        "properties": {
                            "op": {
                                "type": "string",
                                "description": "Operation discriminator.",
                                "enum": [
                                    "set_name",
                                    "clear_name",
                                    "set_title",
                                    "clear_title",
                                    "set_description",
                                    "clear_description",
                                    "set_version",
                                    "clear_version",
                                    "add_tag",
                                    "remove_tag",
                                    "add_dependency",
                                    "remove_dependency",
                                    "set_requires_implementation",
                                    "clear_requires_implementation",
                                    "set_spec",
                                    "clear_spec",
                                    "set_location",
                                    "clear_location",
                                    "set_library",
                                    "clear_library",
                                    "add_reference",
                                    "remove_reference",
                                    "set_primary_language",
                                    "clear_primary_language",
                                    "set_secondary_languages",
                                    "clear_secondary_languages",
                                    "set_target",
                                    "clear_target",
                                    "set_branch",
                                    "clear_branch",
                                    "set_work_type",
                                    "clear_work_type"
                                ]
                            },
                            "name": { "type": "string" },
                            "title": { "type": "string" },
                            "description": { "type": "string" },
                            "version": { "type": "string" },
                            "tag": { "type": "string" },
                            "ref": { "type": "string" },
                            "optional": { "type": "boolean" },
                            "requires": { "type": "boolean" },
                            "location": { "type": "string" },
                            "library": {
                                "description": "Library reference (string or object forms may be accepted by the server).",
                                "type": "string"
                            },
                            "type": {
                                "description": "Optional reference type hint.",
                                "type": "string"
                            },
                            "language": {
                                "description": "Primary language identifier.",
                                "type": "string"
                            },
                            "languages": {
                                "description": "Secondary implementing languages.",
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "additionalProperties": true,
                                    "properties": {
                                        "language": { "type": "string" },
                                        "libraries": { "type": "array", "items": { "type": "string" } },
                                        "properties": { "type": "object", "additionalProperties": true }
                                    },
                                    "required": ["language"]
                                }
                            },
                            "target": { "type": "string" },
                            "branch": { "type": "string" },
                            "work_type": {
                                "description": "Scratch work type object (server validates exact shape).",
                                "type": "object",
                                "additionalProperties": true
                            }
                        },
                        "required": ["op"]
                    }
                }
            },
            "required": ["locator", "expectedKind", "mode", "ops"]
        })
    }
}

/// Result payload returned by the `update_artifact` MCP tool.
///
/// NOTE: Handcrafted schema for compatibility with stricter MCP clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateArtifactResult {
    pub id: ArtifactId,
    pub handle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub updated_document: String,
    pub persisted: bool,
}

impl JsonSchema for UpdateArtifactResult {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("UpdateArtifactResult")
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
            "description": "Result payload returned by the `update_artifact` MCP tool.",
            "additionalProperties": false,
            "properties": {
                "id": {
                    "type": "object",
                    "description": "Stable artifact identifier (kind + name). For HTTPS locators, name is derived from the URL.",
                    "additionalProperties": false,
                    "properties": {
                        "kind": {
                            "type": "string",
                            "description": "Artifact kind segmentation.",
                            "enum": ["Specification", "Implementation", "ScratchPad"]
                        },
                        "name": { "type": "string" }
                    },
                    "required": ["kind", "name"]
                },
                "handle": {
                    "type": "string",
                    "description": "Canonical handle (spec://..., impl://..., scratch://...) for workspace artifacts; for HTTPS locators this is the URL."
                },
                "path": {
                    "type": "string",
                    "description": "Canonical workspace-relative path for workspace artifacts (present for both preview and persist)."
                },
                "updatedDocument": {
                    "type": "string",
                    "description": "Full updated document content. Only YAML front matter may differ from the original."
                },
                "persisted": {
                    "type": "boolean",
                    "description": "Whether the updated document was persisted to disk."
                }
            },
            "required": ["id", "handle", "updatedDocument", "persisted"]
        })
    }
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

    // #[tool(
    //     name = "update_artifact",
    //     description = "Update YAML front matter metadata for a SpecMan artifact (spec/impl/scratch) while preserving the Markdown body. Supports preview and persist modes."
    // )]
    pub(crate) async fn update_artifact(
        &self,
        Parameters(args): Parameters<UpdateArtifactArgs>,
    ) -> Result<Json<UpdateArtifactResult>, McpError> {
        self.update_artifact_internal(args).await
    }
}

impl SpecmanMcpServer {
    async fn update_artifact_internal(
        &self,
        args: UpdateArtifactArgs,
    ) -> Result<Json<UpdateArtifactResult>, McpError> {
        let locator = args.locator.trim();
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

        if args.ops.is_empty() {
            return Err(invalid_params("update requires at least one op"));
        }

        if locator.starts_with("http://") {
            return Err(invalid_params(
                "unsupported url scheme in locator; use https",
            ));
        }

        if locator.starts_with("https://") {
            if matches!(args.mode, UpdateMode::Persist) {
                return Err(invalid_params(
                    "persist is not supported for HTTPS locators; use mode=preview",
                ));
            }

            let raw = fetch_https_document(locator).await?;

            let expected_kind = args.expected_kind.as_artifact_kind();
            let id = ArtifactId {
                kind: expected_kind,
                name: derive_name_from_https(locator),
            };

            let workspace = self.workspace.workspace().map_err(to_mcp_error)?;
            let fake_path = workspace.root().join("remote.md");
            let request = FrontMatterUpdateRequest {
                persist: false,
                ops: args.ops,
            };

            let (updated_document, _mutated) =
                apply_front_matter_update(&id, &fake_path, &workspace, &raw, &request)
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

        let expected = args.expected_kind.as_artifact_kind();
        if tree.root.id.kind != expected {
            return Err(invalid_params(format!(
                "artifact kind mismatch: expected {:?} but locator resolved to {:?}",
                expected, tree.root.id.kind
            )));
        }

        let resolved = resolved_path_or_artifact_path(&tree.root, &workspace);
        let absolute = PathBuf::from(resolved);
        let relative = workspace_relative_path(workspace.root(), &absolute)
            .ok_or_else(|| invalid_params("locator must resolve within the workspace"))?;

        let specman = self.build_specman()?;
        let request = FrontMatterUpdateRequest {
            persist: matches!(args.mode, UpdateMode::Persist),
            ops: args.ops,
        };

        let result = specman
            .update(tree.root.id.clone(), request)
            .map_err(to_mcp_error)?;

        let handle = artifact_handle(&tree.root);
        Ok(Json(UpdateArtifactResult {
            id: result.artifact,
            handle,
            path: Some(relative),
            updated_document: result.updated_document,
            persisted: result.persisted.is_some(),
        }))
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

    // Safety / determinism guard: refuse unbounded downloads.
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
            CreateRequest::ScratchPad { context, .. } => {
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
                    front_matter: None,
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
                    front_matter: None,
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
                    front_matter: None,
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
                front_matter: None,
            })
            .map_err(to_mcp_error)?;

        let branch = default_branch_from_target(
            &context.target,
            scratch_work_type_key(&context.work_type),
            &context.name,
        );

        // Use the declarative metadata mutation API to set branch/work_type,
        // while preserving the immutable scratchpad target.
        let update = FrontMatterUpdateRequest::new()
            .persist(true)
            .with_op(FrontMatterUpdateOp::SetBranch { branch })
            .with_op(FrontMatterUpdateOp::SetWorkType {
                work_type: context.work_type.clone(),
            });
        specman
            .update(persisted.artifact.clone(), update)
            .map_err(to_mcp_error)?;

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
            CreateRequest::Implementation { context, .. } => {
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
                    front_matter: None,
                })
            }
            CreateRequest::ScratchPad { context, .. } => {
                let target = self.normalize_locator_to_workspace_path(&context.target)?;
                Ok(CreateRequest::ScratchPad {
                    context: specman::ScratchPadCreateContext {
                        name: context.name.trim().to_string(),
                        target,
                        work_type: context.work_type,
                    },
                    front_matter: None,
                })
            }
            CreateRequest::Specification { context, .. } => Ok(CreateRequest::Specification {
                context: specman::SpecContext {
                    name: context.name.trim().to_string(),
                    title: context.title.trim().to_string(),
                },
                front_matter: None,
            }),
        }
    }
}
