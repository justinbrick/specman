mod cache;
mod index;
mod indexer;
mod query;

pub use index::{
    ArtifactKey, ArtifactRecord, ConstraintIdentifier, ConstraintRecord, HeadingIdentifier,
    HeadingRecord, RelationshipEdge, RelationshipKind, WorkspaceIndex,
};
pub use indexer::{
    FilesystemStructureIndexer, StructureIndexing, build_workspace_index_for_artifacts,
};
pub use query::StructureQuery;
