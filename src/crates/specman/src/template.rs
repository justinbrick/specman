use handlebars::Handlebars;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::error::SpecmanError;

pub type TokenMap = BTreeMap<String, serde_json::Value>;

/// Identifies the tier that produced a template.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
pub enum TemplateTier {
    WorkspaceOverride,
    PointerFile,
    PointerUrl,
    #[default]
    EmbeddedDefault,
}

/// Records provenance metadata for persisted templates.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default, PartialEq, Eq)]
pub struct TemplateProvenance {
    pub tier: TemplateTier,
    pub locator: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pointer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
}

/// Supported template scenarios described by the specification.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum TemplateScenario {
    Specification,
    Implementation,
    ScratchPad,
    WorkType(String),
}

impl Default for TemplateScenario {
    fn default() -> Self {
        Self::Specification
    }
}

/// Template locators include both filesystem paths and remote URLs.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum TemplateLocator {
    FilePath(PathBuf),
    Url(String),
}

impl Default for TemplateLocator {
    fn default() -> Self {
        Self::FilePath(PathBuf::new())
    }
}

/// Rich descriptor for a template plus its required tokens.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct TemplateDescriptor {
    // [ENSURES: entity-templatedescriptor.requirements:CHECK]
    pub locator: TemplateLocator,
    pub scenario: TemplateScenario,
    pub required_tokens: Vec<String>,
}

/// Result of rendering a template with tokens.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct RenderedTemplate {
    pub body: String,
    pub metadata: TemplateDescriptor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<TemplateProvenance>,
}

/// Trait for engines that render templates into Markdown text.
pub trait TemplateEngine: Send + Sync {
    fn render(
        &self,
        descriptor: &TemplateDescriptor,
        tokens: &TokenMap,
    ) -> Result<RenderedTemplate, SpecmanError>;
}

/// Minimal Markdown template engine that performs token substitution.
#[derive(Default)]
pub struct MarkdownTemplateEngine {
    registry: Handlebars<'static>,
}

impl MarkdownTemplateEngine {
    pub fn new() -> Self {
        let mut registry = Handlebars::new();
        registry.register_escape_fn(handlebars::no_escape);
        Self { registry }
    }
}

impl TemplateEngine for MarkdownTemplateEngine {
    fn render(
        &self,
        descriptor: &TemplateDescriptor,
        tokens: &TokenMap,
    ) -> Result<RenderedTemplate, SpecmanError> {
        match &descriptor.locator {
            TemplateLocator::FilePath(path) => {
                let raw = fs::read_to_string(path)?;
                let body = self
                    .registry
                    .render_template(&raw, tokens)
                    .map_err(|e| SpecmanError::Template(e.to_string()))?;

                // Leverage the `markdown` crate to parse as a validation step.
                let _ = markdown::to_html(&body);
                Ok(RenderedTemplate {
                    body,
                    metadata: descriptor.clone(),
                    provenance: None,
                })
            }
            TemplateLocator::Url(url) => Err(SpecmanError::Template(format!(
                "remote templates are not yet supported: {url}"
            ))),
        }
    }
}

impl<T> TemplateEngine for Arc<T>
where
    T: TemplateEngine + ?Sized,
{
    fn render(
        &self,
        descriptor: &TemplateDescriptor,
        tokens: &TokenMap,
    ) -> Result<RenderedTemplate, SpecmanError> {
        (**self).render(descriptor, tokens)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SpecContext {
    pub name: String,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ImplContext {
    pub name: String,
    pub target: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScratchPadContext {
    pub name: String,
    pub target: String,
    pub work_type: String,
}
