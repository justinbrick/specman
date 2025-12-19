use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::dependency_tree::{ArtifactId, ArtifactSummary, DependencyMapping, DependencyTree};
use crate::error::{LifecycleError, SpecmanError};
use crate::persistence::{ArtifactRemovalStore, RemovedArtifact};
use crate::scratchpad::ScratchPadProfile;
use crate::template::{
    RenderedTemplate, TemplateDescriptor, TemplateEngine, TemplateProvenance, TokenMap,
};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreationRequest {
    pub target: ArtifactId,
    pub template: TemplateDescriptor,
    pub tokens: TokenMap,
    pub provenance: Option<TemplateProvenance>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreationPlan {
    pub rendered: RenderedTemplate,
    pub dependencies: DependencyTree,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeletionPlan {
    pub dependencies: DependencyTree,
    pub blocked: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScratchPadPlan {
    pub rendered: RenderedTemplate,
    pub profile: ScratchPadProfile,
}

pub trait LifecycleController: Send + Sync {
    fn plan_creation(&self, request: CreationRequest) -> Result<CreationPlan, SpecmanError>;
    fn plan_deletion(&self, target: ArtifactId) -> Result<DeletionPlan, SpecmanError>;
    fn plan_scratchpad(&self, profile: ScratchPadProfile) -> Result<ScratchPadPlan, SpecmanError>;
    /// Executes a deletion by validating (or recomputing) the plan, ensuring the artifact is
    /// unblocked, invoking workspace persistence to remove the artifact directory, and
    /// invalidating cached dependency graphs.
    fn execute_deletion(
        &self,
        target: ArtifactId,
        existing_plan: Option<DeletionPlan>,
        persistence: &dyn ArtifactRemovalStore,
        force: bool,
    ) -> Result<RemovedArtifact, SpecmanError>;
}

pub struct DefaultLifecycleController<M, T>
where
    M: DependencyMapping,
    T: TemplateEngine,
{
    mapping: M,
    templates: T,
}

impl<M, T> DefaultLifecycleController<M, T>
where
    M: DependencyMapping,
    T: TemplateEngine,
{
    pub fn new(mapping: M, templates: T) -> Self {
        Self { mapping, templates }
    }
}

impl<M, T> LifecycleController for DefaultLifecycleController<M, T>
where
    M: DependencyMapping,
    T: TemplateEngine,
{
    fn plan_creation(&self, request: CreationRequest) -> Result<CreationPlan, SpecmanError> {
        let mut rendered = self.templates.render(&request.template, &request.tokens)?;
        rendered.provenance = request.provenance.clone();

        let dependencies = match self.mapping.dependency_tree(&request.target) {
            Ok(tree) => tree,
            Err(SpecmanError::MissingTarget(_)) => DependencyTree::empty(ArtifactSummary {
                id: request.target.clone(),
                ..Default::default()
            }),
            Err(err) => return Err(err),
        };
        Ok(CreationPlan {
            rendered,
            dependencies,
        })
    }

    fn plan_deletion(&self, target: ArtifactId) -> Result<DeletionPlan, SpecmanError> {
        let dependencies = self.mapping.dependency_tree(&target)?;
        let blocked = dependencies.has_blocking_dependents();
        Ok(DeletionPlan {
            dependencies,
            blocked,
        })
    }

    fn plan_scratchpad(&self, profile: ScratchPadProfile) -> Result<ScratchPadPlan, SpecmanError> {
        let mut rendered = self
            .templates
            .render(&profile.template, &profile.token_map())?;
        rendered.provenance = profile.provenance.clone();
        Ok(ScratchPadPlan { rendered, profile })
    }

    fn execute_deletion(
        &self,
        target: ArtifactId,
        existing_plan: Option<DeletionPlan>,
        persistence: &dyn ArtifactRemovalStore,
        force: bool,
    ) -> Result<RemovedArtifact, SpecmanError> {
        let plan = match existing_plan {
            Some(plan) => {
                if plan.dependencies.root.id != target {
                    return Err(LifecycleError::PlanTargetMismatch {
                        requested: target,
                        planned: plan.dependencies.root.id,
                    }
                    .into());
                }
                plan
            }
            None => self.plan_deletion(target.clone())?,
        };

        if plan.blocked && !force {
            return Err(LifecycleError::DeletionBlocked { target }.into());
        }

        let removed = persistence.remove_artifact(&target)?;
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::DataModelAdapter;
    use crate::dependency_tree::{
        ArtifactKind, ArtifactSummary, DependencyEdge, DependencyRelation,
    };
    use crate::persistence::WorkspacePersistence;
    use crate::scratchpad::{ScratchPadProfile, ScratchPadProfileKind};
    use crate::template::{TemplateScenario, TokenMap};
    use crate::workspace::FilesystemWorkspaceLocator;
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    #[derive(Clone)]
    struct MockMapping;

    impl DependencyMapping for MockMapping {
        fn dependency_tree(&self, root: &ArtifactId) -> Result<DependencyTree, SpecmanError> {
            Ok(DependencyTree {
                root: ArtifactSummary {
                    id: root.clone(),
                    ..Default::default()
                },
                ..Default::default()
            })
        }

        fn upstream(&self, _root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError> {
            Ok(Vec::new())
        }

        fn downstream(&self, _root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError> {
            Ok(Vec::new())
        }
    }

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

    #[derive(Clone, Default)]
    struct RecordingAdapter {
        saved: Arc<Mutex<Vec<DependencyTree>>>,
        invalidated: Arc<Mutex<Vec<ArtifactId>>>,
    }

    impl DataModelAdapter for RecordingAdapter {
        fn save_dependency_tree(&self, tree: DependencyTree) -> Result<(), SpecmanError> {
            self.saved.lock().unwrap().push(tree);
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

    impl RecordingAdapter {
        fn invalidated_ids(&self) -> Vec<ArtifactId> {
            self.invalidated.lock().unwrap().clone()
        }
    }

    #[derive(Clone, Default)]
    struct FakeTemplateEngine;

    impl TemplateEngine for FakeTemplateEngine {
        fn render(
            &self,
            descriptor: &TemplateDescriptor,
            _tokens: &TokenMap,
        ) -> Result<RenderedTemplate, SpecmanError> {
            Ok(RenderedTemplate {
                body: format!("# artifact\nscenario: {:?}\n", descriptor.scenario),
                metadata: descriptor.clone(),
                provenance: None,
            })
        }
    }

    fn controller() -> DefaultLifecycleController<MockMapping, FakeTemplateEngine> {
        DefaultLifecycleController::new(MockMapping, FakeTemplateEngine)
    }

    #[test]
    fn lifecycle_creation_flow_persists_implementation() {
        let temp = tempdir().unwrap();
        let workspace_root = temp.path().join("ws");
        fs::create_dir_all(workspace_root.join(".specman")).unwrap();
        let start = workspace_root.join("impl");
        fs::create_dir_all(&start).unwrap();

        let controller = controller();
        let artifact = ArtifactId {
            kind: ArtifactKind::Implementation,
            name: "specman-library".into(),
        };
        let request = CreationRequest {
            target: artifact.clone(),
            template: TemplateDescriptor {
                scenario: TemplateScenario::Implementation,
                ..Default::default()
            },
            tokens: TokenMap::new(),
            provenance: None,
        };

        let plan = controller.plan_creation(request).expect("creation plan");
        let persistence = WorkspacePersistence::new(FilesystemWorkspaceLocator::new(start.clone()));
        let persisted = persistence
            .persist(&artifact, &plan.rendered)
            .expect("persist implementation");

        assert!(
            persisted
                .path
                .ends_with(std::path::Path::new("impl/specman-library/impl.md"))
        );
        let contents = fs::read_to_string(persisted.path).unwrap();
        assert!(contents.contains("scenario"));
    }

    #[test]
    fn lifecycle_scratchpad_flow_persists_artifact() {
        let temp = tempdir().unwrap();
        let workspace_root = temp.path().join("ws");
        let dot_specman = workspace_root.join(".specman");
        fs::create_dir_all(dot_specman.join("scratchpad")).unwrap();
        let start = dot_specman.join("scratchpad");

        let controller = controller();
        let profile = ScratchPadProfile {
            kind: ScratchPadProfileKind::Ref,
            name: "workspace-template-persist".into(),
            template: TemplateDescriptor {
                scenario: TemplateScenario::ScratchPad,
                ..Default::default()
            },
            provenance: None,
            configuration: BTreeMap::new(),
        };
        let profile_slug = profile.slug().to_string();

        let plan = controller.plan_scratchpad(profile).expect("scratch plan");
        let persistence = WorkspacePersistence::new(FilesystemWorkspaceLocator::new(start.clone()));
        let artifact = ArtifactId {
            kind: ArtifactKind::ScratchPad,
            name: profile_slug,
        };

        let persisted = persistence
            .persist(&artifact, &plan.rendered)
            .expect("persist scratchpad");
        assert!(persisted.path.ends_with(std::path::Path::new(
            ".specman/scratchpad/workspace-template-persist/scratch.md"
        )));
        let contents = fs::read_to_string(persisted.path).unwrap();
        assert!(contents.contains("scenario"));
    }

    #[test]
    fn lifecycle_deletion_executes_and_invalidates_cache() {
        let temp = tempdir().unwrap();
        let workspace_root = temp.path().join("ws");
        let dot_specman = workspace_root.join(".specman");
        fs::create_dir_all(dot_specman.join("scratchpad")).unwrap();
        let impl_dir = workspace_root.join("impl");
        fs::create_dir_all(&impl_dir).unwrap();

        let artifact = ArtifactId {
            kind: ArtifactKind::Implementation,
            name: "specman-library".into(),
        };
        let artifact_dir = impl_dir.join(&artifact.name);
        fs::create_dir_all(&artifact_dir).unwrap();
        fs::write(artifact_dir.join("impl.md"), "body").unwrap();

        // On Windows, path representations can vary (e.g. `\\?\` prefix and 8.3 short names).
        // Canonicalize while it still exists so we can reliably compare after deletion.
        let artifact_dir_canonical = fs::canonicalize(&artifact_dir).unwrap();

        let controller = controller();
        let adapter = Arc::new(RecordingAdapter::default());
        let adapter_handle: Arc<dyn DataModelAdapter> = adapter.clone();
        let persistence = WorkspacePersistence::with_adapter(
            FilesystemWorkspaceLocator::new(impl_dir.clone()),
            adapter_handle,
        );
        let plan = controller
            .plan_deletion(artifact.clone())
            .expect("deletion plan");
        assert!(!plan.blocked);

        let removed = controller
            .execute_deletion(artifact.clone(), Some(plan), &persistence, false)
            .expect("execute deletion");

        assert_eq!(removed.artifact, artifact);
        assert_eq!(removed.directory, artifact_dir_canonical);
        assert!(!removed.directory.exists());
        let invalidated = adapter.invalidated_ids();
        assert_eq!(invalidated, vec![artifact]);
    }

    #[test]
    fn lifecycle_deletion_blocks_when_plan_marked_blocked() {
        let temp = tempdir().unwrap();
        let workspace_root = temp.path().join("ws");
        let dot_specman = workspace_root.join(".specman");
        fs::create_dir_all(dot_specman.join("scratchpad")).unwrap();
        let impl_dir = workspace_root.join("impl");
        fs::create_dir_all(&impl_dir).unwrap();

        let controller = controller();
        let adapter = Arc::new(RecordingAdapter::default());
        let adapter_handle: Arc<dyn DataModelAdapter> = adapter.clone();
        let persistence = WorkspacePersistence::with_adapter(
            FilesystemWorkspaceLocator::new(impl_dir),
            adapter_handle,
        );
        let artifact = ArtifactId {
            kind: ArtifactKind::Specification,
            name: "has-dependents".into(),
        };

        let plan = DeletionPlan {
            dependencies: DependencyTree::empty(ArtifactSummary {
                id: artifact.clone(),
                ..Default::default()
            }),
            blocked: true,
        };

        let err = controller
            .execute_deletion(artifact.clone(), Some(plan), &persistence, false)
            .expect_err("blocked deletion");
        assert!(matches!(
            err,
            SpecmanError::Lifecycle(crate::error::LifecycleError::DeletionBlocked { .. })
        ));
        assert!(adapter.invalidated_ids().is_empty());
    }

    #[test]
    fn lifecycle_force_override_allows_blocked_deletion() {
        let temp = tempdir().unwrap();
        let workspace_root = temp.path().join("ws");
        let dot_specman = workspace_root.join(".specman");
        fs::create_dir_all(dot_specman.join("scratchpad")).unwrap();
        let impl_dir = workspace_root.join("impl");
        fs::create_dir_all(&impl_dir).unwrap();

        let controller = controller();
        let adapter = Arc::new(RecordingAdapter::default());
        let adapter_handle: Arc<dyn DataModelAdapter> = adapter.clone();
        let persistence = WorkspacePersistence::with_adapter(
            FilesystemWorkspaceLocator::new(impl_dir.clone()),
            adapter_handle,
        );
        let artifact = ArtifactId {
            kind: ArtifactKind::Implementation,
            name: "force-delete".into(),
        };
        let artifact_dir = impl_dir.join(&artifact.name);
        fs::create_dir_all(&artifact_dir).unwrap();
        fs::write(artifact_dir.join("impl.md"), "body").unwrap();

        let plan = DeletionPlan {
            dependencies: DependencyTree::empty(ArtifactSummary {
                id: artifact.clone(),
                ..Default::default()
            }),
            blocked: true,
        };

        let removed = controller
            .execute_deletion(artifact.clone(), Some(plan), &persistence, true)
            .expect("forced deletion succeeds");

        assert_eq!(removed.artifact, artifact);
        assert!(!artifact_dir.exists());
        let invalidated = adapter.invalidated_ids();
        assert_eq!(invalidated, vec![artifact]);
    }

    #[test]
    fn scratchpad_deletion_ignores_target_edges() {
        let root_artifact = ArtifactId {
            kind: ArtifactKind::ScratchPad,
            name: "notes".into(),
        };
        let root_summary = ArtifactSummary {
            id: root_artifact.clone(),
            ..Default::default()
        };
        let target_summary = ArtifactSummary {
            id: ArtifactId {
                kind: ArtifactKind::Implementation,
                name: "impl-target".into(),
            },
            ..Default::default()
        };
        let mut tree = DependencyTree::empty(root_summary.clone());
        tree.downstream.push(DependencyEdge {
            from: target_summary,
            to: root_summary.clone(),
            relation: DependencyRelation::Downstream,
            optional: false,
        });

        let controller = DefaultLifecycleController::new(
            StaticTreeMapping { tree: tree.clone() },
            FakeTemplateEngine,
        );

        let plan = controller
            .plan_deletion(root_artifact.clone())
            .expect("plan deletion");
        assert!(!plan.blocked, "scratch pad should ignore target edge");
    }

    #[test]
    fn scratchpad_deletion_blocks_when_dependent_scratchpads_exist() {
        let root_artifact = ArtifactId {
            kind: ArtifactKind::ScratchPad,
            name: "notes".into(),
        };
        let root_summary = ArtifactSummary {
            id: root_artifact.clone(),
            ..Default::default()
        };
        let dependent_summary = ArtifactSummary {
            id: ArtifactId {
                kind: ArtifactKind::ScratchPad,
                name: "dependent".into(),
            },
            ..Default::default()
        };
        let mut tree = DependencyTree::empty(root_summary.clone());
        tree.downstream.push(DependencyEdge {
            from: dependent_summary,
            to: root_summary.clone(),
            relation: DependencyRelation::Downstream,
            optional: false,
        });

        let controller = DefaultLifecycleController::new(
            StaticTreeMapping { tree: tree.clone() },
            FakeTemplateEngine,
        );

        let plan = controller
            .plan_deletion(root_artifact.clone())
            .expect("plan deletion");
        assert!(plan.blocked, "dependent scratch pad should block deletion");
    }

    #[test]
    fn implementations_still_block_on_any_downstream_edges() {
        let root_artifact = ArtifactId {
            kind: ArtifactKind::Implementation,
            name: "impl".into(),
        };
        let root_summary = ArtifactSummary {
            id: root_artifact.clone(),
            ..Default::default()
        };
        let dependent_summary = ArtifactSummary {
            id: ArtifactId {
                kind: ArtifactKind::Specification,
                name: "spec".into(),
            },
            ..Default::default()
        };
        let mut tree = DependencyTree::empty(root_summary.clone());
        tree.downstream.push(DependencyEdge {
            from: dependent_summary,
            to: root_summary.clone(),
            relation: DependencyRelation::Downstream,
            optional: false,
        });

        let controller = DefaultLifecycleController::new(
            StaticTreeMapping { tree: tree.clone() },
            FakeTemplateEngine,
        );

        let plan = controller
            .plan_deletion(root_artifact.clone())
            .expect("plan deletion");
        assert!(
            plan.blocked,
            "non-scratch artifacts should block on downstream edges"
        );
    }
}
