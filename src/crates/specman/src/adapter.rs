use std::collections::BTreeMap;

use parking_lot::Mutex;

use crate::dependency_tree::{ArtifactId, DependencyTree};
use crate::error::SpecmanError;

pub trait DataModelAdapter: Send + Sync {
    fn save_dependency_tree(&self, tree: DependencyTree) -> Result<(), SpecmanError>;
    fn load_dependency_tree(
        &self,
        root: &ArtifactId,
    ) -> Result<Option<DependencyTree>, SpecmanError>;
    fn invalidate_dependency_tree(&self, _root: &ArtifactId) -> Result<(), SpecmanError> {
        Ok(())
    }
}

#[derive(Default)]
pub struct InMemoryAdapter {
    dependency_trees: Mutex<BTreeMap<ArtifactId, DependencyTree>>,
}

impl InMemoryAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DataModelAdapter for InMemoryAdapter {
    fn save_dependency_tree(&self, tree: DependencyTree) -> Result<(), SpecmanError> {
        self.dependency_trees
            .lock()
            .insert(tree.root.id.clone(), tree);
        Ok(())
    }

    fn load_dependency_tree(
        &self,
        root: &ArtifactId,
    ) -> Result<Option<DependencyTree>, SpecmanError> {
        Ok(self.dependency_trees.lock().get(root).cloned())
    }

    fn invalidate_dependency_tree(&self, root: &ArtifactId) -> Result<(), SpecmanError> {
        self.dependency_trees.lock().remove(root);
        Ok(())
    }
}
