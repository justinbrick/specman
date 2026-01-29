pub mod adapter;
pub mod analysis;
pub mod dependency_tree;
pub mod env;
pub mod error;
pub mod metadata;
pub mod ops;
pub mod persistence;
pub mod reference_validation;
pub mod scratchpad;
pub mod shared_function;
pub mod structure;
pub mod template;
pub mod template_catalog;
pub mod validation;
pub mod workspace;

pub use adapter::{DataModelAdapter, InMemoryAdapter};
pub use analysis::{DeletionImpact, check_deletion_impact};
pub use dependency_tree::{
    ArtifactId, ArtifactKind, ArtifactSummary, DependencyEdge, DependencyGraphServices,
    DependencyMapping, DependencyRelation, DependencyTree, FilesystemDependencyMapper,
    InventoryDependent, WorkspaceInventorySnapshot,
};
pub use env::SpecmanEnv;
pub use error::SpecmanError;
pub use metadata::{
    FrontMatterUpdate, FrontMatterUpdateResult, IdentityUpdate, ImplementationUpdate,
    ScratchUpdate, SpecificationUpdate,
};
pub use ops::update::apply_front_matter_update;
pub use persistence::{
    ArtifactRemovalStore, PersistedArtifact, RemovedArtifact, WorkspacePersistence,
};
pub use reference_validation::{
    DestinationKind, DiscoveredReference, HttpsMethod, HttpsValidationMode, HttpsValidationOptions,
    IssueSeverity, ReachabilityPolicy, ReferenceIssueKind, ReferenceKind, ReferenceRecord,
    ReferenceSource, ReferenceValidationIssue, ReferenceValidationOptions,
    ReferenceValidationReport, ReferenceValidationStatus, ReferenceValidator, SourcePoint,
    SourceRange, TransitiveOptions, ValidationMode, validate_references,
};
pub use scratchpad::ScratchPadProfile;
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
    FilesystemWorkspaceLocator, WorkspaceContext, WorkspaceDiscovery, WorkspaceError,
    WorkspaceLocator, WorkspacePaths, discover as discover_workspace, workspace_relative_path,
};
