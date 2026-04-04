use serde_json::{Map, Value, json};

pub(crate) fn capability_descriptor_metadata() -> Map<String, Value> {
    let value = json!({
        "specmanCapabilityDescriptor": {
            "entity": "SpecManCapabilityDescriptor",
            "completion": {
                "warningTransport": "notifications/message",
                "ordering": "fuzzy-score-desc-then-lexical-handle",
                "matchingMode": "fuzzy",
                "scratchSuggestions": false,
                "surfaces": [
                    { "surface": "prompt.revision.target", "acceptedKinds": ["spec"] },
                    { "surface": "prompt.migration.target", "acceptedKinds": ["spec"] },
                    { "surface": "prompt.impl.spec", "acceptedKinds": ["spec"] },
                    { "surface": "prompt.feat.target", "acceptedKinds": ["impl"] },
                    { "surface": "prompt.ref.target", "acceptedKinds": ["impl"] },
                    { "surface": "prompt.fix.target", "acceptedKinds": ["impl"] },
                    { "surface": "prompt.compliance.implementation", "acceptedKinds": ["impl"] },
                    { "surface": "resource.spec://{artifact}", "acceptedKinds": ["spec"] },
                    { "surface": "resource.impl://{artifact}", "acceptedKinds": ["impl"] },
                    { "surface": "resource.spec://{artifact}/constraints", "acceptedKinds": ["spec"] },
                    { "surface": "resource.spec://{artifact}/constraints/{constraint_id}", "acceptedKinds": ["spec"] },
                    { "surface": "resource.impl://{artifact}/compliance", "acceptedKinds": ["impl"] }
                ]
            }
        }
    });

    value.as_object().cloned().unwrap_or_default()
}
