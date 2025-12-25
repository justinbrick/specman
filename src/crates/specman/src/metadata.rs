use std::collections::HashSet;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

use crate::dependency_tree::{ArtifactId, ArtifactKind, normalize_persisted_reference};
use crate::error::SpecmanError;
use crate::front_matter::{self, ArtifactFrontMatter, FrontMatterKind};
use crate::persistence::PersistedArtifact;
use crate::workspace::WorkspacePaths;

use crate::front_matter::{
    ArtifactIdentityFields, DependencyEntry, ImplementationFrontMatter, ImplementingLanguage,
    LibraryReference, ReferenceEntry, ScratchFrontMatter, ScratchWorkType,
    SpecificationFrontMatter,
};

/// Request to update an artifact's YAML front matter while preserving the Markdown body.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FrontMatterUpdateRequest {
    #[serde(default)]
    pub persist: bool,
    #[serde(default)]
    pub ops: Vec<FrontMatterUpdateOp>,
}

impl FrontMatterUpdateRequest {
    pub fn new() -> Self {
        Self {
            persist: false,
            ops: Vec::new(),
        }
    }

    pub fn persist(mut self, persist: bool) -> Self {
        self.persist = persist;
        self
    }

    pub fn with_op(mut self, op: FrontMatterUpdateOp) -> Self {
        self.ops.push(op);
        self
    }
}

/// Result of applying a front matter update.
#[derive(Clone, Debug)]
pub struct FrontMatterUpdateResult {
    pub artifact: ArtifactId,
    pub updated_document: String,
    pub persisted: Option<PersistedArtifact>,
}

/// Tagged enum of supported front matter update operations.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum FrontMatterUpdateOp {
    // Identity/common
    SetName {
        name: String,
    },
    ClearName,
    SetTitle {
        title: String,
    },
    ClearTitle,
    SetDescription {
        description: String,
    },
    ClearDescription,
    SetVersion {
        version: String,
    },
    ClearVersion,
    AddTag {
        tag: String,
    },
    RemoveTag {
        tag: String,
    },

    // Spec-only
    AddDependency {
        #[serde(rename = "ref")]
        ref_: String,
        #[serde(default)]
        optional: Option<bool>,
    },
    RemoveDependency {
        #[serde(rename = "ref")]
        ref_: String,
    },
    SetRequiresImplementation {
        requires: bool,
    },
    ClearRequiresImplementation,

    // Impl-only
    SetSpec {
        #[serde(rename = "ref")]
        ref_: String,
    },
    ClearSpec,
    SetLocation {
        location: String,
    },
    ClearLocation,
    SetLibrary {
        library: LibraryReference,
    },
    ClearLibrary,
    AddReference {
        #[serde(rename = "ref")]
        ref_: String,
        #[serde(rename = "type")]
        type_: Option<String>,
        #[serde(default)]
        optional: Option<bool>,
    },
    RemoveReference {
        #[serde(rename = "ref")]
        ref_: String,
    },
    SetPrimaryLanguage {
        language: ImplementingLanguage,
    },
    ClearPrimaryLanguage,
    SetSecondaryLanguages {
        languages: Vec<ImplementingLanguage>,
    },
    ClearSecondaryLanguages,

    // Scratch-only
    SetTarget {
        target: String,
    },
    ClearTarget,
    SetBranch {
        branch: String,
    },
    ClearBranch,
    SetWorkType {
        work_type: ScratchWorkType,
    },
    ClearWorkType,
}

/// Applies a `FrontMatterUpdateRequest` to an existing Markdown document.
///
/// - Preserves the Markdown body exactly.
/// - Updates YAML front matter only.
/// - Enforces workspace boundary and persisted-locator scheme rules.
pub fn apply_front_matter_update(
    artifact: &ArtifactId,
    artifact_path: &Path,
    workspace: &WorkspacePaths,
    raw_document: &str,
    request: &FrontMatterUpdateRequest,
) -> Result<(String, bool), SpecmanError> {
    if request.ops.is_empty() {
        return Err(SpecmanError::Template(
            "front matter update requires at least one op".into(),
        ));
    }

    let parent_dir = artifact_path.parent().ok_or_else(|| {
        SpecmanError::Workspace(format!(
            "artifact {} has no parent directory",
            artifact_path.display()
        ))
    })?;

    validate_front_matter_ops(&artifact.kind, &request.ops, parent_dir, workspace)?;

    let canonical_ops =
        canonicalize_front_matter_ops(&artifact.kind, &request.ops, parent_dir, workspace)?;

    #[derive(Default)]
    struct TouchedKeys {
        name: bool,
        title: bool,
        description: bool,
        version: bool,
        tags: bool,

        requires_implementation: bool,

        spec: bool,
        location: bool,
        library: bool,
        primary_language: bool,
        secondary_languages: bool,

        references: bool,
        dependencies: bool,

        branch: bool,
        work_type: bool,
    }

    fn touched_keys(ops: &[FrontMatterUpdateOp]) -> TouchedKeys {
        let mut touched = TouchedKeys::default();
        for op in ops {
            match op {
                FrontMatterUpdateOp::SetName { .. } | FrontMatterUpdateOp::ClearName => {
                    touched.name = true;
                }
                FrontMatterUpdateOp::SetTitle { .. } | FrontMatterUpdateOp::ClearTitle => {
                    touched.title = true;
                }
                FrontMatterUpdateOp::SetDescription { .. }
                | FrontMatterUpdateOp::ClearDescription => {
                    touched.description = true;
                }
                FrontMatterUpdateOp::SetVersion { .. } | FrontMatterUpdateOp::ClearVersion => {
                    touched.version = true;
                }
                FrontMatterUpdateOp::AddTag { .. } | FrontMatterUpdateOp::RemoveTag { .. } => {
                    touched.tags = true;
                }
                FrontMatterUpdateOp::SetRequiresImplementation { .. }
                | FrontMatterUpdateOp::ClearRequiresImplementation => {
                    touched.requires_implementation = true;
                }
                FrontMatterUpdateOp::SetSpec { .. } | FrontMatterUpdateOp::ClearSpec => {
                    touched.spec = true;
                }
                FrontMatterUpdateOp::SetLocation { .. } | FrontMatterUpdateOp::ClearLocation => {
                    touched.location = true;
                }
                FrontMatterUpdateOp::SetLibrary { .. } | FrontMatterUpdateOp::ClearLibrary => {
                    touched.library = true;
                }
                FrontMatterUpdateOp::AddReference { .. }
                | FrontMatterUpdateOp::RemoveReference { .. } => {
                    touched.references = true;
                }
                FrontMatterUpdateOp::SetPrimaryLanguage { .. }
                | FrontMatterUpdateOp::ClearPrimaryLanguage => {
                    touched.primary_language = true;
                }
                FrontMatterUpdateOp::SetSecondaryLanguages { .. }
                | FrontMatterUpdateOp::ClearSecondaryLanguages => {
                    touched.secondary_languages = true;
                }
                FrontMatterUpdateOp::AddDependency { .. }
                | FrontMatterUpdateOp::RemoveDependency { .. } => {
                    touched.dependencies = true;
                }
                FrontMatterUpdateOp::SetBranch { .. } | FrontMatterUpdateOp::ClearBranch => {
                    touched.branch = true;
                }
                FrontMatterUpdateOp::SetWorkType { .. } | FrontMatterUpdateOp::ClearWorkType => {
                    touched.work_type = true;
                }
                FrontMatterUpdateOp::SetTarget { .. } | FrontMatterUpdateOp::ClearTarget => {
                    // immutable (validated elsewhere)
                }
            }
        }
        touched
    }

    fn apply_key_from_typed(target: &mut Mapping, typed: &Mapping, key: &str, touched: bool) {
        if !touched {
            return;
        }
        let k = Value::String(key.to_string());
        match typed.get(&k) {
            Some(Value::Null) | None => {
                target.remove(&k);
            }
            Some(v) => {
                target.insert(k, v.clone());
            }
        }
    }

    let (yaml_segment, body_segment, original_yaml_value) =
        match front_matter::split_front_matter(raw_document) {
            Ok(split) => {
                let yaml_str = split.yaml.to_string();
                let parsed: Value = serde_yaml::from_str(&yaml_str)
                    .map_err(|err| SpecmanError::Serialization(err.to_string()))?;
                (Some(yaml_str), split.body.to_string(), Some(parsed))
            }
            Err(_) => (None, raw_document.to_string(), None),
        };

    let mut mutated = false;

    match artifact.kind {
        ArtifactKind::Specification => {
            let touched = touched_keys(&canonical_ops);

            let mut front = if let Some(yaml) = &yaml_segment {
                let typed = ArtifactFrontMatter::from_yaml_str(yaml)?;
                ensure_kind_matches(artifact, &typed)?;
                typed.as_specification().cloned().unwrap_or_default()
            } else {
                let mut fm = SpecificationFrontMatter::default();
                fm.identity.name = Some(artifact.name.clone());
                fm
            };

            for op in &canonical_ops {
                mutated |= apply_op_spec(op, &mut front, parent_dir, workspace)?;
            }

            let yaml = if let Some(original) = &original_yaml_value {
                let mut merged = original.as_mapping().cloned().unwrap_or_else(Mapping::new);
                let typed_value = serde_yaml::to_value(&front).map_err(|err| {
                    SpecmanError::Serialization(format!("unable to encode front matter: {err}"))
                })?;
                let typed_mapping = typed_value.as_mapping().ok_or_else(|| {
                    SpecmanError::Template("front matter must be a YAML mapping".into())
                })?;

                apply_key_from_typed(&mut merged, typed_mapping, "name", touched.name);
                apply_key_from_typed(&mut merged, typed_mapping, "title", touched.title);
                apply_key_from_typed(
                    &mut merged,
                    typed_mapping,
                    "description",
                    touched.description,
                );
                apply_key_from_typed(&mut merged, typed_mapping, "version", touched.version);
                apply_key_from_typed(&mut merged, typed_mapping, "tags", touched.tags);
                apply_key_from_typed(
                    &mut merged,
                    typed_mapping,
                    "requires_implementation",
                    touched.requires_implementation,
                );
                apply_key_from_typed(
                    &mut merged,
                    typed_mapping,
                    "dependencies",
                    touched.dependencies,
                );

                serialize_front_matter_yaml(&Value::Mapping(merged))?
            } else {
                serialize_front_matter_yaml(&serde_yaml::to_value(&front).map_err(|err| {
                    SpecmanError::Serialization(format!("unable to encode front matter: {err}"))
                })?)?
            };
            let updated = compose_document(&yaml, &body_segment);
            Ok((updated, mutated || yaml_segment.is_none()))
        }
        ArtifactKind::Implementation => {
            let touched = touched_keys(&canonical_ops);

            let mut front = if let Some(yaml) = &yaml_segment {
                let typed = ArtifactFrontMatter::from_yaml_str(yaml)?;
                ensure_kind_matches(artifact, &typed)?;
                typed.as_implementation().cloned().unwrap_or_default()
            } else {
                let mut fm = ImplementationFrontMatter::default();
                fm.identity.name = Some(artifact.name.clone());
                fm
            };

            for op in &canonical_ops {
                mutated |= apply_op_impl(op, &mut front, parent_dir, workspace)?;
            }

            let yaml = if let Some(original) = &original_yaml_value {
                let mut merged = original.as_mapping().cloned().unwrap_or_else(Mapping::new);
                let typed_value = serde_yaml::to_value(&front).map_err(|err| {
                    SpecmanError::Serialization(format!("unable to encode front matter: {err}"))
                })?;
                let typed_mapping = typed_value.as_mapping().ok_or_else(|| {
                    SpecmanError::Template("front matter must be a YAML mapping".into())
                })?;

                apply_key_from_typed(&mut merged, typed_mapping, "name", touched.name);
                apply_key_from_typed(&mut merged, typed_mapping, "title", touched.title);
                apply_key_from_typed(
                    &mut merged,
                    typed_mapping,
                    "description",
                    touched.description,
                );
                apply_key_from_typed(&mut merged, typed_mapping, "version", touched.version);
                apply_key_from_typed(&mut merged, typed_mapping, "tags", touched.tags);

                apply_key_from_typed(&mut merged, typed_mapping, "spec", touched.spec);
                apply_key_from_typed(&mut merged, typed_mapping, "location", touched.location);
                apply_key_from_typed(&mut merged, typed_mapping, "library", touched.library);
                apply_key_from_typed(
                    &mut merged,
                    typed_mapping,
                    "primary_language",
                    touched.primary_language,
                );
                apply_key_from_typed(
                    &mut merged,
                    typed_mapping,
                    "secondary_languages",
                    touched.secondary_languages,
                );
                apply_key_from_typed(&mut merged, typed_mapping, "references", touched.references);
                apply_key_from_typed(
                    &mut merged,
                    typed_mapping,
                    "dependencies",
                    touched.dependencies,
                );

                serialize_front_matter_yaml(&Value::Mapping(merged))?
            } else {
                serialize_front_matter_yaml(&serde_yaml::to_value(&front).map_err(|err| {
                    SpecmanError::Serialization(format!("unable to encode front matter: {err}"))
                })?)?
            };
            let updated = compose_document(&yaml, &body_segment);
            Ok((updated, mutated || yaml_segment.is_none()))
        }
        ArtifactKind::ScratchPad => {
            let touched = touched_keys(&canonical_ops);

            let mut front = if let Some(yaml) = &yaml_segment {
                let typed = ArtifactFrontMatter::from_yaml_str(yaml)?;
                ensure_kind_matches(artifact, &typed)?;
                typed.as_scratch().cloned().unwrap_or_default()
            } else {
                let mut fm = ScratchFrontMatter::default();
                fm.identity.name = Some(artifact.name.clone());
                fm
            };

            for op in &canonical_ops {
                mutated |= apply_op_scratch(op, &mut front, parent_dir, workspace)?;
            }

            let yaml = if let Some(original) = &original_yaml_value {
                let mut merged = original.as_mapping().cloned().unwrap_or_else(Mapping::new);
                let typed_value = serde_yaml::to_value(&front).map_err(|err| {
                    SpecmanError::Serialization(format!("unable to encode front matter: {err}"))
                })?;
                let typed_mapping = typed_value.as_mapping().ok_or_else(|| {
                    SpecmanError::Template("front matter must be a YAML mapping".into())
                })?;

                apply_key_from_typed(&mut merged, typed_mapping, "name", touched.name);
                apply_key_from_typed(&mut merged, typed_mapping, "title", touched.title);
                apply_key_from_typed(
                    &mut merged,
                    typed_mapping,
                    "description",
                    touched.description,
                );
                apply_key_from_typed(&mut merged, typed_mapping, "version", touched.version);
                apply_key_from_typed(&mut merged, typed_mapping, "tags", touched.tags);

                // target is immutable; never merge it unless the request touches it (which errors).
                apply_key_from_typed(&mut merged, typed_mapping, "branch", touched.branch);
                apply_key_from_typed(&mut merged, typed_mapping, "work_type", touched.work_type);
                apply_key_from_typed(
                    &mut merged,
                    typed_mapping,
                    "dependencies",
                    touched.dependencies,
                );

                serialize_front_matter_yaml(&Value::Mapping(merged))?
            } else {
                serialize_front_matter_yaml(&serde_yaml::to_value(&front).map_err(|err| {
                    SpecmanError::Serialization(format!("unable to encode front matter: {err}"))
                })?)?
            };
            let updated = compose_document(&yaml, &body_segment);
            Ok((updated, mutated || yaml_segment.is_none()))
        }
    }
}

fn canonicalize_front_matter_ops(
    kind: &ArtifactKind,
    ops: &[FrontMatterUpdateOp],
    parent_dir: &Path,
    workspace: &WorkspacePaths,
) -> Result<Vec<FrontMatterUpdateOp>, SpecmanError> {
    // To make the API semantics declarative, apply in a canonical order so callers
    // can't accidentally depend on op ordering. Validation already rejects conflicts.
    let mut keyed: Vec<((u8, String), FrontMatterUpdateOp)> = Vec::with_capacity(ops.len());
    for op in ops {
        let (rank, key) = match op {
            // Identity scalars
            FrontMatterUpdateOp::SetName { .. } | FrontMatterUpdateOp::ClearName => {
                (0, String::new())
            }
            FrontMatterUpdateOp::SetTitle { .. } | FrontMatterUpdateOp::ClearTitle => {
                (1, String::new())
            }
            FrontMatterUpdateOp::SetDescription { .. } | FrontMatterUpdateOp::ClearDescription => {
                (2, String::new())
            }
            FrontMatterUpdateOp::SetVersion { .. } | FrontMatterUpdateOp::ClearVersion => {
                (3, String::new())
            }
            // Tags are set-like; sort by tag to ensure stable output ordering.
            FrontMatterUpdateOp::AddTag { tag } | FrontMatterUpdateOp::RemoveTag { tag } => {
                (4, tag.clone())
            }
            // Spec fields
            FrontMatterUpdateOp::SetRequiresImplementation { .. }
            | FrontMatterUpdateOp::ClearRequiresImplementation => (10, String::new()),

            // Impl fields
            FrontMatterUpdateOp::SetSpec { ref_ } => {
                let normalized = normalize_persisted_reference(ref_, parent_dir, workspace)?;
                (20, normalized)
            }
            FrontMatterUpdateOp::ClearSpec => (20, String::new()),
            FrontMatterUpdateOp::SetLocation { location } => (21, location.clone()),
            FrontMatterUpdateOp::ClearLocation => (21, String::new()),
            FrontMatterUpdateOp::SetLibrary { .. } | FrontMatterUpdateOp::ClearLibrary => {
                (22, String::new())
            }
            FrontMatterUpdateOp::SetPrimaryLanguage { .. }
            | FrontMatterUpdateOp::ClearPrimaryLanguage => (23, String::new()),
            FrontMatterUpdateOp::SetSecondaryLanguages { .. }
            | FrontMatterUpdateOp::ClearSecondaryLanguages => (24, String::new()),

            // Dependencies/references are set-like by normalized locator.
            FrontMatterUpdateOp::AddDependency { ref_, .. }
            | FrontMatterUpdateOp::RemoveDependency { ref_ } => {
                let base = match kind {
                    ArtifactKind::ScratchPad => workspace.root(),
                    _ => parent_dir,
                };
                let normalized = normalize_persisted_reference(ref_, base, workspace)?;
                (30, normalized)
            }
            FrontMatterUpdateOp::AddReference { ref_, .. }
            | FrontMatterUpdateOp::RemoveReference { ref_ } => {
                let normalized = normalize_persisted_reference(ref_, parent_dir, workspace)?;
                (31, normalized)
            }

            // Scratch fields
            FrontMatterUpdateOp::SetBranch { branch } => (40, branch.clone()),
            FrontMatterUpdateOp::ClearBranch => (40, String::new()),
            FrontMatterUpdateOp::SetWorkType { .. } | FrontMatterUpdateOp::ClearWorkType => {
                (41, String::new())
            }
            FrontMatterUpdateOp::SetTarget { .. } | FrontMatterUpdateOp::ClearTarget => {
                // Will be rejected for scratch in validation; keep a consistent ordering otherwise.
                (42, String::new())
            }
        };

        keyed.push(((rank, key), op.clone()));
    }

    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(keyed.into_iter().map(|(_, op)| op).collect())
}

fn validate_front_matter_ops(
    kind: &ArtifactKind,
    ops: &[FrontMatterUpdateOp],
    parent_dir: &Path,
    workspace: &WorkspacePaths,
) -> Result<(), SpecmanError> {
    if ops.is_empty() {
        return Err(SpecmanError::Template(
            "front matter update requires at least one op".into(),
        ));
    }

    let mut touched: HashSet<String> = HashSet::new();

    let mut touch = |key: String| -> Result<(), SpecmanError> {
        if touched.insert(key.clone()) {
            Ok(())
        } else {
            Err(SpecmanError::Template(format!(
                "conflicting front matter ops: duplicate declaration for {key}"
            )))
        }
    };

    for op in ops {
        // Kind gating first so conflict errors don't mask a kind mismatch.
        let kind_allows = match kind {
            ArtifactKind::Specification => matches!(
                op,
                FrontMatterUpdateOp::SetName { .. }
                    | FrontMatterUpdateOp::ClearName
                    | FrontMatterUpdateOp::SetTitle { .. }
                    | FrontMatterUpdateOp::ClearTitle
                    | FrontMatterUpdateOp::SetDescription { .. }
                    | FrontMatterUpdateOp::ClearDescription
                    | FrontMatterUpdateOp::SetVersion { .. }
                    | FrontMatterUpdateOp::ClearVersion
                    | FrontMatterUpdateOp::AddTag { .. }
                    | FrontMatterUpdateOp::RemoveTag { .. }
                    | FrontMatterUpdateOp::AddDependency { .. }
                    | FrontMatterUpdateOp::RemoveDependency { .. }
                    | FrontMatterUpdateOp::SetRequiresImplementation { .. }
                    | FrontMatterUpdateOp::ClearRequiresImplementation
            ),
            ArtifactKind::Implementation => matches!(
                op,
                FrontMatterUpdateOp::SetName { .. }
                    | FrontMatterUpdateOp::ClearName
                    | FrontMatterUpdateOp::SetTitle { .. }
                    | FrontMatterUpdateOp::ClearTitle
                    | FrontMatterUpdateOp::SetDescription { .. }
                    | FrontMatterUpdateOp::ClearDescription
                    | FrontMatterUpdateOp::SetVersion { .. }
                    | FrontMatterUpdateOp::ClearVersion
                    | FrontMatterUpdateOp::AddTag { .. }
                    | FrontMatterUpdateOp::RemoveTag { .. }
                    | FrontMatterUpdateOp::SetSpec { .. }
                    | FrontMatterUpdateOp::ClearSpec
                    | FrontMatterUpdateOp::SetLocation { .. }
                    | FrontMatterUpdateOp::ClearLocation
                    | FrontMatterUpdateOp::SetLibrary { .. }
                    | FrontMatterUpdateOp::ClearLibrary
                    | FrontMatterUpdateOp::AddReference { .. }
                    | FrontMatterUpdateOp::RemoveReference { .. }
                    | FrontMatterUpdateOp::AddDependency { .. }
                    | FrontMatterUpdateOp::RemoveDependency { .. }
                    | FrontMatterUpdateOp::SetPrimaryLanguage { .. }
                    | FrontMatterUpdateOp::ClearPrimaryLanguage
                    | FrontMatterUpdateOp::SetSecondaryLanguages { .. }
                    | FrontMatterUpdateOp::ClearSecondaryLanguages
            ),
            ArtifactKind::ScratchPad => matches!(
                op,
                FrontMatterUpdateOp::SetName { .. }
                    | FrontMatterUpdateOp::ClearName
                    | FrontMatterUpdateOp::SetTitle { .. }
                    | FrontMatterUpdateOp::ClearTitle
                    | FrontMatterUpdateOp::SetDescription { .. }
                    | FrontMatterUpdateOp::ClearDescription
                    | FrontMatterUpdateOp::SetVersion { .. }
                    | FrontMatterUpdateOp::ClearVersion
                    | FrontMatterUpdateOp::AddTag { .. }
                    | FrontMatterUpdateOp::RemoveTag { .. }
                    | FrontMatterUpdateOp::SetBranch { .. }
                    | FrontMatterUpdateOp::ClearBranch
                    | FrontMatterUpdateOp::SetWorkType { .. }
                    | FrontMatterUpdateOp::ClearWorkType
                    | FrontMatterUpdateOp::AddDependency { .. }
                    | FrontMatterUpdateOp::RemoveDependency { .. }
                    | FrontMatterUpdateOp::SetTarget { .. }
                    | FrontMatterUpdateOp::ClearTarget
            ),
        };

        if !kind_allows {
            // Preserve existing error class/message patterns as much as possible.
            return Err(SpecmanError::Template(match kind {
                ArtifactKind::Specification => {
                    "unsupported update op for specification front matter".into()
                }
                ArtifactKind::Implementation => {
                    "unsupported update op for implementation front matter".into()
                }
                ArtifactKind::ScratchPad => "unsupported update op for scratch front matter".into(),
            }));
        }

        // Conflict detection is order-independent: treat ops as declarative declarations.
        match op {
            FrontMatterUpdateOp::SetName { .. } | FrontMatterUpdateOp::ClearName => {
                touch("identity.name".into())?;
            }
            FrontMatterUpdateOp::SetTitle { .. } | FrontMatterUpdateOp::ClearTitle => {
                touch("identity.title".into())?;
            }
            FrontMatterUpdateOp::SetDescription { .. } | FrontMatterUpdateOp::ClearDescription => {
                touch("identity.description".into())?;
            }
            FrontMatterUpdateOp::SetVersion { .. } | FrontMatterUpdateOp::ClearVersion => {
                touch("identity.version".into())?;
            }
            FrontMatterUpdateOp::AddTag { tag } | FrontMatterUpdateOp::RemoveTag { tag } => {
                touch(format!("identity.tags:{tag}"))?;
            }
            FrontMatterUpdateOp::SetRequiresImplementation { .. }
            | FrontMatterUpdateOp::ClearRequiresImplementation => {
                touch("spec.requires_implementation".into())?;
            }
            FrontMatterUpdateOp::SetSpec { ref_ } => {
                let _ = normalize_persisted_reference(ref_, parent_dir, workspace)?;
                touch("impl.spec".into())?;
            }
            FrontMatterUpdateOp::ClearSpec => {
                touch("impl.spec".into())?;
            }
            FrontMatterUpdateOp::SetLocation { .. } | FrontMatterUpdateOp::ClearLocation => {
                touch("impl.location".into())?;
            }
            FrontMatterUpdateOp::SetLibrary { .. } | FrontMatterUpdateOp::ClearLibrary => {
                touch("impl.library".into())?;
            }
            FrontMatterUpdateOp::SetPrimaryLanguage { .. }
            | FrontMatterUpdateOp::ClearPrimaryLanguage => {
                touch("impl.primary_language".into())?;
            }
            FrontMatterUpdateOp::SetSecondaryLanguages { .. }
            | FrontMatterUpdateOp::ClearSecondaryLanguages => {
                touch("impl.secondary_languages".into())?;
            }
            FrontMatterUpdateOp::SetBranch { .. } | FrontMatterUpdateOp::ClearBranch => {
                touch("scratch.branch".into())?;
            }
            FrontMatterUpdateOp::SetWorkType { .. } | FrontMatterUpdateOp::ClearWorkType => {
                touch("scratch.work_type".into())?;
            }
            FrontMatterUpdateOp::SetTarget { .. } | FrontMatterUpdateOp::ClearTarget => {
                // Even though the enum supports it, scratch targets are immutable per spec.
                if matches!(kind, ArtifactKind::ScratchPad) {
                    return Err(SpecmanError::Template(
                        "scratch pad `target` is immutable; mutation attempts must fail".into(),
                    ));
                }
                touch("scratch.target".into())?;
            }
            FrontMatterUpdateOp::AddDependency { ref_, .. }
            | FrontMatterUpdateOp::RemoveDependency { ref_ } => {
                let base = match kind {
                    ArtifactKind::ScratchPad => workspace.root(),
                    _ => parent_dir,
                };
                let normalized = normalize_persisted_reference(ref_, base, workspace)?;
                touch(format!("dependencies:{normalized}"))?;
            }
            FrontMatterUpdateOp::AddReference { ref_, .. }
            | FrontMatterUpdateOp::RemoveReference { ref_ } => {
                let normalized = normalize_persisted_reference(ref_, parent_dir, workspace)?;
                touch(format!("references:{normalized}"))?;
            }
        }
    }

    Ok(())
}

fn ensure_kind_matches(
    artifact: &ArtifactId,
    front: &ArtifactFrontMatter,
) -> Result<(), SpecmanError> {
    let expected = artifact.kind;
    let actual = artifact_kind_from_front(front);
    if expected != actual {
        return Err(SpecmanError::Template(format!(
            "artifact kind mismatch: requested {:?} but file front matter looks like {:?}",
            expected, actual
        )));
    }
    Ok(())
}

fn apply_identity_ops(op: &FrontMatterUpdateOp, identity: &mut ArtifactIdentityFields) -> bool {
    match op {
        FrontMatterUpdateOp::SetName { name } => {
            let changed = identity.name.as_deref() != Some(name);
            identity.name = Some(name.clone());
            changed
        }
        FrontMatterUpdateOp::ClearName => {
            let changed = identity.name.is_some();
            identity.name = None;
            changed
        }
        FrontMatterUpdateOp::SetTitle { title } => {
            let changed = identity.title.as_deref() != Some(title);
            identity.title = Some(title.clone());
            changed
        }
        FrontMatterUpdateOp::ClearTitle => {
            let changed = identity.title.is_some();
            identity.title = None;
            changed
        }
        FrontMatterUpdateOp::SetDescription { description } => {
            let changed = identity.description.as_deref() != Some(description);
            identity.description = Some(description.clone());
            changed
        }
        FrontMatterUpdateOp::ClearDescription => {
            let changed = identity.description.is_some();
            identity.description = None;
            changed
        }
        FrontMatterUpdateOp::SetVersion { version } => {
            let changed = identity.version.as_deref() != Some(version);
            identity.version = Some(version.clone());
            changed
        }
        FrontMatterUpdateOp::ClearVersion => {
            let changed = identity.version.is_some();
            identity.version = None;
            changed
        }
        FrontMatterUpdateOp::AddTag { tag } => {
            if identity.tags.iter().any(|t| t == tag) {
                false
            } else {
                identity.tags.push(tag.clone());
                true
            }
        }
        FrontMatterUpdateOp::RemoveTag { tag } => {
            let before = identity.tags.len();
            identity.tags.retain(|t| t != tag);
            before != identity.tags.len()
        }
        _ => false,
    }
}

fn apply_op_spec(
    op: &FrontMatterUpdateOp,
    front: &mut SpecificationFrontMatter,
    parent_dir: &Path,
    workspace: &WorkspacePaths,
) -> Result<bool, SpecmanError> {
    let mut changed = apply_identity_ops(op, &mut front.identity);
    match op {
        FrontMatterUpdateOp::AddDependency { ref_, optional } => {
            let normalized = normalize_persisted_reference(ref_, parent_dir, workspace)?;
            changed |= upsert_dependency(&mut front.dependencies, &normalized, *optional);
        }
        FrontMatterUpdateOp::RemoveDependency { ref_ } => {
            let normalized = normalize_persisted_reference(ref_, parent_dir, workspace)?;
            let before = front.dependencies.len();
            front
                .dependencies
                .retain(|d| dependency_ref(d).map(|r| r != normalized).unwrap_or(true));
            changed |= before != front.dependencies.len();
        }
        FrontMatterUpdateOp::SetRequiresImplementation { requires } => {
            let changed_local = front.requires_implementation != Some(*requires);
            front.requires_implementation = Some(*requires);
            changed |= changed_local;
        }
        FrontMatterUpdateOp::ClearRequiresImplementation => {
            let changed_local = front.requires_implementation.is_some();
            front.requires_implementation = None;
            changed |= changed_local;
        }
        _ => {
            // Reject impl/scratch-only operations for spec.
            if is_kind_specific_op(op) {
                return Err(SpecmanError::Template(
                    "unsupported update op for specification front matter".into(),
                ));
            }
        }
    }
    Ok(changed)
}

fn apply_op_impl(
    op: &FrontMatterUpdateOp,
    front: &mut ImplementationFrontMatter,
    parent_dir: &Path,
    workspace: &WorkspacePaths,
) -> Result<bool, SpecmanError> {
    let mut changed = apply_identity_ops(op, &mut front.identity);
    match op {
        FrontMatterUpdateOp::SetSpec { ref_ } => {
            let normalized = normalize_persisted_reference(ref_, parent_dir, workspace)?;
            let changed_local = front.spec.as_deref() != Some(&normalized);
            front.spec = Some(normalized);
            changed |= changed_local;
        }
        FrontMatterUpdateOp::ClearSpec => {
            let changed_local = front.spec.is_some();
            front.spec = None;
            changed |= changed_local;
        }
        FrontMatterUpdateOp::SetLocation { location } => {
            let changed_local = front.location.as_deref() != Some(location);
            front.location = Some(location.clone());
            changed |= changed_local;
        }
        FrontMatterUpdateOp::ClearLocation => {
            let changed_local = front.location.is_some();
            front.location = None;
            changed |= changed_local;
        }
        FrontMatterUpdateOp::SetLibrary { library } => {
            // library is optional in the struct.
            let changed_local = match &front.library {
                Some(existing) => !library_eq(existing, library),
                None => true,
            };
            front.library = Some(library.clone());
            changed |= changed_local;
        }
        FrontMatterUpdateOp::ClearLibrary => {
            let changed_local = front.library.is_some();
            front.library = None;
            changed |= changed_local;
        }
        FrontMatterUpdateOp::AddReference {
            ref_,
            type_,
            optional,
        } => {
            let normalized = normalize_persisted_reference(ref_, parent_dir, workspace)?;
            changed |= upsert_reference(&mut front.references, &normalized, type_, *optional);
        }
        FrontMatterUpdateOp::RemoveReference { ref_ } => {
            let normalized = normalize_persisted_reference(ref_, parent_dir, workspace)?;
            let before = front.references.len();
            front.references.retain(|r| r.reference != normalized);
            changed |= before != front.references.len();
        }
        FrontMatterUpdateOp::AddDependency { ref_, optional } => {
            let normalized = normalize_persisted_reference(ref_, parent_dir, workspace)?;
            changed |= upsert_dependency(&mut front.dependencies, &normalized, *optional);
        }
        FrontMatterUpdateOp::RemoveDependency { ref_ } => {
            let normalized = normalize_persisted_reference(ref_, parent_dir, workspace)?;
            let before = front.dependencies.len();
            front
                .dependencies
                .retain(|d| dependency_ref(d).map(|r| r != normalized).unwrap_or(true));
            changed |= before != front.dependencies.len();
        }
        FrontMatterUpdateOp::SetPrimaryLanguage { language } => {
            let changed_local = match &front.primary_language {
                Some(existing) => {
                    serde_json::to_value(existing).ok() != serde_json::to_value(language).ok()
                }
                None => true,
            };
            front.primary_language = Some(language.clone());
            changed |= changed_local;
        }
        FrontMatterUpdateOp::ClearPrimaryLanguage => {
            let changed_local = front.primary_language.is_some();
            front.primary_language = None;
            changed |= changed_local;
        }
        FrontMatterUpdateOp::SetSecondaryLanguages { languages } => {
            let changed_local = serde_json::to_value(&front.secondary_languages).ok()
                != serde_json::to_value(languages).ok();
            front.secondary_languages = languages.clone();
            changed |= changed_local;
        }
        FrontMatterUpdateOp::ClearSecondaryLanguages => {
            let changed_local = !front.secondary_languages.is_empty();
            front.secondary_languages.clear();
            changed |= changed_local;
        }
        _ => {
            if is_kind_specific_op(op) {
                return Err(SpecmanError::Template(
                    "unsupported update op for implementation front matter".into(),
                ));
            }
        }
    }
    Ok(changed)
}

fn apply_op_scratch(
    op: &FrontMatterUpdateOp,
    front: &mut ScratchFrontMatter,
    _parent_dir: &Path,
    workspace: &WorkspacePaths,
) -> Result<bool, SpecmanError> {
    let mut changed = apply_identity_ops(op, &mut front.identity);
    match op {
        FrontMatterUpdateOp::SetTarget { .. } | FrontMatterUpdateOp::ClearTarget => {
            return Err(SpecmanError::Template(
                "scratch pad `target` is immutable; mutation attempts must fail".into(),
            ));
        }
        FrontMatterUpdateOp::SetBranch { branch } => {
            let changed_local = front.branch.as_deref() != Some(branch);
            front.branch = Some(branch.clone());
            changed |= changed_local;
        }
        FrontMatterUpdateOp::ClearBranch => {
            let changed_local = front.branch.is_some();
            front.branch = None;
            changed |= changed_local;
        }
        FrontMatterUpdateOp::SetWorkType { work_type } => {
            let changed_local = serde_json::to_value(&front.work_type).ok()
                != serde_json::to_value(&Some(work_type.clone())).ok();
            front.work_type = Some(work_type.clone());
            changed |= changed_local;
        }
        FrontMatterUpdateOp::ClearWorkType => {
            let changed_local = front.work_type.is_some();
            front.work_type = None;
            changed |= changed_local;
        }
        FrontMatterUpdateOp::AddDependency { ref_, optional } => {
            let normalized = normalize_persisted_reference(ref_, workspace.root(), workspace)?;
            changed |= upsert_dependency(&mut front.dependencies, &normalized, *optional);
        }
        FrontMatterUpdateOp::RemoveDependency { ref_ } => {
            let normalized = normalize_persisted_reference(ref_, workspace.root(), workspace)?;
            let before = front.dependencies.len();
            front
                .dependencies
                .retain(|d| dependency_ref(d).map(|r| r != normalized).unwrap_or(true));
            changed |= before != front.dependencies.len();
        }
        _ => {
            if is_kind_specific_op(op) {
                return Err(SpecmanError::Template(
                    "unsupported update op for scratch front matter".into(),
                ));
            }
        }
    }
    Ok(changed)
}

fn is_kind_specific_op(op: &FrontMatterUpdateOp) -> bool {
    matches!(
        op,
        FrontMatterUpdateOp::AddDependency { .. }
            | FrontMatterUpdateOp::RemoveDependency { .. }
            | FrontMatterUpdateOp::SetRequiresImplementation { .. }
            | FrontMatterUpdateOp::ClearRequiresImplementation
            | FrontMatterUpdateOp::SetSpec { .. }
            | FrontMatterUpdateOp::ClearSpec
            | FrontMatterUpdateOp::SetLocation { .. }
            | FrontMatterUpdateOp::ClearLocation
            | FrontMatterUpdateOp::SetLibrary { .. }
            | FrontMatterUpdateOp::ClearLibrary
            | FrontMatterUpdateOp::AddReference { .. }
            | FrontMatterUpdateOp::RemoveReference { .. }
            | FrontMatterUpdateOp::SetPrimaryLanguage { .. }
            | FrontMatterUpdateOp::ClearPrimaryLanguage
            | FrontMatterUpdateOp::SetSecondaryLanguages { .. }
            | FrontMatterUpdateOp::ClearSecondaryLanguages
            | FrontMatterUpdateOp::SetTarget { .. }
            | FrontMatterUpdateOp::ClearTarget
            | FrontMatterUpdateOp::SetBranch { .. }
            | FrontMatterUpdateOp::ClearBranch
            | FrontMatterUpdateOp::SetWorkType { .. }
            | FrontMatterUpdateOp::ClearWorkType
    )
}

fn dependency_ref(entry: &DependencyEntry) -> Option<&str> {
    match entry {
        DependencyEntry::Simple(s) => Some(s.as_str()),
        DependencyEntry::Detailed(obj) => Some(obj.reference.as_str()),
    }
}

fn upsert_dependency(
    deps: &mut Vec<DependencyEntry>,
    reference: &str,
    optional: Option<bool>,
) -> bool {
    for entry in deps.iter_mut() {
        match entry {
            DependencyEntry::Simple(existing) if existing == reference => {
                if optional.is_some() {
                    *entry = DependencyEntry::Detailed(crate::front_matter::DependencyObject {
                        reference: reference.to_string(),
                        optional,
                    });
                    return true;
                }
                return false;
            }
            DependencyEntry::Detailed(existing) if existing.reference == reference => {
                if optional.is_some() && existing.optional != optional {
                    existing.optional = optional;
                    return true;
                }
                return false;
            }
            _ => {}
        }
    }

    if let Some(optional) = optional {
        deps.push(DependencyEntry::Detailed(
            crate::front_matter::DependencyObject {
                reference: reference.to_string(),
                optional: Some(optional),
            },
        ));
    } else {
        deps.push(DependencyEntry::Simple(reference.to_string()));
    }
    true
}

fn upsert_reference(
    refs: &mut Vec<ReferenceEntry>,
    reference: &str,
    type_: &Option<String>,
    optional: Option<bool>,
) -> bool {
    for entry in refs.iter_mut() {
        if entry.reference == reference {
            let mut changed = false;
            if let Some(t) = type_ {
                if entry.reference_type.as_deref() != Some(t) {
                    entry.reference_type = Some(t.clone());
                    changed = true;
                }
            }
            if optional.is_some() && entry.optional != optional {
                entry.optional = optional;
                changed = true;
            }
            return changed;
        }
    }

    refs.push(ReferenceEntry {
        reference: reference.to_string(),
        reference_type: type_.clone(),
        optional,
    });
    true
}

fn library_eq(a: &LibraryReference, b: &LibraryReference) -> bool {
    // Avoid adding `PartialEq` to the public data model types.
    serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
}

fn serialize_front_matter_yaml(value: &Value) -> Result<String, SpecmanError> {
    let mut serialized =
        serde_yaml::to_string(value).map_err(|err| SpecmanError::Serialization(err.to_string()))?;
    if let Some(stripped) = serialized.strip_prefix("---\n") {
        serialized = stripped.to_string();
    }
    if serialized.ends_with("...\n") {
        serialized.truncate(serialized.len() - 4);
    }
    Ok(serialized.trim_end().to_string())
}

fn compose_document(yaml: &str, body: &str) -> String {
    let mut output = String::from("---\n");
    output.push_str(yaml);
    if !yaml.ends_with('\n') {
        output.push('\n');
    }
    output.push_str("---\n");
    output.push_str(body);
    output
}

fn artifact_kind_from_front(front: &ArtifactFrontMatter) -> ArtifactKind {
    match front.kind() {
        FrontMatterKind::Specification => ArtifactKind::Specification,
        FrontMatterKind::Implementation => ArtifactKind::Implementation,
        FrontMatterKind::ScratchPad => ArtifactKind::ScratchPad,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dependency_tree::ArtifactKind;
    use crate::workspace::FilesystemWorkspaceLocator;
    use crate::workspace::WorkspaceLocator;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn scratch_target_set_is_rejected() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join(".specman/scratchpad/demo")).unwrap();

        let scratch_path = root.join(".specman/scratchpad/demo/scratch.md");
        let raw = "---\ntarget: impl/some-impl/impl.md\nwork_type:\n  ref:\n    refactored_headings: []\n---\n# Scratch\n";
        fs::write(&scratch_path, raw).unwrap();

        let workspace = FilesystemWorkspaceLocator::new(&root)
            .workspace()
            .expect("workspace discovery");
        let artifact = ArtifactId {
            kind: ArtifactKind::ScratchPad,
            name: "demo".into(),
        };

        let request = FrontMatterUpdateRequest::new().with_op(FrontMatterUpdateOp::SetTarget {
            target: "impl/other/impl.md".into(),
        });

        let err = apply_front_matter_update(&artifact, &scratch_path, &workspace, raw, &request)
            .expect_err("scratch target updates must fail");
        assert!(matches!(err, SpecmanError::Template(_)));
        assert!(err.to_string().contains("immutable"));
    }

    #[test]
    fn scratch_target_clear_is_rejected() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join(".specman/scratchpad/demo")).unwrap();

        let scratch_path = root.join(".specman/scratchpad/demo/scratch.md");
        let raw = "---\ntarget: impl/some-impl/impl.md\nwork_type:\n  ref:\n    refactored_headings: []\n---\n# Scratch\n";
        fs::write(&scratch_path, raw).unwrap();

        let workspace = FilesystemWorkspaceLocator::new(&root)
            .workspace()
            .expect("workspace discovery");
        let artifact = ArtifactId {
            kind: ArtifactKind::ScratchPad,
            name: "demo".into(),
        };

        let request = FrontMatterUpdateRequest::new().with_op(FrontMatterUpdateOp::ClearTarget);

        let err = apply_front_matter_update(&artifact, &scratch_path, &workspace, raw, &request)
            .expect_err("scratch target updates must fail");
        assert!(matches!(err, SpecmanError::Template(_)));
        assert!(err.to_string().contains("immutable"));
    }

    #[test]
    fn front_matter_update_rejects_conflicting_version_ops() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/core")).unwrap();

        let spec_path = root.join("spec/core/spec.md");
        let raw = "---\nname: core\nversion: \"1.0.0\"\n---\n# Core\n";
        fs::write(&spec_path, raw).unwrap();

        let workspace = FilesystemWorkspaceLocator::new(&root)
            .workspace()
            .expect("workspace discovery");
        let artifact = ArtifactId {
            kind: ArtifactKind::Specification,
            name: "core".into(),
        };

        let request = FrontMatterUpdateRequest::new()
            .with_op(FrontMatterUpdateOp::SetVersion {
                version: "2.1.0".into(),
            })
            .with_op(FrontMatterUpdateOp::ClearVersion);

        let err = apply_front_matter_update(&artifact, &spec_path, &workspace, raw, &request)
            .expect_err("conflicting ops must fail");
        assert!(matches!(err, SpecmanError::Template(_)));
        assert!(err.to_string().contains("conflicting front matter ops"));
    }

    #[test]
    fn front_matter_update_rejects_conflicting_tag_ops() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/core")).unwrap();

        let spec_path = root.join("spec/core/spec.md");
        let raw = "---\nname: core\n---\n# Core\n";
        fs::write(&spec_path, raw).unwrap();

        let workspace = FilesystemWorkspaceLocator::new(&root)
            .workspace()
            .expect("workspace discovery");
        let artifact = ArtifactId {
            kind: ArtifactKind::Specification,
            name: "core".into(),
        };

        let request = FrontMatterUpdateRequest::new()
            .with_op(FrontMatterUpdateOp::AddTag { tag: "demo".into() })
            .with_op(FrontMatterUpdateOp::RemoveTag { tag: "demo".into() });

        let err = apply_front_matter_update(&artifact, &spec_path, &workspace, raw, &request)
            .expect_err("conflicting ops must fail");
        assert!(matches!(err, SpecmanError::Template(_)));
        assert!(err.to_string().contains("identity.tags:demo"));
    }

    #[test]
    fn front_matter_update_rejects_dependency_add_remove_conflict_after_normalization() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/data-model")).unwrap();
        fs::create_dir_all(root.join("spec/core")).unwrap();

        fs::write(
            root.join("spec/data-model/spec.md"),
            "---\nname: data-model\n---\n# Data Model\n",
        )
        .unwrap();

        let spec_path = root.join("spec/core/spec.md");
        let raw = "---\nname: core\n---\n# Core\n";
        fs::write(&spec_path, raw).unwrap();

        let workspace = FilesystemWorkspaceLocator::new(&root)
            .workspace()
            .expect("workspace discovery");
        let artifact = ArtifactId {
            kind: ArtifactKind::Specification,
            name: "core".into(),
        };

        // These two locators normalize to the same persisted value from spec/core.
        let request = FrontMatterUpdateRequest::new()
            .with_op(FrontMatterUpdateOp::AddDependency {
                ref_: "spec://data-model".into(),
                optional: None,
            })
            .with_op(FrontMatterUpdateOp::RemoveDependency {
                ref_: "../data-model/spec.md".into(),
            });

        let err = apply_front_matter_update(&artifact, &spec_path, &workspace, raw, &request)
            .expect_err("conflicting ops must fail");
        assert!(
            err.to_string().contains("conflicting front matter ops"),
            "expected conflict error, got: {err:?}"
        );
        assert!(
            err.to_string()
                .contains("dependencies:../data-model/spec.md")
        );
    }

    #[test]
    fn front_matter_update_rejects_duplicate_dependency_adds_even_if_optional_differs() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/alpha")).unwrap();
        fs::create_dir_all(root.join("spec/core")).unwrap();

        fs::write(
            root.join("spec/alpha/spec.md"),
            "---\nname: alpha\n---\n# Alpha\n",
        )
        .unwrap();

        let spec_path = root.join("spec/core/spec.md");
        let raw = "---\nname: core\n---\n# Core\n";
        fs::write(&spec_path, raw).unwrap();

        let workspace = FilesystemWorkspaceLocator::new(&root)
            .workspace()
            .expect("workspace discovery");
        let artifact = ArtifactId {
            kind: ArtifactKind::Specification,
            name: "core".into(),
        };

        let request = FrontMatterUpdateRequest::new()
            .with_op(FrontMatterUpdateOp::AddDependency {
                ref_: "../alpha/spec.md".into(),
                optional: Some(false),
            })
            .with_op(FrontMatterUpdateOp::AddDependency {
                ref_: "../alpha/spec.md".into(),
                optional: Some(true),
            });

        let err = apply_front_matter_update(&artifact, &spec_path, &workspace, raw, &request)
            .expect_err("duplicate declaration must fail");
        assert!(
            err.to_string().contains("conflicting front matter ops"),
            "expected conflict error, got: {err:?}"
        );
        assert!(err.to_string().contains("dependencies:../alpha/spec.md"));
    }

    #[test]
    fn front_matter_update_rejects_reference_add_remove_conflict_after_normalization() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/beta")).unwrap();
        fs::create_dir_all(root.join("impl/core")).unwrap();

        fs::write(
            root.join("spec/beta/spec.md"),
            "---\nname: beta\n---\n# Beta\n",
        )
        .unwrap();

        let impl_path = root.join("impl/core/impl.md");
        let raw = "---\nname: core\nspec: ../../spec/beta/spec.md\n---\n# Impl\n";
        fs::write(&impl_path, raw).unwrap();

        let workspace = FilesystemWorkspaceLocator::new(&root)
            .workspace()
            .expect("workspace discovery");
        let artifact = ArtifactId {
            kind: ArtifactKind::Implementation,
            name: "core".into(),
        };

        // From impl/core, spec://beta normalizes to ../../spec/beta/spec.md.
        let request = FrontMatterUpdateRequest::new()
            .with_op(FrontMatterUpdateOp::AddReference {
                ref_: "spec://beta".into(),
                type_: None,
                optional: None,
            })
            .with_op(FrontMatterUpdateOp::RemoveReference {
                ref_: "../../spec/beta/spec.md".into(),
            });

        let err = apply_front_matter_update(&artifact, &impl_path, &workspace, raw, &request)
            .expect_err("conflicting ops must fail");
        assert!(
            err.to_string().contains("conflicting front matter ops"),
            "expected conflict error, got: {err:?}"
        );
        assert!(
            err.to_string()
                .contains("references:../../spec/beta/spec.md")
        );
    }

    #[test]
    fn front_matter_update_spec_adds_dependency_and_persists() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/data-model")).unwrap();
        fs::create_dir_all(root.join("spec/core")).unwrap();

        let dependency_path = root.join("spec/data-model/spec.md");
        fs::write(
            &dependency_path,
            "---\nname: data-model\nversion: \"1.0.0\"\n---\n# Data Model",
        )
        .unwrap();

        let spec_path = root.join("spec/core/spec.md");
        fs::write(
            &spec_path,
            "---\nname: spec-core\nversion: \"1.0.0\"\n---\n# Core",
        )
        .unwrap();

        let workspace = FilesystemWorkspaceLocator::new(&root)
            .workspace()
            .expect("workspace discovery");
        let artifact = ArtifactId {
            kind: ArtifactKind::Specification,
            name: "spec-core".into(),
        };

        let raw = fs::read_to_string(&spec_path).unwrap();
        let request = FrontMatterUpdateRequest::new().persist(true).with_op(
            FrontMatterUpdateOp::AddDependency {
                ref_: "../data-model/spec.md".into(),
                optional: None,
            },
        );

        let (updated_document, mutated) =
            apply_front_matter_update(&artifact, &spec_path, &workspace, &raw, &request)
                .expect("update succeeds");

        assert!(mutated);
        if request.persist {
            fs::write(&spec_path, &updated_document).unwrap();
        }

        let contents = fs::read_to_string(spec_path).unwrap();
        assert!(contents.contains("dependencies"));
        assert!(contents.contains("../data-model/spec.md"));
    }

    #[test]
    fn front_matter_update_spec_accepts_resource_handle_dependency() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/data-model")).unwrap();
        fs::create_dir_all(root.join("spec/core")).unwrap();

        fs::write(
            root.join("spec/data-model/spec.md"),
            "---\nname: data-model\n---\n# Data Model",
        )
        .unwrap();

        let spec_path = root.join("spec/core/spec.md");
        fs::write(&spec_path, "---\nname: core\n---\n# Core").unwrap();

        let workspace = FilesystemWorkspaceLocator::new(&root)
            .workspace()
            .expect("workspace discovery");
        let artifact = ArtifactId {
            kind: ArtifactKind::Specification,
            name: "core".into(),
        };

        let raw = fs::read_to_string(&spec_path).unwrap();
        let request = FrontMatterUpdateRequest::new().with_op(FrontMatterUpdateOp::AddDependency {
            ref_: "spec://data-model".into(),
            optional: None,
        });

        let (updated_document, mutated) =
            apply_front_matter_update(&artifact, &spec_path, &workspace, &raw, &request)
                .expect("handle dependency accepted");

        assert!(mutated);
        assert!(!updated_document.contains("spec://data-model"));
        assert!(updated_document.contains("../data-model/spec.md"));
    }

    #[test]
    fn front_matter_update_impl_adds_reference_without_duplicates() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/spec-alpha")).unwrap();
        fs::create_dir_all(root.join("spec/spec-beta")).unwrap();
        fs::create_dir_all(root.join("impl/spec-alpha")).unwrap();

        fs::write(
            root.join("spec/spec-alpha/spec.md"),
            "---\nname: spec-alpha\nversion: \"1.0.0\"\n---\n# Alpha",
        )
        .unwrap();
        fs::write(
            root.join("spec/spec-beta/spec.md"),
            "---\nname: spec-beta\nversion: \"1.0.0\"\n---\n# Beta",
        )
        .unwrap();

        let impl_path = root.join("impl/spec-alpha/impl.md");
        fs::write(
            &impl_path,
            "---\nspec: ../../spec/spec-alpha/spec.md\nname: spec-alpha-impl\nversion: \"1.0.0\"\n---\n# Impl",
        )
        .unwrap();

        let workspace = FilesystemWorkspaceLocator::new(&root)
            .workspace()
            .expect("workspace discovery");
        let artifact = ArtifactId {
            kind: ArtifactKind::Implementation,
            name: "spec-alpha-impl".into(),
        };

        let request = FrontMatterUpdateRequest::new().persist(true).with_op(
            FrontMatterUpdateOp::AddReference {
                ref_: "../../spec/spec-beta/spec.md".into(),
                type_: None,
                optional: None,
            },
        );

        let raw = fs::read_to_string(&impl_path).unwrap();
        let (updated_document, mutated) =
            apply_front_matter_update(&artifact, &impl_path, &workspace, &raw, &request)
                .expect("first mutation");

        assert!(mutated);
        if request.persist {
            fs::write(&impl_path, &updated_document).unwrap();
        }

        let raw_again = fs::read_to_string(&impl_path).unwrap();
        let (_updated_again, mutated_again) =
            apply_front_matter_update(&artifact, &impl_path, &workspace, &raw_again, &request)
                .expect("second mutation");

        assert!(!mutated_again, "no-op should skip persist");

        let contents = fs::read_to_string(&impl_path).unwrap();
        let count = contents.matches("../../spec/spec-beta/spec.md").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn front_matter_update_rejects_http_references() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/core")).unwrap();

        let spec_path = root.join("spec/core/spec.md");
        fs::write(
            &spec_path,
            "---\nname: spec-core\nversion: \"1.0.0\"\n---\n# Core",
        )
        .unwrap();

        let workspace = FilesystemWorkspaceLocator::new(&root)
            .workspace()
            .expect("workspace discovery");
        let artifact = ArtifactId {
            kind: ArtifactKind::Specification,
            name: "spec-core".into(),
        };

        let raw = fs::read_to_string(&spec_path).unwrap();
        let request = FrontMatterUpdateRequest::new().with_op(FrontMatterUpdateOp::AddDependency {
            ref_: "http://example.com/spec.md".into(),
            optional: None,
        });

        let err = apply_front_matter_update(&artifact, &spec_path, &workspace, &raw, &request)
            .expect_err("http refs rejected");
        assert!(matches!(err, SpecmanError::Dependency(_)));
    }
}
