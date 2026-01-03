use std::borrow::Cow;
use std::collections::BTreeMap;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::de::{self, DeserializeOwned, Deserializer};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_yaml::{Mapping, Value as YamlValue};

use crate::error::SpecmanError;

/// Shared identity fields repeated across specification, implementation, and scratch metadata
/// per the SpecMan Data Model requirements.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct ArtifactIdentityFields {
    pub name: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub version: Option<String>,
}

/// Specification YAML fields defined in the Specification Metadata section of the
/// SpecMan Data Model (see `spec/specman-data-model/spec.md`).
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct SpecificationFrontMatter {
    #[serde(flatten)]
    pub identity: ArtifactIdentityFields,
    #[serde(default)]
    pub dependencies: Vec<DependencyEntry>,
    #[serde(default)]
    pub requires_implementation: Option<bool>,
}

/// Implementation YAML fields defined in the Implementation Metadata section of the
/// SpecMan Data Model (see `spec/specman-data-model/spec.md`).
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct ImplementationFrontMatter {
    #[serde(flatten)]
    pub identity: ArtifactIdentityFields,
    pub spec: Option<String>,
    pub location: Option<String>,
    #[serde(default)]
    pub library: Option<LibraryReference>,
    #[serde(default)]
    pub primary_language: Option<ImplementingLanguage>,
    #[serde(default)]
    pub secondary_languages: Vec<ImplementingLanguage>,
    #[serde(default)]
    pub references: Vec<ReferenceEntry>,
    #[serde(default)]
    pub dependencies: Vec<DependencyEntry>,
}

/// Scratch pad YAML fields defined in the Scratch Pad Metadata section of the
/// SpecMan Data Model (see `spec/specman-data-model/spec.md`).
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct ScratchFrontMatter {
    #[serde(flatten)]
    pub identity: ArtifactIdentityFields,
    pub target: Option<String>,
    pub branch: Option<String>,
    #[serde(default)]
    pub work_type: Option<ScratchWorkType>,
    #[serde(default)]
    pub dependencies: Vec<DependencyEntry>,
}

/// Unified view of artifact-specific front matter.
#[derive(Debug, Clone)]
pub enum ArtifactFrontMatter {
    Specification(SpecificationFrontMatter),
    Implementation(ImplementationFrontMatter),
    Scratch(ScratchFrontMatter),
}

/// High-level classification used to map front matter variants to `ArtifactKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontMatterKind {
    Specification,
    Implementation,
    ScratchPad,
}

impl ArtifactFrontMatter {
    /// Parses YAML into a typed front matter enum using the discriminators recorded in the
    /// SpecMan Data Model.
    pub fn from_yaml_str(yaml: &str) -> Result<Self, SpecmanError> {
        let value: YamlValue = serde_yaml::from_str(yaml)
            .map_err(|err| SpecmanError::Serialization(err.to_string()))?;
        Self::from_value(value)
    }

    /// Builds a typed front matter enum from a previously parsed YAML value.
    pub fn from_value(value: YamlValue) -> Result<Self, SpecmanError> {
        let mapping = value
            .as_mapping()
            .ok_or_else(|| SpecmanError::Template("front matter must be a YAML mapping".into()))?;
        let kind = detect_kind(mapping);
        match kind {
            FrontMatterKind::Specification => parse_variant::<SpecificationFrontMatter>(&value)
                .map(ArtifactFrontMatter::Specification),
            FrontMatterKind::Implementation => parse_variant::<ImplementationFrontMatter>(&value)
                .map(ArtifactFrontMatter::Implementation),
            FrontMatterKind::ScratchPad => {
                parse_variant::<ScratchFrontMatter>(&value).map(ArtifactFrontMatter::Scratch)
            }
        }
    }

    /// Provides typed access to the specification variant.
    pub fn as_specification(&self) -> Option<&SpecificationFrontMatter> {
        match self {
            ArtifactFrontMatter::Specification(value) => Some(value),
            _ => None,
        }
    }

    /// Provides typed access to the implementation variant.
    pub fn as_implementation(&self) -> Option<&ImplementationFrontMatter> {
        match self {
            ArtifactFrontMatter::Implementation(value) => Some(value),
            _ => None,
        }
    }

    /// Provides typed access to the scratch variant.
    pub fn as_scratch(&self) -> Option<&ScratchFrontMatter> {
        match self {
            ArtifactFrontMatter::Scratch(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the high-level classification for this front matter record.
    pub fn kind(&self) -> FrontMatterKind {
        match self {
            ArtifactFrontMatter::Specification(_) => FrontMatterKind::Specification,
            ArtifactFrontMatter::Implementation(_) => FrontMatterKind::Implementation,
            ArtifactFrontMatter::Scratch(_) => FrontMatterKind::ScratchPad,
        }
    }

    /// Returns the explicitly declared artifact name, when present.
    pub fn name(&self) -> Option<&str> {
        self.identity().name.as_deref()
    }

    /// Returns the explicitly declared version string, when present.
    pub fn version(&self) -> Option<&str> {
        self.identity().version.as_deref()
    }

    fn identity(&self) -> &ArtifactIdentityFields {
        match self {
            ArtifactFrontMatter::Specification(front) => &front.identity,
            ArtifactFrontMatter::Implementation(front) => &front.identity,
            ArtifactFrontMatter::Scratch(front) => &front.identity,
        }
    }

    /// Parses from a borrowed YAML value, cloning only when necessary.
    pub fn from_yaml_value(value: &YamlValue) -> Result<Self, SpecmanError> {
        Self::from_value(value.clone())
    }
}

/// Represents dependency entries defined in the Specification Metadata section.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum DependencyEntry {
    Simple(String),
    Detailed(DependencyObject),
}

/// Structured dependency with `ref` + optional flag per SpecMan Data Model.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct DependencyObject {
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default, rename = "optional")]
    pub optional: Option<bool>,
}

/// Implementation reference entry defined alongside the Implementation Metadata rules.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ReferenceEntry {
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(rename = "type")]
    pub reference_type: Option<String>,
    #[serde(default, rename = "optional")]
    pub optional: Option<bool>,
}

/// Represents either a plain string or object-form implementation library reference.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum LibraryReference {
    Simple(String),
    Detailed(LibraryObject),
}

/// Additional metadata associated with a named library dependency.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct LibraryObject {
    pub name: String,
    #[serde(flatten)]
    pub extras: BTreeMap<String, JsonValue>,
}

/// Implements the SpecMan Data Model definition for an implementing language entry.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct ImplementingLanguage {
    pub language: String,
    #[serde(default)]
    pub properties: BTreeMap<String, JsonValue>,
    #[serde(default)]
    pub libraries: Vec<LibraryReference>,
}

/// Discriminated enum capturing scratch pad work types (draft, revision, feat, ref, fix).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScratchWorkType {
    Draft(ScratchWorkloadExtras),
    Revision(ScratchRevisionMetadata),
    Feat(ScratchWorkloadExtras),
    Refactor(ScratchRefactorMetadata),
    Fix(ScratchFixMetadata),
}

/// Identifies the concrete scratch work type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScratchWorkTypeKind {
    Draft,
    Revision,
    Feat,
    Refactor,
    Fix,
}

impl ScratchWorkType {
    /// Returns the normalized work type label (draft, revision, feat, ref, or fix).
    pub fn kind(&self) -> ScratchWorkTypeKind {
        match self {
            ScratchWorkType::Draft(_) => ScratchWorkTypeKind::Draft,
            ScratchWorkType::Revision(_) => ScratchWorkTypeKind::Revision,
            ScratchWorkType::Feat(_) => ScratchWorkTypeKind::Feat,
            ScratchWorkType::Refactor(_) => ScratchWorkTypeKind::Refactor,
            ScratchWorkType::Fix(_) => ScratchWorkTypeKind::Fix,
        }
    }
}

impl ScratchWorkTypeKind {
    /// Returns the canonical string label for this work type.
    pub fn as_str(&self) -> &'static str {
        match self {
            ScratchWorkTypeKind::Draft => "draft",
            ScratchWorkTypeKind::Revision => "revision",
            ScratchWorkTypeKind::Feat => "feat",
            ScratchWorkTypeKind::Refactor => "ref",
            ScratchWorkTypeKind::Fix => "fix",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ScratchWorkloadExtras {
    #[serde(flatten)]
    pub extras: BTreeMap<String, JsonValue>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ScratchRevisionMetadata {
    #[serde(default, deserialize_with = "deserialize_heading_list")]
    pub revised_headings: Vec<String>,
    #[serde(flatten)]
    pub extras: BTreeMap<String, JsonValue>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ScratchRefactorMetadata {
    #[serde(default, deserialize_with = "deserialize_heading_list")]
    pub refactored_headings: Vec<String>,
    #[serde(flatten)]
    pub extras: BTreeMap<String, JsonValue>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ScratchFixMetadata {
    #[serde(default, deserialize_with = "deserialize_heading_list")]
    pub fixed_headings: Vec<String>,
    #[serde(flatten)]
    pub extras: BTreeMap<String, JsonValue>,
}

impl Serialize for ScratchWorkType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            ScratchWorkType::Draft(data) => map.serialize_entry("draft", data)?,
            ScratchWorkType::Revision(data) => map.serialize_entry("revision", data)?,
            ScratchWorkType::Feat(data) => map.serialize_entry("feat", data)?,
            ScratchWorkType::Refactor(data) => map.serialize_entry("ref", data)?,
            ScratchWorkType::Fix(data) => map.serialize_entry("fix", data)?,
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ScratchWorkType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw: serde_json::Map<String, JsonValue> = serde_json::Map::deserialize(deserializer)?;
        if raw.len() != 1 {
            return Err(de::Error::custom(
                "work_type must contain exactly one draft|revision|feat|ref|fix entry",
            ));
        }
        let (key, value) = raw.into_iter().next().unwrap();
        match key.as_str() {
            "draft" => {
                let data: ScratchWorkloadExtras =
                    serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(ScratchWorkType::Draft(data))
            }
            "revision" => {
                let data: ScratchRevisionMetadata =
                    serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(ScratchWorkType::Revision(data))
            }
            "feat" => {
                let data: ScratchWorkloadExtras =
                    serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(ScratchWorkType::Feat(data))
            }
            "ref" => {
                let data: ScratchRefactorMetadata =
                    serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(ScratchWorkType::Refactor(data))
            }
            "fix" => {
                let data: ScratchFixMetadata =
                    serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(ScratchWorkType::Fix(data))
            }
            other => Err(de::Error::custom(format!(
                "unsupported work_type '{other}'"
            ))),
        }
    }
}

impl JsonSchema for ScratchWorkType {
    fn schema_name() -> Cow<'static, str> {
        Cow::from("ScratchWorkType")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let variants = vec![
            variant_schema("draft", generator.subschema_for::<ScratchWorkloadExtras>()),
            variant_schema(
                "revision",
                generator.subschema_for::<ScratchRevisionMetadata>(),
            ),
            variant_schema("feat", generator.subschema_for::<ScratchWorkloadExtras>()),
            variant_schema("ref", generator.subschema_for::<ScratchRefactorMetadata>()),
            variant_schema("fix", generator.subschema_for::<ScratchFixMetadata>()),
        ];

        serde_json::from_value(serde_json::json!({ "anyOf": variants }))
            .expect("valid ScratchWorkType schema")
    }
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

fn parse_variant<T>(value: &YamlValue) -> Result<T, SpecmanError>
where
    T: DeserializeOwned,
{
    serde_yaml::from_value(value.clone())
        .map_err(|err| SpecmanError::Serialization(err.to_string()))
}

fn detect_kind(mapping: &Mapping) -> FrontMatterKind {
    if has_key(mapping, "work_type") || has_key(mapping, "target") {
        return FrontMatterKind::ScratchPad;
    }
    if has_key(mapping, "spec") {
        return FrontMatterKind::Implementation;
    }
    FrontMatterKind::Specification
}

fn has_key(mapping: &Mapping, name: &str) -> bool {
    mapping.contains_key(YamlValue::String(name.to_string()))
}

fn deserialize_heading_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw: Vec<YamlValue> = Vec::deserialize(deserializer)?;
    raw.into_iter()
        .map(coerce_heading_value)
        .collect::<Result<Vec<_>, _>>()
}

fn coerce_heading_value<E>(value: YamlValue) -> Result<String, E>
where
    E: de::Error,
{
    match value {
        YamlValue::String(s) => Ok(s),
        YamlValue::Number(n) => Ok(n.to_string()),
        YamlValue::Bool(flag) => Ok(flag.to_string()),
        YamlValue::Null => Ok(String::new()),
        YamlValue::Mapping(map) => {
            if map.len() != 1 {
                return Err(E::custom("heading mapping must contain exactly one entry"));
            }
            let (key, value) = map.into_iter().next().unwrap();
            let key = coerce_heading_value::<E>(key)?;
            let value = coerce_heading_value::<E>(value)?;
            if value.is_empty() {
                Ok(key)
            } else {
                Ok(format!("{key}: {value}"))
            }
        }
        YamlValue::Sequence(_) => Err(E::custom(
            "heading value must be a scalar or single-entry mapping",
        )),
        YamlValue::Tagged(tagged) => Err(E::custom(format!(
            "unsupported tagged heading value: {:?}",
            tagged
        ))),
    }
}

fn variant_schema(key: &str, schema: Schema) -> Schema {
    serde_json::from_value(serde_json::json!({
        "type": "object",
        "properties": { key: schema },
        "required": [key],
        "minProperties": 1,
        "maxProperties": 1,
        "additionalProperties": false
    }))
    .expect("valid work_type variant schema")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn parses_specification_front_matter() {
        let yaml = r#"
name: spec-core
version: "1.0.0"
dependencies:
    - ../specman-data-model/spec.md
"#;

        let front = ArtifactFrontMatter::from_yaml_str(yaml).expect("parse front matter");
        assert!(front.as_specification().is_some());
        assert_eq!(front.kind(), FrontMatterKind::Specification);
        assert_eq!(front.name(), Some("spec-core"));
        assert_eq!(front.version(), Some("1.0.0"));
    }

    #[test]
    fn classifies_scratch_work_type() {
        let yaml = r#"
target: ../impl/foo/impl.md
work_type:
  fix:
    fixed_headings:
      - ../spec/foo/spec.md#concept
"#;

        let front = ArtifactFrontMatter::from_yaml_str(yaml).expect("parse scratch");
        let scratch = front.as_scratch().expect("scratch variant");
        let work_type = scratch.work_type.as_ref().expect("work_type present");
        assert_eq!(work_type.kind(), ScratchWorkTypeKind::Fix);
    }

    #[test]
    fn scratch_work_type_round_trip() {
        let ty = ScratchWorkType::Revision(ScratchRevisionMetadata {
            revised_headings: vec!["../spec/core/spec.md#concept".into()],
            extras: BTreeMap::new(),
        });

        let serialized = serde_yaml::to_string(&ty).expect("serialize work type");
        let deserialized: ScratchWorkType =
            serde_yaml::from_str(&serialized).expect("deserialize work type");
        assert_eq!(deserialized.kind(), ScratchWorkTypeKind::Revision);
    }

    #[test]
    fn parses_headings_with_embedded_colons() {
        let yaml = r#"
target: ../spec/specman-mcp/spec.md
work_type:
  revision:
    revised_headings:
      - Concept: Prompt Catalog
      - Concept: SpecMan Capability Parity
"#;

        let front = ArtifactFrontMatter::from_yaml_str(yaml).expect("parse scratch front matter");
        let scratch = front.as_scratch().expect("scratch variant");
        let headings = match scratch.work_type.as_ref().expect("work_type") {
            ScratchWorkType::Revision(meta) => meta.revised_headings.clone(),
            other => panic!("unexpected work_type variant: {other:?}"),
        };

        assert_eq!(
            headings,
            vec![
                "Concept: Prompt Catalog".to_string(),
                "Concept: SpecMan Capability Parity".to_string()
            ]
        );
    }
}
