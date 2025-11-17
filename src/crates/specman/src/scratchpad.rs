use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::template::{TemplateDescriptor, TokenMap};

/// Profiles describe scratch pad templates and optional configuration.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct ScratchPadProfile {
    pub name: String,
    pub template: TemplateDescriptor,
    #[serde(default)]
    pub configuration: BTreeMap<String, serde_json::Value>,
}

impl ScratchPadProfile {
    pub fn token_map(&self) -> TokenMap {
        self.configuration
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}
