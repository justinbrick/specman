use std::collections::HashSet;

use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{PromptMessage, PromptMessageRole};
use rmcp::schemars::JsonSchema;
use rmcp::{prompt, prompt_router};
use serde::{Deserialize, Serialize};

use specman::{DependencyTree, SpecmanError, WorkspaceLocator, WorkspacePaths};

use crate::error::{McpError, to_mcp_error};
use crate::resources::{artifact_handle, resolved_path_or_artifact_path};
use crate::server::SpecmanMcpServer;

pub(crate) fn build_prompt_router() -> PromptRouter<SpecmanMcpServer> {
    SpecmanMcpServer::prompt_router()
}

const SCRATCH_FEAT_TEMPLATE: &str = include_str!("../templates/prompts/scratch-feat.md");
const SCRATCH_FIX_TEMPLATE: &str = include_str!("../templates/prompts/scratch-fix.md");
const SCRATCH_REF_TEMPLATE: &str = include_str!("../templates/prompts/scratch-ref.md");
const SCRATCH_REVISION_TEMPLATE: &str = include_str!("../templates/prompts/scratch-revision.md");
const SPEC_TEMPLATE: &str = include_str!("../templates/prompts/spec.md");
const IMPL_TEMPLATE: &str = include_str!("../templates/prompts/impl.md");

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ScratchPromptArgs {
    pub target: String,
    pub branch_name: Option<String>,
}

/// Arguments for rendering a prompt that creates a new specification.
///
/// New specifications do not have stable dependency context until the author defines it,
/// so this prompt intentionally accepts no dependency-prefill arguments.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct SpecPromptArgs {}

/// Arguments for rendering a prompt that creates a new implementation from a governing specification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImplPromptArgs {
    pub spec: String,
}

pub(crate) struct ResolvedTarget {
    pub(crate) tree: DependencyTree,
    pub(crate) workspace: WorkspacePaths,
    pub(crate) handle: String,
    pub(crate) path: String,
}

#[prompt_router]
impl SpecmanMcpServer {
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

    #[prompt(
        name = "spec",
        description = "Generate a SpecMan prompt for creating a new specification using the standard template"
    )]
    pub async fn spec_prompt(
        &self,
        Parameters(args): Parameters<SpecPromptArgs>,
    ) -> Result<Vec<PromptMessage>, McpError> {
        let _ = args;
        self.render_spec_prompt(SPEC_TEMPLATE)
    }

    #[prompt(
        name = "impl",
        description = "Generate a SpecMan prompt for creating a new implementation using the standard template"
    )]
    pub async fn impl_prompt(
        &self,
        Parameters(args): Parameters<ImplPromptArgs>,
    ) -> Result<Vec<PromptMessage>, McpError> {
        self.render_impl_prompt(IMPL_TEMPLATE, &args.spec)
    }
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

        let context = bullet_list(&dependency_lines(&resolved));
        let dependencies = context.clone();

        let replacements = vec![
            ("{{branch_name}}", branch_instruction.clone()),
            ("{{target_path}}", resolved.handle.clone()),
            ("{{context}}", context),
            ("{{dependencies}}", dependencies),
        ];

        let rendered = apply_tokens(template, &replacements)?;
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            rendered,
        )])
    }

    /// Render the specification-creation prompt. Since a new specification has no canonical locator yet,
    /// callers may optionally provide an existing locator (`seed_target`) to prefill dependency context.
    fn render_spec_prompt(&self, template: &str) -> Result<Vec<PromptMessage>, McpError> {
        let rendered = apply_tokens(template, &[])?;
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            rendered,
        )])
    }

    /// Render the implementation-creation prompt from a governing specification locator.
    fn render_impl_prompt(
        &self,
        template: &str,
        spec_locator: &str,
    ) -> Result<Vec<PromptMessage>, McpError> {
        let resolved = self.resolve_target(spec_locator)?;

        let context = bullet_list(&dependency_lines(&resolved));
        let dependencies = context.clone();

        let replacements = vec![
            ("{{target_path}}", resolved.handle.clone()),
            ("{{target_spec_path}}", resolved.handle.clone()),
            ("{{context}}", context),
            ("{{dependencies}}", dependencies),
        ];

        let rendered = apply_tokens(template, &replacements)?;
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            rendered,
        )])
    }
}

pub(crate) fn apply_tokens(
    template: &str,
    replacements: &[(&str, String)],
) -> Result<String, McpError> {
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

pub(crate) fn dependency_lines(resolved: &ResolvedTarget) -> Vec<String> {
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
