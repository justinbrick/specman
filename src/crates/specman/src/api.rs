use crate::error::SpecmanError;
use crate::template::{SpecContext, ImplContext, ScratchPadContext, TemplateScenario, TokenMap};
use crate::template_catalog::TemplateCatalog;
use crate::lifecycle::{CreationRequest, LifecycleController, DefaultLifecycleController};
use crate::dependency_tree::{ArtifactId, ArtifactKind};
use crate::persistence::WorkspacePersistence;
use crate::workspace::WorkspaceLocator;

/// Helper to create a specification using the default lifecycle controller.
pub fn create_specification<M, T, L>(
    controller: &DefaultLifecycleController<M, T>,
    catalog: &TemplateCatalog,
    persistence: &WorkspacePersistence<L>,
    context: &SpecContext,
) -> Result<(), SpecmanError>
where
    M: crate::dependency_tree::DependencyMapping,
    T: crate::template::TemplateEngine,
    L: WorkspaceLocator,
{
    let resolved = catalog.resolve(TemplateScenario::Specification)?;
    let artifact_id = ArtifactId {
        kind: ArtifactKind::Specification,
        name: context.name.clone(),
    };
    
    let mut tokens = TokenMap::new();
    tokens.insert("name".to_string(), serde_json::Value::String(context.name.clone()));
    tokens.insert("title".to_string(), serde_json::Value::String(context.title.clone()));

    let request = CreationRequest {
        target: artifact_id.clone(),
        template: resolved.descriptor,
        tokens,
        provenance: Some(resolved.provenance),
    };

    let plan = controller.plan_creation(request)?;
    persistence.persist(&artifact_id, &plan.rendered)?;
    Ok(())
}

/// Helper to create an implementation using the default lifecycle controller.
pub fn create_implementation<M, T, L>(
    controller: &DefaultLifecycleController<M, T>,
    catalog: &TemplateCatalog,
    persistence: &WorkspacePersistence<L>,
    context: &ImplContext,
) -> Result<(), SpecmanError>
where
    M: crate::dependency_tree::DependencyMapping,
    T: crate::template::TemplateEngine,
    L: WorkspaceLocator,
{
    let resolved = catalog.resolve(TemplateScenario::Implementation)?;
    let artifact_id = ArtifactId {
        kind: ArtifactKind::Implementation,
        name: context.name.clone(),
    };

    let mut tokens = TokenMap::new();
    tokens.insert("name".to_string(), serde_json::Value::String(context.name.clone()));
    tokens.insert("target".to_string(), serde_json::Value::String(context.target.clone()));

    let request = CreationRequest {
        target: artifact_id.clone(),
        template: resolved.descriptor,
        tokens,
        provenance: Some(resolved.provenance),
    };

    let plan = controller.plan_creation(request)?;
    persistence.persist(&artifact_id, &plan.rendered)?;
    Ok(())
}

/// Helper to create a scratch pad using the default lifecycle controller.
pub fn create_scratch_pad<M, T, L>(
    controller: &DefaultLifecycleController<M, T>,
    catalog: &TemplateCatalog,
    persistence: &WorkspacePersistence<L>,
    context: &ScratchPadContext,
) -> Result<(), SpecmanError>
where
    M: crate::dependency_tree::DependencyMapping,
    T: crate::template::TemplateEngine,
    L: WorkspaceLocator,
{
    let resolved = catalog.resolve(TemplateScenario::ScratchPad)?;
    let artifact_id = ArtifactId {
        kind: ArtifactKind::ScratchPad,
        name: context.name.clone(),
    };

    let mut tokens = TokenMap::new();
    tokens.insert("name".to_string(), serde_json::Value::String(context.name.clone()));
    tokens.insert("target".to_string(), serde_json::Value::String(context.target.clone()));
    tokens.insert("work_type".to_string(), serde_json::Value::String(context.work_type.clone()));

    let request = CreationRequest {
        target: artifact_id.clone(),
        template: resolved.descriptor,
        tokens,
        provenance: Some(resolved.provenance),
    };

    let plan = controller.plan_creation(request)?;
    persistence.persist(&artifact_id, &plan.rendered)?;
    Ok(())
}
