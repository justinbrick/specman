use crate::dependency_tree::{ArtifactId, ArtifactKind, DependencyMapping, DependencyTree, FilesystemDependencyMapper};
use crate::error::{LifecycleError, SpecmanError};
use crate::front_matter::ScratchWorkType;
use crate::lifecycle::{
    CreationRequest, DefaultLifecycleController, DeletionPlan, LifecycleController,
};
use crate::persistence::{PersistedArtifact, RemovedArtifact, WorkspacePersistence};
use crate::template::{
    ImplContext, RenderedTemplate, SpecContext, TemplateDescriptor, TemplateEngine,
    TemplateProvenance, TemplateScenario, TokenMap,
};
use crate::template_catalog::TemplateCatalog;
use crate::workspace::{FilesystemWorkspaceLocator, WorkspaceLocator};

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
        let (artifact, request) = self.to_creation_request(req)?;
        let plan = self.controller.plan_creation(request)?;
        Ok(CreatePlan {
            artifact,
            rendered: plan.rendered,
        })
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
        let deps = self.controller.plan_deletion(plan.artifact.clone())?.dependencies;
        self.persistence.save_dependency_tree(&plan.artifact, &deps)?;

        Ok(persisted)
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
    ) -> Result<(ArtifactId, CreationRequest), SpecmanError> {
        match req {
            CreateRequest::Specification { context } => {
                let resolved = self.catalog.resolve(TemplateScenario::Specification)?;
                let artifact = ArtifactId {
                    kind: ArtifactKind::Specification,
                    name: context.name.clone(),
                };

                let mut tokens = TokenMap::new();
                tokens.insert(
                    "name".to_string(),
                    serde_json::Value::String(context.name.clone()),
                );
                tokens.insert(
                    "title".to_string(),
                    serde_json::Value::String(context.title.clone()),
                );

                Ok((
                    artifact.clone(),
                    CreationRequest {
                        target: artifact,
                        template: resolved.descriptor,
                        tokens,
                        provenance: Some(resolved.provenance),
                    },
                ))
            }
            CreateRequest::Implementation { context } => {
                let resolved = self.catalog.resolve(TemplateScenario::Implementation)?;
                let artifact = ArtifactId {
                    kind: ArtifactKind::Implementation,
                    name: context.name.clone(),
                };

                let mut tokens = TokenMap::new();
                tokens.insert(
                    "name".to_string(),
                    serde_json::Value::String(context.name.clone()),
                );
                tokens.insert(
                    "target".to_string(),
                    serde_json::Value::String(context.target.clone()),
                );

                Ok((
                    artifact.clone(),
                    CreationRequest {
                        target: artifact,
                        template: resolved.descriptor,
                        tokens,
                        provenance: Some(resolved.provenance),
                    },
                ))
            }
            CreateRequest::ScratchPad { context } => {
                let scenario =
                    TemplateScenario::WorkType(context.work_type.kind().as_str().to_string());
                let resolved = self.catalog.resolve(scenario)?;
                let artifact = ArtifactId {
                    kind: ArtifactKind::ScratchPad,
                    name: context.name.clone(),
                };

                let mut tokens = TokenMap::new();
                tokens.insert(
                    "name".to_string(),
                    serde_json::Value::String(context.name.clone()),
                );
                tokens.insert(
                    "target".to_string(),
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

                Ok((
                    artifact.clone(),
                    CreationRequest {
                        target: artifact,
                        template: resolved.descriptor,
                        tokens,
                        provenance: Some(resolved.provenance),
                    },
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
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub enum CreateRequest {
    Specification {
        context: SpecContext,
    },
    Implementation {
        context: ImplContext,
    },
    ScratchPad {
        context: ScratchPadCreateContext,
    },

    /// Escape hatch for advanced hosts.
    Custom {
        artifact: ArtifactId,
        template: TemplateDescriptor,
        tokens: TokenMap,
        provenance: Option<TemplateProvenance>,
    },
}

#[derive(Clone, Debug)]
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
    use crate::dependency_tree::{ArtifactSummary, DependencyEdge, DependencyRelation};
    use crate::template::{RenderedTemplate, TemplateDescriptor, TokenMap};
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
}
