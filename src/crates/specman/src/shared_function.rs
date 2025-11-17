use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Semantic version tracking used across SpecMan entities.
#[derive(
    Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl SemVer {
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

/// Identifiers for the SpecMan entity families.
#[derive(
    Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub enum EntityKind {
    Specification,
    Implementation,
    ScratchPad,
    Template,
    Other(String),
}

impl Default for EntityKind {
    fn default() -> Self {
        Self::Other("unspecified".into())
    }
}

/// Schema metadata attached to inputs and outputs.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct SchemaRef {
    pub name: String,
    pub version: Option<SemVer>,
    #[serde(default)]
    pub schema: serde_json::Value,
}
