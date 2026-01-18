use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::template::{TemplateDescriptor, TemplateProvenance, TokenMap};

/// Standard scratch pad profiles aligned with SpecMan work types.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum ScratchPadProfileKind {
    Ref,
    Feat,
    Fix,
    Revision,
}

impl ScratchPadProfileKind {
    /// Returns the canonical slug used for workspace directories and provenance.
    pub fn slug(&self) -> &'static str {
        // [ENSURES: concept-scratch-pads.work-type:CHECK]
        match self {
            Self::Ref => "ref",
            Self::Feat => "feat",
            Self::Fix => "fix",
            Self::Revision => "revision",
        }
    }
}

impl Default for ScratchPadProfileKind {
    fn default() -> Self {
        Self::Ref
    }
}

/// Profiles describe scratch pad templates and optional configuration.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct ScratchPadProfile {
    // [ENSURES: entity-scratchpadprofile.requirements:CHECK]
    pub kind: ScratchPadProfileKind,
    /// Caller-provided scratch pad slug; falls back to the canonical kind slug when empty.
    #[serde(default)]
    pub name: String,
    pub template: TemplateDescriptor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<TemplateProvenance>,
    #[serde(default)]
    pub configuration: BTreeMap<String, serde_json::Value>,
}

impl ScratchPadProfile {
    /// Returns the canonical slug for this profile.
    pub fn slug(&self) -> &str {
        // [ENSURES: concept-scratch-pads.naming:CHECK]
        if self.name.is_empty() {
            self.kind.slug()
        } else {
            &self.name
        }
    }

    pub fn token_map(&self) -> TokenMap {
        self.configuration
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}
