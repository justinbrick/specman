use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

use crate::metadata::frontmatter::{DependencyEntry, ReferenceEntry, ScratchWorkType};

/// Shared identity fields for updates.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct IdentityUpdate {
    pub name: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Update fields for Specification artifacts.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct SpecificationUpdate {
    #[serde(flatten)]
    pub identity: IdentityUpdate,
    pub requires_implementation: Option<bool>,
    pub dependencies: Option<Vec<DependencyEntry>>,
}

/// Update fields for Implementation artifacts.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct ImplementationUpdate {
    #[serde(flatten)]
    pub identity: IdentityUpdate,
    pub spec: Option<String>,
    pub location: Option<String>,
    pub references: Option<Vec<ReferenceEntry>>,
    pub dependencies: Option<Vec<DependencyEntry>>,
}

/// Update fields for Scratch Pad artifacts.
/// Note: `target` is immutable and thus not present here.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct ScratchUpdate {
    #[serde(flatten)]
    pub identity: IdentityUpdate,
    pub branch: Option<String>,
    pub work_type: Option<ScratchWorkType>,
    pub dependencies: Option<Vec<DependencyEntry>>,
}

/// Discriminated enum for type-safe front matter updates.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind")]
pub enum FrontMatterUpdate {
    Specification(SpecificationUpdate),
    Implementation(ImplementationUpdate),
    Scratch(ScratchUpdate),
}
