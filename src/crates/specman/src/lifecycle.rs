use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::adapter::DataModelAdapter;
use crate::dependency_tree::{ArtifactId, DependencyMapping, DependencyTree};
use crate::error::SpecmanError;
use crate::scratchpad::ScratchPadProfile;
use crate::template::{RenderedTemplate, TemplateDescriptor, TemplateEngine, TokenMap};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreationRequest {
    pub target: ArtifactId,
    pub template: TemplateDescriptor,
    pub tokens: TokenMap,
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
}

pub struct DefaultLifecycleController<M, T, A>
where
    M: DependencyMapping,
    T: TemplateEngine,
    A: DataModelAdapter,
{
    mapping: M,
    templates: T,
    adapter: A,
}

impl<M, T, A> DefaultLifecycleController<M, T, A>
where
    M: DependencyMapping,
    T: TemplateEngine,
    A: DataModelAdapter,
{
    pub fn new(mapping: M, templates: T, adapter: A) -> Self {
        Self {
            mapping,
            templates,
            adapter,
        }
    }
}

impl<M, T, A> LifecycleController for DefaultLifecycleController<M, T, A>
where
    M: DependencyMapping,
    T: TemplateEngine,
    A: DataModelAdapter,
{
    fn plan_creation(&self, request: CreationRequest) -> Result<CreationPlan, SpecmanError> {
        let dependencies = self.mapping.dependency_tree(&request.target)?;
        let rendered = self.templates.render(&request.template, &request.tokens)?;
        self.adapter.save_dependency_tree(dependencies.clone())?;
        Ok(CreationPlan {
            rendered,
            dependencies,
        })
    }

    fn plan_deletion(&self, target: ArtifactId) -> Result<DeletionPlan, SpecmanError> {
        let dependencies = self.mapping.dependency_tree(&target)?;
        let blocked = !dependencies.downstream.is_empty();
        Ok(DeletionPlan {
            dependencies,
            blocked,
        })
    }

    fn plan_scratchpad(&self, profile: ScratchPadProfile) -> Result<ScratchPadPlan, SpecmanError> {
        let rendered = self
            .templates
            .render(&profile.template, &profile.token_map())?;
        Ok(ScratchPadPlan { rendered, profile })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::DataModelAdapter;
    use crate::dependency_tree::{ArtifactKind, ArtifactSummary, DependencyEdge};
    use crate::persistence::WorkspacePersistence;
    use crate::scratchpad::ScratchPadProfile;
    use crate::template::{TemplateScenario, TokenMap};
    use crate::workspace::FilesystemWorkspaceLocator;
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::Mutex;
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

    #[derive(Default)]
    struct RecordingAdapter {
        saved: Mutex<Vec<DependencyTree>>,
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
            })
        }
    }

    fn controller() -> DefaultLifecycleController<MockMapping, FakeTemplateEngine, RecordingAdapter>
    {
        DefaultLifecycleController::new(
            MockMapping,
            FakeTemplateEngine::default(),
            RecordingAdapter::default(),
        )
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
            name: "workspace-template-persist".into(),
            template: TemplateDescriptor {
                scenario: TemplateScenario::ScratchPad,
                ..Default::default()
            },
            configuration: BTreeMap::new(),
        };
        let profile_name = profile.name.clone();

        let plan = controller.plan_scratchpad(profile).expect("scratch plan");
        let persistence = WorkspacePersistence::new(FilesystemWorkspaceLocator::new(start.clone()));
        let artifact = ArtifactId {
            kind: ArtifactKind::ScratchPad,
            name: profile_name,
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
}
