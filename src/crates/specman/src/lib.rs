pub(crate) mod core;
pub(crate) mod graph;
pub(crate) mod index;
pub(crate) mod metadata;
pub(crate) mod ops;
pub(crate) mod scratchpad;
pub(crate) mod storage;
pub(crate) mod templates;
pub(crate) mod validation;
pub(crate) mod workspace;

pub use core::env::SpecmanEnv;
pub use core::error::{LifecycleError, SpecmanError};
pub use core::shared::{EntityKind, SchemaRef, SemVer};
pub use graph::tree::{
    ArtifactId, ArtifactKind, ArtifactSummary, DependencyEdge, DependencyGraphServices,
    DependencyMapping, DependencyRelation, DependencyTree, FilesystemDependencyMapper,
    InventoryDependent, WorkspaceInventorySnapshot,
};
pub use index::{
    ArtifactKey, ArtifactRecord, ConstraintIdentifier, ConstraintRecord,
    FilesystemStructureIndexer, HeadingIdentifier, HeadingRecord, RelationshipEdge,
    RelationshipKind, StructureIndexing, StructureQuery, WorkspaceIndex,
};
pub use metadata::frontmatter::{
    ArtifactIdentityFields, ArtifactFrontMatter, DependencyEntry, ImplementationFrontMatter,
    ReferenceEntry, ScratchFrontMatter, ScratchRefactorMetadata, ScratchRevisionMetadata,
    ScratchFixMetadata,
    ScratchWorkType, ScratchWorkloadExtras, SpecificationFrontMatter, split_front_matter,
};
pub use metadata::{
    FrontMatterUpdate, FrontMatterUpdateResult, IdentityUpdate, ImplementationUpdate,
    ScratchUpdate, SpecificationUpdate,
};
pub use ops::create::{
    CreateImplOptions, CreateResult, CreateScratchOptions, CreateSpecOptions,
    create_implementation, create_scratch_pad, create_specification,
};
pub use ops::delete::{DeleteOptions, DeleteResult, delete_artifact};
pub use ops::update::apply_front_matter_update;
pub use scratchpad::ScratchPadProfile;
pub use storage::adapter::{DataModelAdapter, InMemoryAdapter};
pub use storage::persistence::{
    ArtifactRemovalStore, PersistedArtifact, RemovedArtifact, WorkspacePersistence,
};
pub use templates::catalog::{ResolvedTemplate, TemplateCatalog};
pub use templates::engine::{
    ImplContext, MarkdownTemplateEngine, RenderedTemplate, ScratchPadContext, SpecContext,
    TemplateDescriptor, TemplateEngine, TemplateLocator, TemplateProvenance, TemplateScenario,
    TemplateTier, TokenMap,
};
pub use validation::status::{
    ArtifactStatus, StatusResult, WorkspaceStatusConfig, WorkspaceStatusReport,
    validate_workspace_status,
};
pub use validation::{ValidationTag, validate_compliance};
pub use validation::analysis::{DeletionImpact, check_deletion_impact};
pub use validation::references::{
    DestinationKind, DiscoveredReference, HttpsMethod, HttpsValidationMode, HttpsValidationOptions,
    IssueSeverity, ReachabilityPolicy, ReferenceIssueKind, ReferenceKind, ReferenceRecord,
    ReferenceSource, ReferenceValidationIssue, ReferenceValidationOptions,
    ReferenceValidationReport, ReferenceValidationStatus, ReferenceValidator, SourcePoint,
    SourceRange, TransitiveOptions, ValidationMode, validate_references,
};
pub use workspace::{
    FilesystemWorkspaceLocator, WorkspaceContext, WorkspaceDiscovery, WorkspaceError,
    WorkspaceLocator, WorkspacePaths, discover as discover_workspace, workspace_relative_path,
};
