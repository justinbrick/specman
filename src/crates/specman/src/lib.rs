pub mod adapter;
pub mod api;
pub mod dependency_tree;
pub mod error;
pub mod front_matter;
pub mod lifecycle;
pub mod metadata;
pub mod persistence;
pub mod scratchpad;
pub mod shared_function;
pub mod template;
pub mod template_catalog;
pub mod workspace;

pub use api::{create_implementation, create_scratch_pad, create_specification};

pub use adapter::{DataModelAdapter, InMemoryAdapter};
pub use dependency_tree::{
    ArtifactId, ArtifactKind, ArtifactSummary, DependencyEdge, DependencyGraphServices,
    DependencyMapping, DependencyRelation, DependencyTree, FilesystemDependencyMapper,
    InventoryDependent, WorkspaceInventorySnapshot,
};
pub use error::SpecmanError;
pub use lifecycle::{
    CreationPlan, CreationRequest, DefaultLifecycleController, DeletionPlan, LifecycleController,
    ScratchPadPlan,
};
pub use metadata::{
    MetadataMutationRequest, MetadataMutationResult, MetadataMutator, ReferenceAddition,
};
pub use persistence::{
    ArtifactRemovalStore, PersistedArtifact, RemovedArtifact, WorkspacePersistence,
};
pub use scratchpad::ScratchPadProfile;
pub use shared_function::{EntityKind, SchemaRef, SemVer};
pub use template::{
    ImplContext, MarkdownTemplateEngine, RenderedTemplate, ScratchPadContext, SpecContext,
    TemplateDescriptor, TemplateEngine, TemplateLocator, TemplateProvenance, TemplateScenario,
    TemplateTier, TokenMap,
};
pub use template_catalog::{ResolvedTemplate, TemplateCatalog};
pub use workspace::{
    FilesystemWorkspaceLocator, WorkspaceLocator, WorkspacePaths, discover as discover_workspace,
};
