use crate::dependency_tree::normalize_persisted_reference_for_create;
use crate::dependency_tree::{
    ArtifactId, ArtifactKind, DependencyMapping, DependencyTree, FilesystemDependencyMapper,
};
use crate::error::{LifecycleError, SpecmanError};
use crate::front_matter::{
    ArtifactFrontMatter, ImplementationFrontMatter, ScratchFrontMatter, ScratchWorkType,
    SpecificationFrontMatter, split_front_matter,
};
use crate::lifecycle::{
    CreationRequest, DefaultLifecycleController, DeletionPlan, LifecycleController,
};
use crate::metadata::{
    FrontMatterUpdateRequest, FrontMatterUpdateResult, apply_front_matter_update,
};
use crate::persistence::{PersistedArtifact, RemovedArtifact, WorkspacePersistence};
use crate::template::{
    ImplContext, RenderedTemplate, SpecContext, TemplateDescriptor, TemplateEngine,
    TemplateProvenance, TemplateScenario, TokenMap,
};
use crate::template_catalog::TemplateCatalog;
use crate::workspace::{FilesystemWorkspaceLocator, WorkspaceLocator};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// High-level façade that bundles lifecycle planning/execution, template selection,
/// and workspace persistence into a single ergonomic entry point.
pub struct Specman<M, T, L>
where
    M: DependencyMapping,
    T: TemplateEngine,
    L: WorkspaceLocator,
{
    controller: DefaultLifecycleController<M, T>,
    catalog: TemplateCatalog,
    persistence: WorkspacePersistence<L>,
}

impl<M, T, L> Specman<M, T, L>
where
    M: DependencyMapping,
    T: TemplateEngine,
    L: WorkspaceLocator,
{
    pub fn new(
        controller: DefaultLifecycleController<M, T>,
        catalog: TemplateCatalog,
        persistence: WorkspacePersistence<L>,
    ) -> Self {
        Self {
            controller,
            catalog,
            persistence,
        }
    }

    pub fn catalog(&self) -> &TemplateCatalog {
        &self.catalog
    }

    pub fn persistence(&self) -> &WorkspacePersistence<L> {
        &self.persistence
    }

    pub fn controller(&self) -> &DefaultLifecycleController<M, T> {
        &self.controller
    }

    /// Side-effect free: template resolution + rendering.
    pub fn plan_create(&self, req: CreateRequest) -> Result<CreatePlan, SpecmanError> {
        let workspace = self.persistence.workspace()?;

        let (artifact, request, front_matter) = self.to_creation_request(req, &workspace)?;

        let plan = self.controller.plan_creation(request)?;
        let mut rendered = plan.rendered;

        if let Some(front_matter) = front_matter {
            let artifact_path = artifact_path_for(&artifact, &workspace);
            rendered.body = merge_front_matter_after_render(
                &rendered.body,
                &artifact_path,
                &workspace,
                &front_matter,
            )?;
        }

        Ok(CreatePlan { artifact, rendered })
    }

    /// Side-effect free: dependency mapping + deletion blocking computation.
    pub fn plan_delete(&self, target: ArtifactId) -> Result<DeletePlan, SpecmanError> {
        let plan = self.controller.plan_deletion(target.clone())?;
        Ok(DeletePlan {
            target,
            dependencies: plan.dependencies,
            blocked: plan.blocked,
        })
    }

    /// Convenience: plan + persist + dependency-tree registration (when adapter configured).
    pub fn create(&self, req: CreateRequest) -> Result<PersistedArtifact, SpecmanError> {
        let plan = self.plan_create(req)?;
        let persisted = self.persistence.persist(&plan.artifact, &plan.rendered)?;

        // Dependency trees can only be computed once the artifact exists on disk.
        let deps = self
            .controller
            .plan_deletion(plan.artifact.clone())?
            .dependencies;
        self.persistence
            .save_dependency_tree(&plan.artifact, &deps)?;

        Ok(persisted)
    }

    /// Mutates an existing artifact's YAML front matter without rewriting the Markdown body.
    pub fn update(
        &self,
        target: ArtifactId,
        update: FrontMatterUpdateRequest,
    ) -> Result<FrontMatterUpdateResult, SpecmanError> {
        let workspace = self.persistence.workspace()?;
        let path = self.persistence.artifact_path(&target)?;
        let raw = std::fs::read_to_string(&path)?;

        let (updated_document, mutated) =
            apply_front_matter_update(&target, &path, &workspace, &raw, &update)?;

        let persisted = if mutated && update.persist {
            Some(
                self.persistence
                    .persist_document(&target, &updated_document)?,
            )
        } else {
            None
        };

        Ok(FrontMatterUpdateResult {
            artifact: target,
            updated_document,
            persisted,
        })
    }

    /// Deletion with optional precomputed plan.
    pub fn delete(&self, req: DeleteRequest) -> Result<RemovedArtifact, SpecmanError> {
        let force = req.policy.force;
        let target = req.target;

        let plan = match req.plan {
            Some(plan) => {
                if plan.target != target {
                    return Err(LifecycleError::PlanTargetMismatch {
                        requested: target,
                        planned: plan.target,
                    }
                    .into());
                }
                plan
            }
            None => self.plan_delete(target.clone())?,
        };

        if plan.blocked && !force {
            return Err(LifecycleError::DeletionBlocked { target }.into());
        }

        let existing = Some(DeletionPlan {
            dependencies: plan.dependencies,
            blocked: plan.blocked,
        });

        self.controller
            .execute_deletion(target, existing, &self.persistence, force)
    }

    /// Ergonomic adapter for flows that render/modify Markdown before persisting.
    pub fn persist_rendered(
        &self,
        artifact: &ArtifactId,
        rendered: &RenderedTemplate,
        dependencies: Option<&DependencyTree>,
    ) -> Result<PersistedArtifact, SpecmanError> {
        match dependencies {
            Some(tree) => self
                .persistence
                .persist_with_dependency_tree(artifact, rendered, tree),
            None => self.persistence.persist(artifact, rendered),
        }
    }

    fn to_creation_request(
        &self,
        req: CreateRequest,
        workspace: &crate::workspace::WorkspacePaths,
    ) -> Result<(ArtifactId, CreationRequest, Option<serde_yaml::Mapping>), SpecmanError> {
        match req {
            CreateRequest::Specification {
                context,
                front_matter,
            } => {
                let resolved = self.catalog.resolve(TemplateScenario::Specification)?;
                let artifact = ArtifactId {
                    kind: ArtifactKind::Specification,
                    name: context.name.clone(),
                };

                let mut tokens = TokenMap::new();
                tokens.insert(
                    "output_name".to_string(),
                    serde_json::Value::String(context.name.clone()),
                );
                tokens.insert(
                    "title".to_string(),
                    serde_json::Value::String(context.title.clone()),
                );

                let artifact_path = artifact_path_for(&artifact, workspace);
                let desired = build_spec_create_front_matter(
                    &artifact,
                    &context,
                    front_matter,
                    &artifact_path,
                    workspace,
                )?;

                Ok((
                    artifact.clone(),
                    CreationRequest {
                        target: artifact,
                        template: resolved.descriptor,
                        tokens,
                        provenance: Some(resolved.provenance),
                    },
                    desired,
                ))
            }
            CreateRequest::Implementation {
                context,
                front_matter,
            } => {
                let resolved = self.catalog.resolve(TemplateScenario::Implementation)?;
                let artifact = ArtifactId {
                    kind: ArtifactKind::Implementation,
                    name: context.name.clone(),
                };

                let mut tokens = TokenMap::new();
                tokens.insert(
                    "output_name".to_string(),
                    serde_json::Value::String(context.name.clone()),
                );
                tokens.insert(
                    "target_path".to_string(),
                    serde_json::Value::String(context.target.clone()),
                );

                let artifact_path = artifact_path_for(&artifact, workspace);
                let desired = build_impl_create_front_matter(
                    &artifact,
                    &context,
                    front_matter,
                    &artifact_path,
                    workspace,
                )?;

                Ok((
                    artifact.clone(),
                    CreationRequest {
                        target: artifact,
                        template: resolved.descriptor,
                        tokens,
                        provenance: Some(resolved.provenance),
                    },
                    desired,
                ))
            }
            CreateRequest::ScratchPad {
                context,
                front_matter,
            } => {
                let scenario =
                    TemplateScenario::WorkType(context.work_type.kind().as_str().to_string());
                let resolved = self.catalog.resolve(scenario)?;
                let artifact = ArtifactId {
                    kind: ArtifactKind::ScratchPad,
                    name: context.name.clone(),
                };

                let mut tokens = TokenMap::new();
                tokens.insert(
                    "output_name".to_string(),
                    serde_json::Value::String(context.name.clone()),
                );
                tokens.insert(
                    "target_path".to_string(),
                    serde_json::Value::String(context.target.clone()),
                );
                tokens.insert(
                    "work_type".to_string(),
                    serde_json::Value::String(context.work_type.kind().as_str().to_string()),
                );
                tokens.insert(
                    "work_type_kind".to_string(),
                    serde_json::Value::String(context.work_type.kind().as_str().to_string()),
                );
                tokens.insert(
                    "work_type_object".to_string(),
                    serde_json::to_value(&context.work_type)?,
                );

                let artifact_path = artifact_path_for(&artifact, workspace);
                let desired = build_scratch_create_front_matter(
                    &artifact,
                    &context,
                    front_matter,
                    &artifact_path,
                    workspace,
                )?;

                Ok((
                    artifact.clone(),
                    CreationRequest {
                        target: artifact,
                        template: resolved.descriptor,
                        tokens,
                        provenance: Some(resolved.provenance),
                    },
                    desired,
                ))
            }
            CreateRequest::Custom {
                artifact,
                template,
                tokens,
                provenance,
            } => Ok((
                artifact.clone(),
                CreationRequest {
                    target: artifact,
                    template,
                    tokens,
                    provenance,
                },
                None,
            )),
        }
    }
}

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

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum CreateRequest {
    Specification {
        context: SpecContext,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        front_matter: Option<SpecificationFrontMatter>,
    },
    Implementation {
        context: ImplContext,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        front_matter: Option<ImplementationFrontMatter>,
    },
    ScratchPad {
        context: ScratchPadCreateContext,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        front_matter: Option<ScratchFrontMatter>,
    },

    /// Escape hatch for advanced hosts.
    Custom {
        artifact: ArtifactId,
        template: TemplateDescriptor,
        tokens: TokenMap,
        provenance: Option<TemplateProvenance>,
    },
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

    // Normalize any locators embedded in the desired mapping by round-tripping through
    // the typed front matter parser, which encodes the data-model shapes.
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
                    crate::front_matter::DependencyEntry::Simple(s) => {
                        *s = normalize_persisted_reference_for_create(s, parent, workspace)?;
                    }
                    crate::front_matter::DependencyEntry::Detailed(obj) => {
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
                    crate::front_matter::DependencyEntry::Simple(s) => {
                        *s = normalize_persisted_reference_for_create(s, parent, workspace)?;
                    }
                    crate::front_matter::DependencyEntry::Detailed(obj) => {
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
                    crate::front_matter::DependencyEntry::Simple(s) => {
                        *s = normalize_persisted_reference_for_create(
                            s,
                            workspace.root(),
                            workspace,
                        )?;
                    }
                    crate::front_matter::DependencyEntry::Detailed(obj) => {
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
    artifact: &ArtifactId,
    context: &SpecContext,
    input: Option<SpecificationFrontMatter>,
    artifact_path: &std::path::Path,
    workspace: &crate::workspace::WorkspacePaths,
) -> Result<Option<serde_yaml::Mapping>, SpecmanError> {
    let mut fm = input.unwrap_or_default();
    if fm.identity.name.is_none() {
        fm.identity.name = Some(artifact.name.clone());
    }
    if fm.identity.title.is_none() {
        fm.identity.title = Some(context.title.clone());
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
    artifact: &ArtifactId,
    context: &ImplContext,
    input: Option<ImplementationFrontMatter>,
    artifact_path: &std::path::Path,
    workspace: &crate::workspace::WorkspacePaths,
) -> Result<Option<serde_yaml::Mapping>, SpecmanError> {
    let mut fm = input.unwrap_or_default();
    if fm.identity.name.is_none() {
        fm.identity.name = Some(artifact.name.clone());
    }
    if fm.spec.is_none() {
        fm.spec = Some(context.target.clone());
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
    artifact: &ArtifactId,
    context: &ScratchPadCreateContext,
    input: Option<ScratchFrontMatter>,
    artifact_path: &std::path::Path,
    workspace: &crate::workspace::WorkspacePaths,
) -> Result<Option<serde_yaml::Mapping>, SpecmanError> {
    let mut fm = input.unwrap_or_default();
    if fm.identity.name.is_none() {
        fm.identity.name = Some(artifact.name.clone());
    }
    if fm.target.is_none() {
        fm.target = Some(context.target.clone());
    }
    if fm.work_type.is_none() {
        fm.work_type = Some(context.work_type.clone());
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

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScratchPadCreateContext {
    pub name: String,
    pub target: String,
    pub work_type: ScratchWorkType,
}

#[derive(Clone, Debug)]
pub struct CreatePlan {
    pub artifact: ArtifactId,
    pub rendered: RenderedTemplate,
}

#[derive(Clone, Debug, Default)]
pub struct DeletePolicy {
    pub force: bool,
}

#[derive(Clone, Debug)]
pub struct DeleteRequest {
    pub target: ArtifactId,
    pub plan: Option<DeletePlan>,
    pub policy: DeletePolicy,
}

#[derive(Clone, Debug)]
pub struct DeletePlan {
    pub target: ArtifactId,
    pub dependencies: DependencyTree,
    pub blocked: bool,
}

pub type DefaultWorkspaceLocator = std::sync::Arc<FilesystemWorkspaceLocator>;

pub type DefaultDependencyMapping = FilesystemDependencyMapper<DefaultWorkspaceLocator>;

pub type DefaultTemplateEngine = crate::template::MarkdownTemplateEngine;

pub type DefaultSpecman =
    Specman<DefaultDependencyMapping, DefaultTemplateEngine, DefaultWorkspaceLocator>;

impl DefaultSpecman {
    pub fn from_current_dir() -> Result<Self, SpecmanError> {
        let locator = std::sync::Arc::new(FilesystemWorkspaceLocator::from_current_dir()?);
        let workspace = locator.workspace()?;

        let mapper = FilesystemDependencyMapper::new(locator.clone());
        let inventory = mapper.inventory_handle();

        let templates = crate::template::MarkdownTemplateEngine::new();
        let controller = DefaultLifecycleController::new(mapper, templates);

        let catalog = TemplateCatalog::new(workspace);
        let persistence = WorkspacePersistence::with_inventory(locator, inventory);

        Ok(Self::new(controller, catalog, persistence))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dependency_tree::{
        ArtifactSummary, DependencyEdge, DependencyRelation, FilesystemDependencyMapper,
    };
    use crate::front_matter::ScratchFixMetadata;
    use crate::front_matter::{ArtifactIdentityFields, SpecificationFrontMatter};
    use crate::metadata::FrontMatterUpdateOp;
    use crate::template::{MarkdownTemplateEngine, RenderedTemplate, TemplateDescriptor, TokenMap};
    use crate::workspace::FilesystemWorkspaceLocator;
    use std::fs;
    use tempfile::tempdir;

    #[derive(Clone)]
    struct StaticTreeMapping {
        tree: DependencyTree,
    }

    impl DependencyMapping for StaticTreeMapping {
        fn dependency_tree(&self, _root: &ArtifactId) -> Result<DependencyTree, SpecmanError> {
            Ok(self.tree.clone())
        }

        fn upstream(&self, _root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError> {
            Ok(self.tree.upstream.clone())
        }

        fn downstream(&self, _root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError> {
            Ok(self.tree.downstream.clone())
        }
    }

    #[derive(Default)]
    struct UnusedTemplateEngine;

    impl TemplateEngine for UnusedTemplateEngine {
        fn render(
            &self,
            _descriptor: &TemplateDescriptor,
            _tokens: &TokenMap,
        ) -> Result<RenderedTemplate, SpecmanError> {
            Err(SpecmanError::Template("unused".into()))
        }
    }

    fn workspace_fixture() -> (
        tempfile::TempDir,
        FilesystemWorkspaceLocator,
        TemplateCatalog,
    ) {
        let temp = tempdir().unwrap();
        let root = temp.path().join("ws");
        fs::create_dir_all(root.join(".specman")).unwrap();
        fs::create_dir_all(root.join("spec")).unwrap();
        fs::create_dir_all(root.join("impl")).unwrap();
        let start = root.join("impl");
        let locator = FilesystemWorkspaceLocator::new(start);
        let workspace = locator.workspace().unwrap();
        let catalog = TemplateCatalog::new(workspace);
        (temp, locator, catalog)
    }

    #[test]
    fn delete_returns_structured_plan_mismatch_error() {
        let (_temp, locator, catalog) = workspace_fixture();
        let mapping = StaticTreeMapping {
            tree: DependencyTree::empty(ArtifactSummary {
                id: ArtifactId {
                    kind: ArtifactKind::Implementation,
                    name: "ignored".into(),
                },
                ..Default::default()
            }),
        };
        let controller = DefaultLifecycleController::new(mapping, UnusedTemplateEngine);
        let persistence = WorkspacePersistence::new(locator);
        let svc = Specman::new(controller, catalog, persistence);

        let requested = ArtifactId {
            kind: ArtifactKind::Implementation,
            name: "a".into(),
        };
        let planned = ArtifactId {
            kind: ArtifactKind::Implementation,
            name: "b".into(),
        };

        let err = svc
            .delete(DeleteRequest {
                target: requested.clone(),
                plan: Some(DeletePlan {
                    target: planned.clone(),
                    dependencies: DependencyTree::empty(ArtifactSummary {
                        id: planned.clone(),
                        ..Default::default()
                    }),
                    blocked: false,
                }),
                policy: DeletePolicy { force: false },
            })
            .expect_err("expected plan mismatch error");

        match err {
            SpecmanError::Lifecycle(LifecycleError::PlanTargetMismatch {
                requested: r,
                planned: p,
            }) => {
                assert_eq!(r, requested);
                assert_eq!(p, planned);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn delete_returns_structured_blocked_deletion_error() {
        let (_temp, locator, catalog) = workspace_fixture();
        let target = ArtifactId {
            kind: ArtifactKind::Implementation,
            name: "blocked".into(),
        };
        let root_summary = ArtifactSummary {
            id: target.clone(),
            ..Default::default()
        };
        let dependent = ArtifactSummary {
            id: ArtifactId {
                kind: ArtifactKind::Specification,
                name: "downstream".into(),
            },
            ..Default::default()
        };
        let mut tree = DependencyTree::empty(root_summary.clone());
        tree.downstream.push(DependencyEdge {
            from: dependent,
            to: root_summary,
            relation: DependencyRelation::Downstream,
            optional: false,
        });

        let mapping = StaticTreeMapping { tree };
        let controller = DefaultLifecycleController::new(mapping, UnusedTemplateEngine);
        let persistence = WorkspacePersistence::new(locator);
        let svc = Specman::new(controller, catalog, persistence);

        let err = svc
            .delete(DeleteRequest {
                target: target.clone(),
                plan: None,
                policy: DeletePolicy { force: false },
            })
            .expect_err("expected blocked deletion error");

        match err {
            SpecmanError::Lifecycle(LifecycleError::DeletionBlocked { target: t }) => {
                assert_eq!(t, target);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn create_scratchpad_does_not_fail_when_dependency_tree_computed_post_create() {
        let (_temp, locator, catalog) = workspace_fixture();
        let workspace = locator.workspace().unwrap();
        let workspace_root = workspace.root().to_path_buf();

        // Create a minimal target implementation (and its spec) so dependency mapping succeeds.
        fs::create_dir_all(workspace.spec_dir().join("specman-core")).unwrap();
        fs::write(
            workspace.spec_dir().join("specman-core/spec.md"),
            "---\nname: specman-core\nversion: \"1.0.0\"\n---\n# Core\n",
        )
        .unwrap();

        fs::create_dir_all(workspace.impl_dir().join("specman-library")).unwrap();
        fs::write(
            workspace.impl_dir().join("specman-library/impl.md"),
            "---\nspec: ../../spec/specman-core/spec.md\nname: specman-library\nversion: \"0.1.0\"\nreferences: []\n---\n# Impl\n",
        )
        .unwrap();

        let mapping_locator = FilesystemWorkspaceLocator::new(workspace_root.join("impl"));
        let mapping = FilesystemDependencyMapper::new(mapping_locator);
        let templates = MarkdownTemplateEngine::new();
        let controller = DefaultLifecycleController::new(mapping, templates);
        let persistence_locator = FilesystemWorkspaceLocator::new(workspace_root.join("impl"));
        let persistence = WorkspacePersistence::new(persistence_locator);
        let svc = Specman::new(controller, catalog, persistence);

        let persisted = svc
            .create(CreateRequest::ScratchPad {
                context: ScratchPadCreateContext {
                    name: "post-create-deps".into(),
                    target: "impl://specman-library".into(),
                    work_type: ScratchWorkType::Fix(ScratchFixMetadata {
                        fixed_headings: Vec::new(),
                        extras: Default::default(),
                    }),
                },
                front_matter: None,
            })
            .expect("scratch pad create should succeed");

        assert!(persisted.path.exists());
    }

    #[test]
    fn create_implementation_does_not_fail_when_dependency_tree_computed_post_create() {
        let (_temp, locator, catalog) = workspace_fixture();
        let workspace = locator.workspace().unwrap();
        let workspace_root = workspace.root().to_path_buf();

        // Seed the workspace with a spec so the implementation's `spec:` reference resolves.
        fs::create_dir_all(workspace.spec_dir().join("specman-core")).unwrap();
        fs::write(
            workspace.spec_dir().join("specman-core/spec.md"),
            "---\nname: specman-core\nversion: \"1.0.0\"\n---\n# Core\n",
        )
        .unwrap();

        let mapping_locator = FilesystemWorkspaceLocator::new(workspace_root.join("impl"));
        let mapping = FilesystemDependencyMapper::new(mapping_locator);
        let templates = MarkdownTemplateEngine::new();
        let controller = DefaultLifecycleController::new(mapping, templates);
        let persistence_locator: FilesystemWorkspaceLocator =
            FilesystemWorkspaceLocator::new(workspace_root.join("impl"));
        let persistence = WorkspacePersistence::new(persistence_locator);
        let svc = Specman::new(controller, catalog, persistence);

        let persisted = svc
            .create(CreateRequest::Implementation {
                context: ImplContext {
                    name: "impl-post-create-deps".into(),
                    target: "spec://specman-core".into(),
                },
                front_matter: None,
            })
            .expect("implementation create should succeed");

        assert!(persisted.path.exists());
    }

    #[test]
    fn create_specification_does_not_fail_when_dependency_tree_computed_post_create() {
        let (_temp, locator, catalog) = workspace_fixture();
        let workspace = locator.workspace().unwrap();
        let workspace_root = workspace.root().to_path_buf();

        let mapping_locator = FilesystemWorkspaceLocator::new(workspace_root.join("impl"));
        let mapping = FilesystemDependencyMapper::new(mapping_locator);
        let templates = MarkdownTemplateEngine::new();
        let controller = DefaultLifecycleController::new(mapping, templates);
        let persistence_locator = FilesystemWorkspaceLocator::new(workspace_root.join("impl"));
        let persistence = WorkspacePersistence::new(persistence_locator);
        let svc = Specman::new(controller, catalog, persistence);

        let persisted = svc
            .create(CreateRequest::Specification {
                context: SpecContext {
                    name: "spec-post-create-deps".into(),
                    title: "Spec Post Create Deps".into(),
                },
                front_matter: None,
            })
            .expect("spec create should succeed");

        assert!(persisted.path.exists());
    }

    #[derive(Clone)]
    struct PassthroughMapping;

    impl DependencyMapping for PassthroughMapping {
        fn dependency_tree(&self, root: &ArtifactId) -> Result<DependencyTree, SpecmanError> {
            Ok(DependencyTree::empty(ArtifactSummary {
                id: root.clone(),
                ..Default::default()
            }))
        }

        fn upstream(&self, _root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError> {
            Ok(Vec::new())
        }

        fn downstream(&self, _root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn update_preserves_body_and_normalizes_handles() {
        let (_temp, locator, catalog) = workspace_fixture();
        let workspace = locator.workspace().unwrap();
        let root = workspace.root().to_path_buf();

        fs::create_dir_all(workspace.spec_dir().join("specman-core")).unwrap();
        fs::write(
            workspace.spec_dir().join("specman-core/spec.md"),
            "---\nname: specman-core\nversion: \"1.0.0\"\n---\n# Core\n",
        )
        .unwrap();

        fs::create_dir_all(workspace.impl_dir().join("demo-impl")).unwrap();
        let impl_path = workspace.impl_dir().join("demo-impl/impl.md");
        fs::write(
            &impl_path,
            "---\nname: demo-impl\nspec: ../../spec/specman-core/spec.md\nreferences: []\n---\n# Body\nKeep this body.\n",
        )
        .unwrap();

        let mapping_locator = FilesystemWorkspaceLocator::new(root.join("impl"));
        let mapping = FilesystemDependencyMapper::new(mapping_locator);
        let templates = MarkdownTemplateEngine::new();
        let controller = DefaultLifecycleController::new(mapping, templates);
        let persistence_locator = FilesystemWorkspaceLocator::new(root.join("impl"));
        let persistence = WorkspacePersistence::new(persistence_locator);
        let svc = Specman::new(controller, catalog, persistence);

        let result = svc
            .update(
                ArtifactId {
                    kind: ArtifactKind::Implementation,
                    name: "demo-impl".into(),
                },
                FrontMatterUpdateRequest::new().with_op(FrontMatterUpdateOp::SetSpec {
                    ref_: "spec://specman-core".into(),
                }),
            )
            .expect("update should succeed");

        let before_raw = fs::read_to_string(&impl_path).unwrap();
        let before = crate::front_matter::split_front_matter(&before_raw).unwrap();
        let after = crate::front_matter::split_front_matter(&result.updated_document).unwrap();
        assert_eq!(before.body, after.body);
        assert!(after.yaml.contains("spec: ../../spec/specman-core/spec.md"));
        assert!(!after.yaml.contains("spec://"));
    }

    #[test]
    fn update_synthesizes_front_matter_when_missing() {
        let (_temp, locator, catalog) = workspace_fixture();
        let workspace = locator.workspace().unwrap();
        let root = workspace.root().to_path_buf();

        fs::create_dir_all(workspace.spec_dir().join("no-front")).unwrap();
        let spec_path = workspace.spec_dir().join("no-front/spec.md");
        fs::write(&spec_path, "# Body Only\n\nHello.\n").unwrap();

        let mapping_locator = FilesystemWorkspaceLocator::new(root.join("impl"));
        let mapping = FilesystemDependencyMapper::new(mapping_locator);
        let templates = MarkdownTemplateEngine::new();
        let controller = DefaultLifecycleController::new(mapping, templates);
        let persistence_locator = FilesystemWorkspaceLocator::new(root.join("impl"));
        let persistence = WorkspacePersistence::new(persistence_locator);
        let svc = Specman::new(controller, catalog, persistence);

        let result = svc
            .update(
                ArtifactId {
                    kind: ArtifactKind::Specification,
                    name: "no-front".into(),
                },
                FrontMatterUpdateRequest::new()
                    .with_op(FrontMatterUpdateOp::AddTag { tag: "demo".into() }),
            )
            .expect("update should succeed");

        let split = crate::front_matter::split_front_matter(&result.updated_document).unwrap();
        assert!(split.yaml.contains("tags:"));
        assert!(split.body.contains("# Body Only"));
    }

    #[test]
    fn create_with_front_matter_persists_in_first_write() {
        let (_temp, locator, catalog) = workspace_fixture();
        let workspace = locator.workspace().unwrap();
        let root = workspace.root().to_path_buf();

        let controller =
            DefaultLifecycleController::new(PassthroughMapping, MarkdownTemplateEngine::new());
        let persistence_locator = FilesystemWorkspaceLocator::new(root.join("impl"));
        let persistence = WorkspacePersistence::new(persistence_locator);
        let svc = Specman::new(controller, catalog, persistence);

        let front = SpecificationFrontMatter {
            identity: ArtifactIdentityFields {
                title: Some("Custom Title".into()),
                description: Some("Custom Description".into()),
                ..Default::default()
            },
            ..Default::default()
        };

        let persisted = svc
            .create(CreateRequest::Specification {
                context: SpecContext {
                    name: "spec-frontmatter".into(),
                    title: "Ignored".into(),
                },
                front_matter: Some(front),
            })
            .expect("create should succeed");

        let raw = fs::read_to_string(&persisted.path).unwrap();
        let split = crate::front_matter::split_front_matter(&raw).unwrap();
        assert!(split.yaml.contains("title: Custom Title"));
        assert!(split.yaml.contains("description: Custom Description"));
    }
}
