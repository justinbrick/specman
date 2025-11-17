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
