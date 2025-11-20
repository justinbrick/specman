use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::SpecmanError;

/// Canonical paths for a SpecMan workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspacePaths {
    root: PathBuf,
    dot_specman: PathBuf,
}

impl WorkspacePaths {
    pub fn new(root: PathBuf, dot_specman: PathBuf) -> Self {
        Self { root, dot_specman }
    }

    /// Returns the workspace root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the `.specman` folder for this workspace.
    pub fn dot_specman(&self) -> &Path {
        &self.dot_specman
    }

    /// Canonical specification directory (`{root}/spec`).
    pub fn spec_dir(&self) -> PathBuf {
        self.root.join("spec")
    }

    /// Canonical implementation directory (`{root}/impl`).
    pub fn impl_dir(&self) -> PathBuf {
        self.root.join("impl")
    }

    /// Canonical scratch pad directory (`{root}/.specman/scratchpad`).
    pub fn scratchpad_dir(&self) -> PathBuf {
        self.dot_specman.join("scratchpad")
    }
}

/// Trait describing a reusable workspace locator.
pub trait WorkspaceLocator: Send + Sync {
    fn workspace(&self) -> Result<WorkspacePaths, SpecmanError>;
}

/// Filesystem-backed workspace locator with lightweight caching.
pub struct FilesystemWorkspaceLocator {
    start: PathBuf,
    cache: Mutex<Option<WorkspacePaths>>,
}

impl FilesystemWorkspaceLocator {
    pub fn new(start: impl Into<PathBuf>) -> Self {
        Self {
            start: start.into(),
            cache: Mutex::new(None),
        }
    }

    pub fn from_current_dir() -> Result<Self, SpecmanError> {
        Ok(Self::new(env::current_dir()?))
    }

    fn refresh(&self) -> Result<WorkspacePaths, SpecmanError> {
        discover(&self.start)
    }
}

impl WorkspaceLocator for FilesystemWorkspaceLocator {
    fn workspace(&self) -> Result<WorkspacePaths, SpecmanError> {
        if let Some(paths) = self.cache.lock().unwrap().clone() {
            if paths.root().is_dir() && paths.dot_specman().is_dir() {
                return Ok(paths);
            }
        }

        let discovered = self.refresh()?;
        *self.cache.lock().unwrap() = Some(discovered.clone());
        Ok(discovered)
    }
}

/// Performs one-off workspace discovery from an arbitrary starting path.
pub fn discover(start: impl AsRef<Path>) -> Result<WorkspacePaths, SpecmanError> {
    let canonical_start = normalize_start(start.as_ref())?;

    for ancestor in canonical_start.ancestors() {
        let candidate = ancestor.join(".specman");
        if candidate.is_dir() {
            return Ok(WorkspacePaths::new(ancestor.to_path_buf(), candidate));
        }
    }

    Err(SpecmanError::Workspace(format!(
        "no .specman directory found from {}",
        canonical_start.display()
    )))
}

fn normalize_start(start: &Path) -> Result<PathBuf, SpecmanError> {
    let mut cursor = start.to_path_buf();

    // Walk up until a real path exists to avoid failures for not-yet-created files.
    while !cursor.exists() {
        if !cursor.pop() {
            return Err(SpecmanError::Workspace(format!(
                "unable to find existing ancestor for {}",
                start.display()
            )));
        }
    }

    if cursor.is_file() {
        cursor = cursor.parent().map(Path::to_path_buf).ok_or_else(|| {
            SpecmanError::Workspace(format!(
                "file path {} has no parent directory",
                start.display()
            ))
        })?;
    }

    if !cursor.is_dir() {
        return Err(SpecmanError::Workspace(format!(
            "start path {} is not a directory",
            cursor.display()
        )));
    }

    Ok(fs::canonicalize(cursor)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn discover_locates_nearest_workspace() {
        let temp = tempdir().unwrap();
        let workspace_root = temp.path().join("repo");
        fs::create_dir_all(workspace_root.join(".specman")).unwrap();
        fs::create_dir_all(workspace_root.join("spec").join("feature")).unwrap();

        let nested = workspace_root.join("spec").join("feature");
        let paths = discover(&nested).expect("workspace should be discovered");

        let expected_root = workspace_root.canonicalize().unwrap();
        let expected_dot = expected_root.join(".specman");

        assert_eq!(paths.root(), expected_root.as_path());
        assert_eq!(paths.dot_specman(), expected_dot.as_path());
    }

    #[test]
    fn discover_errors_when_dot_specman_missing() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("orphaned");
        fs::create_dir_all(&root).unwrap();

        let err = discover(&root).expect_err("expected workspace error");
        assert!(matches!(err, SpecmanError::Workspace(_)));
    }

    #[test]
    fn filesystem_locator_revalidates_cache() {
        let temp = tempdir().unwrap();
        let workspace_root = temp.path().join("workspace");
        fs::create_dir_all(workspace_root.join(".specman")).unwrap();
        let locator = FilesystemWorkspaceLocator::new(workspace_root.join("sub"));

        let first = locator.workspace().expect("initial lookup succeeds");
        assert_eq!(
            first.root(),
            workspace_root.canonicalize().unwrap().as_path()
        );

        fs::remove_dir_all(first.dot_specman()).unwrap();

        let err = locator.workspace().expect_err("should error after removal");
        assert!(matches!(err, SpecmanError::Workspace(_)));
    }
}
