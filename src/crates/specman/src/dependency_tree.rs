use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::SpecmanError;
use crate::shared_function::SemVer;

/// Identity for any SpecMan artifact (specification, implementation, or scratch pad).
#[derive(
    Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
pub struct ArtifactId {
    pub kind: ArtifactKind,
    pub name: String,
}

/// Artifact kind segmentation.
#[derive(
    Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
pub enum ArtifactKind {
    #[default]
    Specification,
    Implementation,
    ScratchPad,
}

/// Lightweight summary that includes version data for dependency planning.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct ArtifactSummary {
    pub id: ArtifactId,
    pub version: Option<SemVer>,
    pub metadata: BTreeMap<String, String>,
}

/// Directed edge between two artifacts.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct DependencyEdge {
    pub from: ArtifactSummary,
    pub to: ArtifactSummary,
    pub relation: DependencyRelation,
}

/// Relationship classification for dependency edges.
#[derive(
    Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
pub enum DependencyRelation {
    #[default]
    Upstream,
    Downstream,
}

/// Aggregated dependency data across upstream, downstream, and combined views.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct DependencyTree {
    pub root: ArtifactSummary,
    pub upstream: Vec<DependencyEdge>,
    pub downstream: Vec<DependencyEdge>,
    pub aggregate: Vec<DependencyEdge>,
}

impl DependencyTree {
    pub fn empty(root: ArtifactSummary) -> Self {
        Self {
            root,
            upstream: Vec::new(),
            downstream: Vec::new(),
            aggregate: Vec::new(),
        }
    }
}

/// Contract for dependency traversal services.
pub trait DependencyMapping: Send + Sync {
    fn dependency_tree(&self, root: &ArtifactId) -> Result<DependencyTree, SpecmanError>;
    fn upstream(&self, root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError>;
    fn downstream(&self, root: &ArtifactId) -> Result<Vec<DependencyEdge>, SpecmanError>;
}
