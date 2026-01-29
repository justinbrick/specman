use crate::dependency_tree::{
    ArtifactId, ArtifactKind, ArtifactSummary, DependencyTree,
    normalize_persisted_reference_for_create,
};
use crate::env::SpecmanEnv;
use crate::error::SpecmanError;
use crate::metadata::frontmatter::{
    ArtifactFrontMatter, ImplementationFrontMatter, ScratchFrontMatter, ScratchWorkType,
    SpecificationFrontMatter, split_front_matter,
};
use crate::persistence::PersistedArtifact;
use crate::template::{RenderedTemplate, TemplateScenario, TokenMap};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct CreateSpecOptions {
    pub name: String,
    pub title: String,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front_matter: Option<SpecificationFrontMatter>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateImplOptions {
    pub name: String,
    pub target: String,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front_matter: Option<ImplementationFrontMatter>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateScratchOptions {
    pub name: String,
    pub target: String,
    pub work_type: ScratchWorkType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front_matter: Option<ScratchFrontMatter>,
}

#[derive(Clone, Debug)]
pub enum CreateResult {
    Persisted(PersistedArtifact),
    DryRun(RenderedTemplate),
}

pub fn create_specification(
    env: &SpecmanEnv,
    opts: CreateSpecOptions,
) -> Result<CreateResult, SpecmanError> {
    let resolved = env.catalog.resolve(TemplateScenario::Specification)?;
    let artifact = ArtifactId {
        kind: ArtifactKind::Specification,
        name: opts.name.clone(),
    };

    let mut tokens = TokenMap::new();
    tokens.insert(
        "output_name".to_string(),
        serde_json::Value::String(opts.name.clone()),
    );
    tokens.insert(
        "title".to_string(),
        serde_json::Value::String(opts.title.clone()),
    );

    let workspace = env.persistence.workspace()?;
    let artifact_path = artifact_path_for(&artifact, &workspace);
    let desired = build_spec_create_front_matter(
        &artifact,
        &opts.name,
        &opts.title,
        opts.front_matter,
        &artifact_path,
        &workspace,
    )?;

    let mut rendered = env.templates.render(&resolved.descriptor, &tokens)?;
    rendered.provenance = Some(resolved.provenance);

    if let Some(front_matter) = desired {
        rendered.body = merge_front_matter_after_render(
            &rendered.body,
            &artifact_path,
            &workspace,
            &front_matter,
        )?;
    }

    if opts.dry_run {
        return Ok(CreateResult::DryRun(rendered));
    }

    let persisted = env.persistence.persist(&artifact, &rendered)?;

    // Compute and save dependency tree
    // We handle the error gracefully here or propagate it? 
    // Specman::create propagates it.
    let dependencies = match env.mapping.dependency_tree(&artifact) {
         Ok(tree) => tree,
         Err(SpecmanError::MissingTarget(_)) => DependencyTree::empty(ArtifactSummary {
             id: artifact.clone(),
             ..Default::default()
         }),
         Err(err) => return Err(err),
    };

    env.persistence.save_dependency_tree(&artifact, &dependencies)?;

    Ok(CreateResult::Persisted(persisted))
}

pub fn create_implementation(
    env: &SpecmanEnv,
    opts: CreateImplOptions,
) -> Result<CreateResult, SpecmanError> {
    let resolved = env.catalog.resolve(TemplateScenario::Implementation)?;
    let artifact = ArtifactId {
        kind: ArtifactKind::Implementation,
        name: opts.name.clone(),
    };

    let mut tokens = TokenMap::new();
    tokens.insert(
        "output_name".to_string(),
        serde_json::Value::String(opts.name.clone()),
    );
    tokens.insert(
        "target_path".to_string(),
        serde_json::Value::String(opts.target.clone()),
    );

    let workspace = env.persistence.workspace()?;
    let artifact_path = artifact_path_for(&artifact, &workspace);
    let desired = build_impl_create_front_matter(
        &artifact,
        &opts.name,
        &opts.target,
        opts.front_matter,
        &artifact_path,
        &workspace,
    )?;

    let mut rendered = env.templates.render(&resolved.descriptor, &tokens)?;
    rendered.provenance = Some(resolved.provenance);

    if let Some(front_matter) = desired {
        rendered.body = merge_front_matter_after_render(
            &rendered.body,
            &artifact_path,
            &workspace,
            &front_matter,
        )?;
    }

    if opts.dry_run {
        return Ok(CreateResult::DryRun(rendered));
    }

    let persisted = env.persistence.persist(&artifact, &rendered)?;

    let dependencies = match env.mapping.dependency_tree(&artifact) {
         Ok(tree) => tree,
         Err(SpecmanError::MissingTarget(_)) => DependencyTree::empty(ArtifactSummary {
             id: artifact.clone(),
             ..Default::default()
         }),
         Err(err) => return Err(err),
    };

    env.persistence.save_dependency_tree(&artifact, &dependencies)?;

    Ok(CreateResult::Persisted(persisted))
}

pub fn create_scratch_pad(
    env: &SpecmanEnv,
    opts: CreateScratchOptions,
) -> Result<CreateResult, SpecmanError> {
    let scenario =
        TemplateScenario::WorkType(opts.work_type.kind().as_str().to_string());
    let resolved = env.catalog.resolve(scenario)?;
    let artifact = ArtifactId {
        kind: ArtifactKind::ScratchPad,
        name: opts.name.clone(),
    };

    let mut tokens = TokenMap::new();
    tokens.insert(
        "output_name".to_string(),
        serde_json::Value::String(opts.name.clone()),
    );
    tokens.insert(
        "target_path".to_string(),
        serde_json::Value::String(opts.target.clone()),
    );
    tokens.insert(
        "work_type".to_string(),
        serde_json::Value::String(opts.work_type.kind().as_str().to_string()),
    );
    tokens.insert(
        "work_type_kind".to_string(),
        serde_json::Value::String(opts.work_type.kind().as_str().to_string()),
    );
    tokens.insert(
        "work_type_object".to_string(),
        serde_json::to_value(&opts.work_type)?,
    );

    let workspace = env.persistence.workspace()?;
    let artifact_path = artifact_path_for(&artifact, &workspace);
    let desired = build_scratch_create_front_matter(
        &artifact,
        &opts.name,
        &opts.target,
        &opts.work_type,
        opts.branch,
        opts.front_matter,
        &artifact_path,
        &workspace,
    )?;

    let mut rendered = env.templates.render(&resolved.descriptor, &tokens)?;
    rendered.provenance = Some(resolved.provenance);

    if let Some(front_matter) = desired {
        rendered.body = merge_front_matter_after_render(
            &rendered.body,
            &artifact_path,
            &workspace,
            &front_matter,
        )?;
    }

    if opts.dry_run {
        return Ok(CreateResult::DryRun(rendered));
    }

    let persisted = env.persistence.persist(&artifact, &rendered)?;

    let dependencies = match env.mapping.dependency_tree(&artifact) {
         Ok(tree) => tree,
         Err(SpecmanError::MissingTarget(_)) => DependencyTree::empty(ArtifactSummary {
             id: artifact.clone(),
             ..Default::default()
         }),
         Err(err) => return Err(err),
    };

    env.persistence.save_dependency_tree(&artifact, &dependencies)?;

    Ok(CreateResult::Persisted(persisted))
}

// Helpers copied/adapted from service.rs

fn artifact_path_for(
    artifact: &ArtifactId,
    workspace: &crate::workspace::WorkspacePaths,
) -> std::path::PathBuf {
    match artifact.kind {
        ArtifactKind::Specification => workspace.spec_dir().join(&artifact.name).join("spec.md"),
        ArtifactKind::Implementation => workspace.impl_dir().join(&artifact.name).join("impl.md"),
        ArtifactKind::ScratchPad => workspace
            .scratchpad_dir()
            .join(&artifact.name)
            .join("scratch.md"),
    }
}

fn merge_front_matter_after_render(
    rendered_body: &str,
    artifact_path: &std::path::Path,
    workspace: &crate::workspace::WorkspacePaths,
    desired: &serde_yaml::Mapping,
) -> Result<String, SpecmanError> {
    let parent = artifact_path.parent().ok_or_else(|| {
        SpecmanError::Workspace(format!(
            "artifact {} has no parent directory",
            artifact_path.display()
        ))
    })?;

    let desired_value = serde_yaml::Value::Mapping(desired.clone());
    let desired_typed = ArtifactFrontMatter::from_yaml_value(&desired_value)?;
    let normalized_mapping =
        normalize_create_front_matter_mapping(desired_typed, parent, workspace)?;

    match split_front_matter(rendered_body) {
        Ok(split) => {
            let mut existing: serde_yaml::Mapping =
                serde_yaml::from_str(split.yaml).map_err(|err| {
                    SpecmanError::Template(format!("invalid front matter YAML: {err}"))
                })?;

            for (k, v) in normalized_mapping {
                existing.insert(k, v);
            }

            let yaml = serialize_yaml_value(&serde_yaml::Value::Mapping(existing))?;
            Ok(compose_document(&yaml, split.body))
        }
        Err(_) => {
            let yaml = serialize_yaml_value(&serde_yaml::Value::Mapping(normalized_mapping))?;
            Ok(compose_document(&yaml, rendered_body))
        }
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

fn serialize_yaml_value(value: &serde_yaml::Value) -> Result<String, SpecmanError> {
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

fn normalize_create_front_matter_mapping(
    front: ArtifactFrontMatter,
    parent: &std::path::Path,
    workspace: &crate::workspace::WorkspacePaths,
) -> Result<serde_yaml::Mapping, SpecmanError> {
    match front {
        ArtifactFrontMatter::Specification(mut fm) => {
            for dep in fm.dependencies.iter_mut() {
                match dep {
                    crate::metadata::frontmatter::DependencyEntry::Simple(s) => {
                        *s = normalize_persisted_reference_for_create(s, parent, workspace)?;
                    }
                    crate::metadata::frontmatter::DependencyEntry::Detailed(obj) => {
                        obj.reference = normalize_persisted_reference_for_create(
                            &obj.reference,
                            parent,
                            workspace,
                        )?;
                    }
                }
            }
            let value = serde_yaml::to_value(&fm)
                .map_err(|err| SpecmanError::Serialization(err.to_string()))?;
            Ok(value
                .as_mapping()
                .cloned()
                .unwrap_or_else(serde_yaml::Mapping::new))
        }
        ArtifactFrontMatter::Implementation(mut fm) => {
            if let Some(spec) = &fm.spec {
                fm.spec = Some(normalize_persisted_reference_for_create(
                    spec, parent, workspace,
                )?);
            }
            for dep in fm.dependencies.iter_mut() {
                match dep {
                    crate::metadata::frontmatter::DependencyEntry::Simple(s) => {
                        *s = normalize_persisted_reference_for_create(s, parent, workspace)?;
                    }
                    crate::metadata::frontmatter::DependencyEntry::Detailed(obj) => {
                        obj.reference = normalize_persisted_reference_for_create(
                            &obj.reference,
                            parent,
                            workspace,
                        )?;
                    }
                }
            }
            for r in fm.references.iter_mut() {
                r.reference =
                    normalize_persisted_reference_for_create(&r.reference, parent, workspace)?;
            }
            let value = serde_yaml::to_value(&fm)
                .map_err(|err| SpecmanError::Serialization(err.to_string()))?;
            Ok(value
                .as_mapping()
                .cloned()
                .unwrap_or_else(serde_yaml::Mapping::new))
        }
        ArtifactFrontMatter::Scratch(mut fm) => {
            if let Some(target) = &fm.target {
                fm.target = Some(normalize_persisted_reference_for_create(
                    target,
                    workspace.root(),
                    workspace,
                )?);
            }
            for dep in fm.dependencies.iter_mut() {
                match dep {
                    crate::metadata::frontmatter::DependencyEntry::Simple(s) => {
                        *s = normalize_persisted_reference_for_create(
                            s,
                            workspace.root(),
                            workspace,
                        )?;
                    }
                    crate::metadata::frontmatter::DependencyEntry::Detailed(obj) => {
                        obj.reference = normalize_persisted_reference_for_create(
                            &obj.reference,
                            workspace.root(),
                            workspace,
                        )?;
                    }
                }
            }
            let value = serde_yaml::to_value(&fm)
                .map_err(|err| SpecmanError::Serialization(err.to_string()))?;
            Ok(value
                .as_mapping()
                .cloned()
                .unwrap_or_else(serde_yaml::Mapping::new))
        }
    }
}

fn build_spec_create_front_matter(
    _artifact: &ArtifactId,
    name: &str,
    title: &str,
    input: Option<SpecificationFrontMatter>,
    artifact_path: &std::path::Path,
    workspace: &crate::workspace::WorkspacePaths,
) -> Result<Option<serde_yaml::Mapping>, SpecmanError> {
    let mut fm = input.unwrap_or_default();
    if fm.identity.name.is_none() {
        fm.identity.name = Some(name.to_string());
    }
    if fm.identity.title.is_none() {
        fm.identity.title = Some(title.to_string());
    }
    let value =
        serde_yaml::to_value(&fm).map_err(|err| SpecmanError::Serialization(err.to_string()))?;
    Ok(Some(normalize_create_front_matter_mapping(
        ArtifactFrontMatter::from_yaml_value(&value)?,
        artifact_path
            .parent()
            .ok_or_else(|| SpecmanError::Workspace("missing parent".into()))?,
        workspace,
    )?))
}

fn build_impl_create_front_matter(
    _artifact: &ArtifactId,
    name: &str,
    target: &str,
    input: Option<ImplementationFrontMatter>,
    artifact_path: &std::path::Path,
    workspace: &crate::workspace::WorkspacePaths,
) -> Result<Option<serde_yaml::Mapping>, SpecmanError> {
    let mut fm = input.unwrap_or_default();
    if fm.identity.name.is_none() {
        fm.identity.name = Some(name.to_string());
    }
    if fm.spec.is_none() {
        fm.spec = Some(target.to_string());
    }
    let value =
        serde_yaml::to_value(&fm).map_err(|err| SpecmanError::Serialization(err.to_string()))?;
    Ok(Some(normalize_create_front_matter_mapping(
        ArtifactFrontMatter::from_yaml_value(&value)?,
        artifact_path
            .parent()
            .ok_or_else(|| SpecmanError::Workspace("missing parent".into()))?,
        workspace,
    )?))
}

fn build_scratch_create_front_matter(
    _artifact: &ArtifactId,
    name: &str,
    target: &str,
    work_type: &ScratchWorkType,
    branch: Option<String>,
    input: Option<ScratchFrontMatter>,
    artifact_path: &std::path::Path,
    workspace: &crate::workspace::WorkspacePaths,
) -> Result<Option<serde_yaml::Mapping>, SpecmanError> {
    let mut fm = input.unwrap_or_default();
    if fm.identity.name.is_none() {
        fm.identity.name = Some(name.to_string());
    }
    if fm.target.is_none() {
        fm.target = Some(target.to_string());
    }
    if fm.work_type.is_none() {
        fm.work_type = Some(work_type.clone());
    }
    if fm.branch.is_none() {
        fm.branch = branch;
    }
    let value =
        serde_yaml::to_value(&fm).map_err(|err| SpecmanError::Serialization(err.to_string()))?;
    Ok(Some(normalize_create_front_matter_mapping(
        ArtifactFrontMatter::from_yaml_value(&value)?,
        artifact_path
            .parent()
            .ok_or_else(|| SpecmanError::Workspace("missing parent".into()))?,
        workspace,
    )?))
}
