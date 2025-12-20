pub mod adapter;
pub mod dependency_tree;
pub mod error;
pub mod front_matter;
pub mod lifecycle;
pub mod metadata;
pub mod persistence;
pub mod reference_validation;
pub mod scratchpad;
pub mod service;
pub mod shared_function;
pub mod structure;
pub mod template;
pub mod template_catalog;
pub mod workspace;

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
    FrontMatterUpdateOp, FrontMatterUpdateRequest, FrontMatterUpdateResult,
    MetadataMutationRequest, MetadataMutationResult, MetadataMutator, ReferenceAddition,
    apply_front_matter_update,
};
pub use persistence::{
    ArtifactRemovalStore, PersistedArtifact, RemovedArtifact, WorkspacePersistence,
};
pub use reference_validation::{
    DestinationKind, DiscoveredReference, HttpsMethod, HttpsValidationMode, HttpsValidationOptions,
    IssueSeverity, ReferenceSource, ReferenceValidationIssue, ReferenceValidationOptions,
    ReferenceValidationReport, ReferenceValidationStatus, SourcePoint, SourceRange,
    TransitiveOptions, validate_references,
};
pub use scratchpad::ScratchPadProfile;
pub use service::{
    CreatePlan, CreateRequest, DefaultSpecman, DeletePlan, DeletePolicy, DeleteRequest,
    ScratchPadCreateContext, Specman,
};
pub use shared_function::{EntityKind, SchemaRef, SemVer};
pub use structure::{
    ArtifactKey, ArtifactRecord, ConstraintIdentifier, ConstraintRecord,
    FilesystemStructureIndexer, HeadingIdentifier, HeadingRecord, RelationshipEdge,
    RelationshipKind, StructureIndexing, StructureQuery, WorkspaceIndex,
};
pub use template::{
    ImplContext, MarkdownTemplateEngine, RenderedTemplate, ScratchPadContext, SpecContext,
    TemplateDescriptor, TemplateEngine, TemplateLocator, TemplateProvenance, TemplateScenario,
    TemplateTier, TokenMap,
};
pub use template_catalog::{ResolvedTemplate, TemplateCatalog};
pub use workspace::{
    FilesystemWorkspaceLocator, WorkspaceLocator, WorkspacePaths, discover as discover_workspace,
};
