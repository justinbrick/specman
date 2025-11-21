use serde::{Deserialize, Serialize};

use crate::error::SpecmanError;

/// Parsed view of the raw YAML front matter attached to an artifact.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RawFrontMatter {
    pub name: Option<String>,
    pub version: Option<String>,
    pub spec: Option<String>,
    pub target: Option<String>,
    pub work_type: Option<serde_yaml::Value>,
    #[serde(default)]
    pub dependencies: Vec<DependencyEntry>,
    #[serde(default)]
    pub references: Vec<ReferenceEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DependencyEntry {
    Simple(String),
    Detailed(DependencyObject),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DependencyObject {
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default, rename = "optional")]
    pub optional: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReferenceEntry {
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(rename = "type")]
    pub reference_type: Option<String>,
    #[serde(default, rename = "optional")]
    pub optional: Option<bool>,
}

/// Borrowed slices of a document split into front matter YAML and body.
pub struct FrontMatterSplit<'a> {
    pub yaml: &'a str,
    pub body: &'a str,
}

/// Attempts to split raw markdown content into YAML front matter and body.
pub fn split_front_matter(content: &str) -> Result<FrontMatterSplit<'_>, SpecmanError> {
    let stripped = content.trim_start_matches('\u{feff}');
    let Some(rest) = stripped.strip_prefix("---") else {
        return Err(SpecmanError::Template(
            "missing front matter delimiter (---)".into(),
        ));
    };

    let rest = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
        .ok_or_else(|| SpecmanError::Template("missing newline after front matter start".into()))?;

    if let Some(idx) = rest.find("\n---") {
        let yaml = rest[..idx].trim_end();
        let after = &rest[idx + 4..]; // skip `\n---`
        let body = after
            .strip_prefix('\n')
            .or_else(|| after.strip_prefix("\r\n"))
            .unwrap_or(after);
        Ok(FrontMatterSplit { yaml, body })
    } else {
        Err(SpecmanError::Template(
            "missing closing front matter delimiter (---)".into(),
        ))
    }
}

/// Matches the previous optional parsing behavior used by dependency traversal.
pub fn optional_front_matter(content: &str) -> (Option<&str>, Option<String>) {
    match split_front_matter(content) {
        Ok(split) => (Some(split.yaml), None),
        Err(_) => (None, Some("missing".into())),
    }
}
