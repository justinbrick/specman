use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::dependency_tree::ArtifactKind;
use crate::front_matter::ArtifactFrontMatter;

pub const WORKSPACE_INDEX_SCHEMA_VERSION: u32 = 1;

/// Stable identifier for an indexed artifact.
///
/// For this parsing-only iteration, the key is workspace-relative path + kind.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactKey {
    pub kind: ArtifactKind,
    /// Workspace-relative path using forward slashes.
    pub workspace_path: String,
}

#[derive(Clone, Debug)]
pub struct ArtifactRecord {
    pub key: ArtifactKey,
    /// Canonical absolute path.
    pub absolute_path: PathBuf,
    pub front_matter: Option<ArtifactFrontMatter>,
}

/// Artifact-qualified heading identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeadingIdentifier {
    pub artifact: ArtifactKey,
    pub slug: String,
}

#[derive(Clone, Debug)]
pub struct HeadingRecord {
    pub id: HeadingIdentifier,
    pub level: u8,
    /// Original inline markdown from the heading line (after the `#` prefix).
    pub title: String,
    /// Document order (0-based) within the artifact.
    pub order: usize,
    pub parent: Option<HeadingIdentifier>,
    pub children: Vec<HeadingIdentifier>,
    /// Markdown content directly under this heading, excluding child heading blocks.
    pub content: String,
    /// Resolved heading references in inline-link order (dedup not applied).
    pub referenced_headings: Vec<HeadingIdentifier>,
}

/// Artifact-qualified constraint group identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstraintIdentifier {
    pub artifact: ArtifactKey,
    /// Group set without leading `!` or trailing `:`.
    pub group: String,
}

#[derive(Clone, Debug)]
pub struct ConstraintRecord {
    pub id: ConstraintIdentifier,
    /// Heading this constraint group is associated with.
    pub heading: HeadingIdentifier,
    /// Line number within the artifact body (1-based) where the identifier appears.
    pub line: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelationshipKind {
    HeadingToArtifact,
    HeadingToHeading,
    HeadingToFile,
    ConstraintToHeading,
    ParentToChild,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelationshipEdge {
    pub kind: RelationshipKind,
    pub from: String,
    pub to: String,
}

/// Parsing-only, in-memory structure index for a workspace.
#[derive(Clone, Debug, Default)]
pub struct WorkspaceIndex {
    pub schema_version: u32,
    pub workspace_root: PathBuf,

    pub artifacts: BTreeMap<ArtifactKey, ArtifactRecord>,
    pub headings: BTreeMap<HeadingIdentifier, HeadingRecord>,
    pub constraints: BTreeMap<ConstraintIdentifier, ConstraintRecord>,
    pub relationships: Vec<RelationshipEdge>,
}
