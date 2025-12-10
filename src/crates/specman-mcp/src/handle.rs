use crate::error::{Result, SpecmanMcpError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specman::ArtifactKind;
use specman::workspace::WorkspacePaths;
use std::path::PathBuf;

/// Canonical MCP handle covering SpecMan `spec://`, `impl://`, and `scratch://` schemes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpResourceHandle {
    kind: ArtifactKind,
    slug: String,
}

/// Represents either the artifact body or its `/dependencies` virtual handle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResourceTarget {
    /// Direct artifact content.
    Artifact(McpResourceHandle),
    /// Derived handle exposing the dependency tree snapshot.
    Dependencies(McpResourceHandle),
}

impl ResourceTarget {
    /// Parses a handle reference, returning `None` when the URI does not use an MCP scheme.
    pub fn parse(reference: &str) -> Result<Option<Self>> {
        let trimmed = reference.trim();
        if let Some(base) = trimmed.strip_suffix("/dependencies") {
            return McpResourceHandle::parse(base)
                .map(|handle| handle.map(ResourceTarget::Dependencies));
        }

        McpResourceHandle::parse(trimmed).map(|handle| handle.map(ResourceTarget::Artifact))
    }

    /// Returns the underlying artifact handle regardless of variant.
    pub fn base_handle(&self) -> &McpResourceHandle {
        match self {
            ResourceTarget::Artifact(handle) | ResourceTarget::Dependencies(handle) => handle,
        }
    }
}

impl McpResourceHandle {
    /// Parses a raw string reference into a canonical MCP handle.
    pub fn parse(reference: &str) -> Result<Option<Self>> {
        let trimmed = reference.trim();
        if let Some(rest) = trimmed.strip_prefix("spec://") {
            return Self::new(ArtifactKind::Specification, rest).map(Some);
        }
        if let Some(rest) = trimmed.strip_prefix("impl://") {
            return Self::new(ArtifactKind::Implementation, rest).map(Some);
        }
        if let Some(rest) = trimmed.strip_prefix("scratch://") {
            return Self::new(ArtifactKind::ScratchPad, rest).map(Some);
        }

        if trimmed.contains("://")
            && !trimmed.starts_with("http://")
            && !trimmed.starts_with("https://")
        {
            let (scheme, _) = trimmed.split_once("://").unwrap_or((trimmed, ""));
            return Err(SpecmanMcpError::resource(format!(
                "unsupported locator scheme {}:// (expected https://, spec://, impl://, scratch://, or workspace-relative path)",
                scheme
            )));
        }

        Ok(None)
    }

    /// Creates a handle from a known slug that already lives inside the workspace.
    pub fn from_slug(kind: ArtifactKind, slug: &str) -> Result<Self> {
        Self::new(kind, slug)
    }

    fn new(kind: ArtifactKind, slug: &str) -> Result<Self> {
        let canonical = Self::canonical_slug(slug)?;
        Ok(Self {
            kind,
            slug: canonical,
        })
    }

    fn canonical_slug(raw: &str) -> Result<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(SpecmanMcpError::resource(
                "resource handle must include a non-empty identifier",
            ));
        }

        if trimmed.contains('/') || trimmed.contains('\\') {
            return Err(SpecmanMcpError::resource(
                "resource handle identifiers cannot contain path separators",
            ));
        }

        let canonical = trimmed.to_ascii_lowercase();
        if !canonical
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
        {
            return Err(SpecmanMcpError::resource(
                "resource handle identifiers may only contain letters, numbers, '-' or '_'",
            ));
        }

        Ok(canonical)
    }

    /// Returns the canonical URI string.
    pub fn uri(&self) -> String {
        format!("{}://{}", scheme_for_kind(self.kind), self.slug)
    }

    /// Returns the derived `/dependencies` URI string.
    pub fn dependencies_uri(&self) -> String {
        format!("{}/dependencies", self.uri())
    }

    /// Returns the artifact kind.
    pub fn kind(&self) -> ArtifactKind {
        self.kind
    }

    /// Returns the canonical slug.
    pub fn slug(&self) -> &str {
        &self.slug
    }

    /// Resolves the canonical filesystem path for the artifact.
    pub fn to_path(&self, workspace: &WorkspacePaths) -> PathBuf {
        match self.kind {
            ArtifactKind::Specification => workspace.spec_dir().join(&self.slug).join("spec.md"),
            ArtifactKind::Implementation => workspace.impl_dir().join(&self.slug).join("impl.md"),
            ArtifactKind::ScratchPad => workspace
                .scratchpad_dir()
                .join(&self.slug)
                .join("scratch.md"),
        }
    }
}

fn scheme_for_kind(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Specification => "spec",
        ArtifactKind::Implementation => "impl",
        ArtifactKind::ScratchPad => "scratch",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spec_handle() {
        let handle = McpResourceHandle::parse("spec://SpecMan-Core")
            .expect("parse succeeded")
            .expect("handle present");
        assert_eq!(handle.kind(), ArtifactKind::Specification);
        assert_eq!(handle.slug(), "specman-core");
    }

    #[test]
    fn detects_dependencies_suffix() {
        let target = ResourceTarget::parse("impl://engine/dependencies")
            .expect("parse")
            .expect("handle");
        match target {
            ResourceTarget::Dependencies(handle) => {
                assert_eq!(handle.kind(), ArtifactKind::Implementation)
            }
            _ => panic!("expected dependencies variant"),
        }
    }
}
