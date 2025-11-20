use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::SpecmanError;

pub type TokenMap = BTreeMap<String, serde_json::Value>;

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
    pub locator: TemplateLocator,
    pub scenario: TemplateScenario,
    pub required_tokens: Vec<String>,
}

/// Result of rendering a template with tokens.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct RenderedTemplate {
    pub body: String,
    pub metadata: TemplateDescriptor,
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
pub struct MarkdownTemplateEngine;

impl TemplateEngine for MarkdownTemplateEngine {
    fn render(
        &self,
        descriptor: &TemplateDescriptor,
        tokens: &TokenMap,
    ) -> Result<RenderedTemplate, SpecmanError> {
        match &descriptor.locator {
            TemplateLocator::FilePath(path) => {
                let raw = fs::read_to_string(path)?;
                let body = apply_tokens(&raw, descriptor, tokens)?;
                // Leverage the `markdown` crate to parse as a validation step.
                let _ = markdown::to_html(&body);
                Ok(RenderedTemplate {
                    body,
                    metadata: descriptor.clone(),
                })
            }
            TemplateLocator::Url(url) => Err(SpecmanError::Template(format!(
                "remote templates are not yet supported: {url}"
            ))),
        }
    }
}

fn apply_tokens(
    content: &str,
    descriptor: &TemplateDescriptor,
    tokens: &TokenMap,
) -> Result<String, SpecmanError> {
    for key in &descriptor.required_tokens {
        if !tokens.contains_key(key) {
            return Err(SpecmanError::Template(format!("missing token: {key}")));
        }
    }

    let mut rendered = content.to_owned();
    for (key, value) in tokens {
        let needle = format!("{{{{{key}}}}}");
        if rendered.contains(&needle) {
            rendered = rendered.replace(&needle, &value_to_string(value));
        }
    }

    if rendered.contains("{{") {
        return Err(SpecmanError::Template(
            "unresolved template tokens remain in output".into(),
        ));
    }

    Ok(rendered)
}

fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}
