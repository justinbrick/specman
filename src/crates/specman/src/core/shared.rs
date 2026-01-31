use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use semver::Version as SemVer;

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
