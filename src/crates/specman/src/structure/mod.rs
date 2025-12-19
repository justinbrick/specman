mod cache;
mod index;
mod indexer;
mod query;

pub use cache::{IndexCache, UnresolvedHeadingRef, UnresolvedTarget};
pub use index::{
    ArtifactKey, ArtifactRecord, ConstraintIdentifier, ConstraintRecord, HeadingIdentifier,
    HeadingRecord, RelationshipEdge, RelationshipKind, WorkspaceIndex,
};
pub use indexer::{FilesystemStructureIndexer, StructureIndexing};
pub use query::StructureQuery;
