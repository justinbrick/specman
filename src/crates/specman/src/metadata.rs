use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

use crate::adapter::DataModelAdapter;
use crate::dependency_tree::{
    ArtifactId, ArtifactKind, normalize_persisted_reference, validate_workspace_reference,
};
use crate::error::SpecmanError;
use crate::front_matter::{self, ArtifactFrontMatter, FrontMatterKind};
use crate::persistence::PersistedArtifact;
use crate::workspace::{WorkspaceLocator, WorkspacePaths};

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

    let (yaml_segment, body_segment) = match front_matter::split_front_matter(raw_document) {
        Ok(split) => (Some(split.yaml.to_string()), split.body.to_string()),
        Err(_) => (None, raw_document.to_string()),
    };

    let mut mutated = false;

    match artifact.kind {
        ArtifactKind::Specification => {
            let mut front = if let Some(yaml) = &yaml_segment {
                let typed = ArtifactFrontMatter::from_yaml_str(yaml)?;
                ensure_kind_matches(artifact, &typed)?;
                typed.as_specification().cloned().unwrap_or_default()
            } else {
                let mut fm = SpecificationFrontMatter::default();
                fm.identity.name = Some(artifact.name.clone());
                fm
            };

            for op in &request.ops {
                mutated |= apply_op_spec(op, &mut front, parent_dir, workspace)?;
            }

            let yaml =
                serialize_front_matter_yaml(&serde_yaml::to_value(&front).map_err(|err| {
                    SpecmanError::Serialization(format!("unable to encode front matter: {err}"))
                })?)?;
            let updated = compose_document(&yaml, &body_segment);
            Ok((updated, mutated || yaml_segment.is_none()))
        }
        ArtifactKind::Implementation => {
            let mut front = if let Some(yaml) = &yaml_segment {
                let typed = ArtifactFrontMatter::from_yaml_str(yaml)?;
                ensure_kind_matches(artifact, &typed)?;
                typed.as_implementation().cloned().unwrap_or_default()
            } else {
                let mut fm = ImplementationFrontMatter::default();
                fm.identity.name = Some(artifact.name.clone());
                fm
            };

            for op in &request.ops {
                mutated |= apply_op_impl(op, &mut front, parent_dir, workspace)?;
            }

            let yaml =
                serialize_front_matter_yaml(&serde_yaml::to_value(&front).map_err(|err| {
                    SpecmanError::Serialization(format!("unable to encode front matter: {err}"))
                })?)?;
            let updated = compose_document(&yaml, &body_segment);
            Ok((updated, mutated || yaml_segment.is_none()))
        }
        ArtifactKind::ScratchPad => {
            let mut front = if let Some(yaml) = &yaml_segment {
                let typed = ArtifactFrontMatter::from_yaml_str(yaml)?;
                ensure_kind_matches(artifact, &typed)?;
                typed.as_scratch().cloned().unwrap_or_default()
            } else {
                let mut fm = ScratchFrontMatter::default();
                fm.identity.name = Some(artifact.name.clone());
                fm
            };

            for op in &request.ops {
                mutated |= apply_op_scratch(op, &mut front, parent_dir, workspace)?;
            }

            let yaml =
                serialize_front_matter_yaml(&serde_yaml::to_value(&front).map_err(|err| {
                    SpecmanError::Serialization(format!("unable to encode front matter: {err}"))
                })?)?;
            let updated = compose_document(&yaml, &body_segment);
            Ok((updated, mutated || yaml_segment.is_none()))
        }
    }
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
        FrontMatterUpdateOp::SetTarget { target } => {
            let normalized = normalize_persisted_reference(target, workspace.root(), workspace)?;
            let changed_local = front.target.as_deref() != Some(&normalized);
            front.target = Some(normalized);
            changed |= changed_local;
        }
        FrontMatterUpdateOp::ClearTarget => {
            let changed_local = front.target.is_some();
            front.target = None;
            changed |= changed_local;
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

/// Adds dependencies or references to an artifact's YAML front matter without
/// rewriting the Markdown body.
pub struct MetadataMutator<L: WorkspaceLocator> {
    workspace: L,
    adapter: Option<Arc<dyn DataModelAdapter>>,
}

impl<L: WorkspaceLocator> MetadataMutator<L> {
    pub fn new(workspace: L) -> Self {
        Self {
            workspace,
            adapter: None,
        }
    }

    pub fn with_adapter(workspace: L, adapter: Arc<dyn DataModelAdapter>) -> Self {
        Self {
            workspace,
            adapter: Some(adapter),
        }
    }

    pub fn mutate(
        &self,
        request: MetadataMutationRequest,
    ) -> Result<MetadataMutationResult, SpecmanError> {
        if request.add_dependencies.is_empty() && request.add_references.is_empty() {
            return Err(SpecmanError::Template(
                "metadata mutation requires at least one operation".into(),
            ));
        }

        let workspace_paths = self.workspace.workspace()?;
        let canonical_path = fs::canonicalize(&request.path)?;
        if !canonical_path.starts_with(workspace_paths.root()) {
            return Err(SpecmanError::Workspace(format!(
                "path {} is outside the workspace {}",
                canonical_path.display(),
                workspace_paths.root().display()
            )));
        }

        let dir = canonical_path.parent().ok_or_else(|| {
            SpecmanError::Workspace(format!(
                "artifact {} has no parent directory",
                canonical_path.display()
            ))
        })?;

        let raw = fs::read_to_string(&canonical_path)?;
        let split = front_matter::split_front_matter(&raw)?;
        let yaml_segment = split.yaml.to_string();
        let body_segment = split.body.to_string();

        let mut yaml_value: Value = serde_yaml::from_str(&yaml_segment)
            .map_err(|err| SpecmanError::Serialization(err.to_string()))?;
        let typed_front = ArtifactFrontMatter::from_yaml_value(&yaml_value)?;
        let mapping = yaml_value
            .as_mapping_mut()
            .ok_or_else(|| SpecmanError::Template("front matter must be a YAML mapping".into()))?;
        let artifact_kind = artifact_kind_from_front(&typed_front);
        let artifact_name = infer_name(typed_front.name(), &canonical_path);
        let artifact = ArtifactId {
            kind: artifact_kind,
            name: artifact_name,
        };

        let context = MetadataContext {
            parent_dir: dir,
            workspace: &workspace_paths,
        };

        let mut mutated = false;
        if !request.add_dependencies.is_empty() {
            let handler = SpecificationMetadataHandler::new(&request.add_dependencies);
            mutated |= handler.apply(&artifact, mapping, &context)?;
        }

        if !request.add_references.is_empty() {
            let handler = ImplementationMetadataHandler::new(&request.add_references);
            mutated |= handler.apply(&artifact, mapping, &context)?;
        }

        let mut updated_document = raw;
        if mutated {
            let rendered_yaml = serde_yaml::to_string(&Value::Mapping(mapping.clone()))
                .map_err(|err| SpecmanError::Serialization(err.to_string()))?;
            updated_document = compose_document(rendered_yaml.trim_end(), &body_segment);
        }

        let mut persisted = None;
        if mutated && request.persist {
            fs::write(&canonical_path, &updated_document)?;
            let artifact_record = PersistedArtifact {
                artifact: artifact.clone(),
                path: canonical_path.clone(),
                workspace: workspace_paths.clone(),
            };
            if let Some(adapter) = &self.adapter {
                adapter.invalidate_dependency_tree(&artifact)?;
            }
            persisted = Some(artifact_record);
        }

        Ok(MetadataMutationResult {
            artifact,
            updated_document,
            persisted,
        })
    }
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

fn infer_name(name: Option<&str>, path: &Path) -> String {
    if let Some(name) = name {
        return name.to_string();
    }
    path.parent()
        .and_then(|dir| dir.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .or_else(|| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| path.display().to_string())
}

fn ensure_spec_kind(kind: &ArtifactKind) -> Result<(), SpecmanError> {
    if matches!(kind, ArtifactKind::Specification) {
        Ok(())
    } else {
        Err(SpecmanError::Template(
            "dependencies can only be added to specifications".into(),
        ))
    }
}

fn ensure_impl_kind(kind: &ArtifactKind) -> Result<(), SpecmanError> {
    if matches!(kind, ArtifactKind::Implementation) {
        Ok(())
    } else {
        Err(SpecmanError::Template(
            "references can only be added to implementations".into(),
        ))
    }
}

fn insert_dependency(mapping: &mut Mapping, locator: &str) -> Result<bool, SpecmanError> {
    let key = Value::String("dependencies".into());
    let entry = mapping
        .entry(key)
        .or_insert_with(|| Value::Sequence(Vec::new()));
    let seq = entry
        .as_sequence_mut()
        .ok_or_else(|| SpecmanError::Template("`dependencies` must be a sequence".into()))?;

    if seq.iter().any(|value| dependency_matches(value, locator)) {
        return Ok(false);
    }

    seq.push(Value::String(locator.to_string()));
    Ok(true)
}

fn insert_reference(
    mapping: &mut Mapping,
    addition: &ReferenceAddition,
) -> Result<bool, SpecmanError> {
    let key = Value::String("references".into());
    let entry = mapping
        .entry(key)
        .or_insert_with(|| Value::Sequence(Vec::new()));
    let seq = entry
        .as_sequence_mut()
        .ok_or_else(|| SpecmanError::Template("`references` must be a sequence".into()))?;

    if seq
        .iter()
        .any(|value| reference_matches(value, &addition.locator))
    {
        return Ok(false);
    }

    let mut map = Mapping::new();
    map.insert(
        Value::String("ref".into()),
        Value::String(addition.locator.clone()),
    );
    if let Some(reference_type) = &addition.reference_type {
        map.insert(
            Value::String("type".into()),
            Value::String(reference_type.clone()),
        );
    }
    if let Some(optional) = addition.optional {
        map.insert(Value::String("optional".into()), Value::Bool(optional));
    }

    seq.push(Value::Mapping(map));
    Ok(true)
}

fn dependency_matches(value: &Value, locator: &str) -> bool {
    match value {
        Value::String(existing) => existing == locator,
        Value::Mapping(map) => map
            .get(Value::String("ref".into()))
            .and_then(Value::as_str)
            .map(|value| value == locator)
            .unwrap_or(false),
        _ => false,
    }
}

fn reference_matches(value: &Value, locator: &str) -> bool {
    match value {
        Value::String(existing) => existing == locator,
        Value::Mapping(map) => map
            .get(Value::String("ref".into()))
            .and_then(Value::as_str)
            .map(|value| value == locator)
            .unwrap_or(false),
        _ => false,
    }
}

/// Parameters for metadata mutation operations.
#[derive(Debug, Default)]
pub struct MetadataMutationRequest {
    pub path: PathBuf,
    pub add_dependencies: Vec<String>,
    pub add_references: Vec<ReferenceAddition>,
    pub persist: bool,
}

impl MetadataMutationRequest {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            ..Default::default()
        }
    }

    pub fn persist(mut self, persist: bool) -> Self {
        self.persist = persist;
        self
    }
}

/// Reference metadata to add to an implementation artifact.
#[derive(Debug, Clone)]
pub struct ReferenceAddition {
    pub locator: String,
    pub reference_type: Option<String>,
    pub optional: Option<bool>,
}

impl ReferenceAddition {
    pub fn new(locator: impl Into<String>) -> Self {
        Self {
            locator: locator.into(),
            reference_type: None,
            optional: None,
        }
    }

    pub fn reference_type(mut self, reference_type: impl Into<String>) -> Self {
        self.reference_type = Some(reference_type.into());
        self
    }

    pub fn optional(mut self, optional: bool) -> Self {
        self.optional = Some(optional);
        self
    }
}

/// Result of a metadata mutation attempt.
#[derive(Debug)]
pub struct MetadataMutationResult {
    pub artifact: ArtifactId,
    pub updated_document: String,
    pub persisted: Option<PersistedArtifact>,
}

struct MetadataContext<'a> {
    parent_dir: &'a Path,
    workspace: &'a WorkspacePaths,
}

trait MetadataHandler {
    fn apply(
        &self,
        artifact: &ArtifactId,
        mapping: &mut Mapping,
        ctx: &MetadataContext,
    ) -> Result<bool, SpecmanError>;
}

struct SpecificationMetadataHandler<'a> {
    dependencies: &'a [String],
}

impl<'a> SpecificationMetadataHandler<'a> {
    fn new(dependencies: &'a [String]) -> Self {
        Self { dependencies }
    }
}

impl<'a> MetadataHandler for SpecificationMetadataHandler<'a> {
    fn apply(
        &self,
        artifact: &ArtifactId,
        mapping: &mut Mapping,
        ctx: &MetadataContext,
    ) -> Result<bool, SpecmanError> {
        ensure_spec_kind(&artifact.kind)?;
        let mut mutated = false;
        for dependency in self.dependencies {
            validate_workspace_reference(dependency, ctx.parent_dir, ctx.workspace)?;
            mutated |= insert_dependency(mapping, dependency)?;
        }
        Ok(mutated)
    }
}

struct ImplementationMetadataHandler<'a> {
    references: &'a [ReferenceAddition],
}

impl<'a> ImplementationMetadataHandler<'a> {
    fn new(references: &'a [ReferenceAddition]) -> Self {
        Self { references }
    }
}

impl<'a> MetadataHandler for ImplementationMetadataHandler<'a> {
    fn apply(
        &self,
        artifact: &ArtifactId,
        mapping: &mut Mapping,
        ctx: &MetadataContext,
    ) -> Result<bool, SpecmanError> {
        ensure_impl_kind(&artifact.kind)?;
        let mut mutated = false;
        for reference in self.references {
            validate_workspace_reference(&reference.locator, ctx.parent_dir, ctx.workspace)?;
            mutated |= insert_reference(mapping, reference)?;
        }
        Ok(mutated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::DataModelAdapter;
    use crate::dependency_tree::{ArtifactKind, DependencyTree};
    use crate::workspace::FilesystemWorkspaceLocator;
    use std::sync::Mutex;
    use tempfile::tempdir;

    #[test]
    fn mutate_spec_adds_dependency_and_persists() {
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

        let mutator = MetadataMutator::new(FilesystemWorkspaceLocator::new(&root));
        let request = MetadataMutationRequest {
            path: spec_path.canonicalize().unwrap(),
            add_dependencies: vec!["../data-model/spec.md".into()],
            add_references: Vec::new(),
            persist: true,
        };

        let result = mutator.mutate(request).expect("mutation succeeds");
        assert!(matches!(result.artifact.kind, ArtifactKind::Specification));
        assert!(
            result
                .persisted
                .as_ref()
                .map(|artifact| artifact.path.ends_with("spec/core/spec.md"))
                .unwrap_or(false)
        );

        let contents = fs::read_to_string(spec_path).unwrap();
        assert!(contents.contains("dependencies"));
        assert!(contents.contains("../data-model/spec.md"));
    }

    #[test]
    fn mutate_spec_accepts_resource_handle_dependency() {
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

        let mutator = MetadataMutator::new(FilesystemWorkspaceLocator::new(&root));
        let request = MetadataMutationRequest {
            path: spec_path.canonicalize().unwrap(),
            add_dependencies: vec!["spec://data-model".into()],
            add_references: Vec::new(),
            persist: false,
        };

        let result = mutator.mutate(request).expect("handle dependency accepted");
        assert!(result.updated_document.contains("spec://data-model"));
    }

    #[test]
    fn mutate_impl_adds_reference_without_duplicates() {
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

        let mutator = MetadataMutator::new(FilesystemWorkspaceLocator::new(&root));
        let request = MetadataMutationRequest {
            path: impl_path.canonicalize().unwrap(),
            add_dependencies: Vec::new(),
            add_references: vec![ReferenceAddition::new("../../spec/spec-beta/spec.md")],
            persist: true,
        };

        mutator.mutate(request).expect("first mutation");

        let duplicate_request = MetadataMutationRequest {
            path: impl_path.canonicalize().unwrap(),
            add_dependencies: Vec::new(),
            add_references: vec![ReferenceAddition::new("../../spec/spec-beta/spec.md")],
            persist: true,
        };

        let duplicate_result = mutator.mutate(duplicate_request).expect("second mutation");
        assert!(
            duplicate_result.persisted.is_none(),
            "no-op should skip persist"
        );

        let contents = fs::read_to_string(impl_path).unwrap();
        let count = contents.matches("../../spec/spec-beta/spec.md").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn mutate_rejects_http_references() {
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

        let mutator = MetadataMutator::new(FilesystemWorkspaceLocator::new(&root));
        let request = MetadataMutationRequest {
            path: spec_path.canonicalize().unwrap(),
            add_dependencies: vec!["http://example.com/spec.md".into()],
            add_references: Vec::new(),
            persist: false,
        };

        let err = mutator.mutate(request).expect_err("http refs rejected");
        assert!(matches!(err, SpecmanError::Dependency(_)));
    }

    #[test]
    fn mutate_notifies_adapter_on_persist() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec/base")).unwrap();
        fs::create_dir_all(root.join("spec/extra")).unwrap();

        let extra = root.join("spec/extra/spec.md");
        fs::write(&extra, "---\nname: extra\n---\n# Extra").unwrap();

        let base = root.join("spec/base/spec.md");
        fs::write(&base, "---\nname: base\n---\n# Base").unwrap();

        #[derive(Default)]
        struct RecordingAdapter {
            invalidated: Mutex<Vec<ArtifactId>>,
        }

        impl DataModelAdapter for RecordingAdapter {
            fn save_dependency_tree(&self, _tree: DependencyTree) -> Result<(), SpecmanError> {
                Ok(())
            }

            fn load_dependency_tree(
                &self,
                _root: &ArtifactId,
            ) -> Result<Option<DependencyTree>, SpecmanError> {
                Ok(None)
            }

            fn invalidate_dependency_tree(&self, root: &ArtifactId) -> Result<(), SpecmanError> {
                self.invalidated.lock().unwrap().push(root.clone());
                Ok(())
            }
        }

        let adapter = Arc::new(RecordingAdapter::default());
        let mutator =
            MetadataMutator::with_adapter(FilesystemWorkspaceLocator::new(&root), adapter.clone());

        let request = MetadataMutationRequest {
            path: base.canonicalize().unwrap(),
            add_dependencies: vec!["../extra/spec.md".into()],
            add_references: Vec::new(),
            persist: true,
        };

        mutator.mutate(request).expect("mutation succeeds");

        let entries = adapter.invalidated.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "base");
    }
}
