pub mod adapter;
pub mod dependency_tree;
pub mod error;
pub mod front_matter;
pub mod lifecycle;
pub mod metadata;
pub mod persistence;
pub mod scratchpad;
pub mod shared_function;
pub mod template;
pub mod workspace;

pub use adapter::{DataModelAdapter, InMemoryAdapter};
pub use dependency_tree::{
    ArtifactId, ArtifactKind, ArtifactSummary, DependencyEdge, DependencyMapping,
    DependencyRelation, DependencyTree, FilesystemDependencyMapper,
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
    MarkdownTemplateEngine, RenderedTemplate, TemplateDescriptor, TemplateEngine, TemplateLocator,
    TemplateScenario, TokenMap,
};
pub use workspace::{
    FilesystemWorkspaceLocator, WorkspaceLocator, WorkspacePaths, discover as discover_workspace,
};
