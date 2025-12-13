use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_yaml::{Mapping, Value};

use crate::adapter::DataModelAdapter;
use crate::dependency_tree::{ArtifactId, ArtifactKind, validate_workspace_reference};
use crate::error::SpecmanError;
use crate::front_matter::{self, ArtifactFrontMatter, FrontMatterKind};
use crate::persistence::PersistedArtifact;
use crate::workspace::{WorkspaceLocator, WorkspacePaths};

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
