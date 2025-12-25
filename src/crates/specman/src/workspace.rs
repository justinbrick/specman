use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use thiserror::Error;

use crate::error::SpecmanError;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error(
        "no .specman directory found from {searched_from} (hint: create .specman at the workspace root)"
    )]
    NotFound { searched_from: PathBuf },
    #[error(
        "workspace root {workspace_root} is missing a .specman directory (hint: create {workspace_root}/.specman)"
    )]
    DotSpecmanMissing { workspace_root: PathBuf },
    #[error("start path {start} is invalid: {message}")]
    InvalidStart { start: PathBuf, message: String },
    #[error("invalid workspace locator {locator}: {message}")]
    InvalidHandle { locator: String, message: String },
    #[error(
        "cannot create nested workspace at {requested}; ancestor workspace already has .specman at {existing}"
    )]
    NestedWorkspace {
        existing: PathBuf,
        requested: PathBuf,
    },
    #[error("path {candidate} escapes workspace {workspace_root}")]
    OutsideWorkspace {
        candidate: PathBuf,
        workspace_root: PathBuf,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

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

#[derive(Clone, Debug)]
pub struct WorkspaceContext {
    paths: WorkspacePaths,
    resolved: Arc<Mutex<HashMap<String, PathBuf>>>,
}

impl WorkspaceContext {
    pub fn new(paths: WorkspacePaths) -> Self {
        Self {
            paths,
            resolved: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn paths(&self) -> &WorkspacePaths {
        &self.paths
    }

    pub fn into_paths(self) -> WorkspacePaths {
        self.paths
    }

    /// Resolves SpecMan handles (`spec://`, `impl://`, `scratch://`) or workspace paths into
    /// absolute paths within the active workspace. Results are memoized per locator string to
    /// avoid repeated filesystem checks.
    pub fn resolve_locator(&self, locator: impl AsRef<str>) -> Result<PathBuf, WorkspaceError> {
        let key = locator.as_ref().to_string();
        if let Some(existing) = self.resolved.lock().unwrap().get(&key) {
            return Ok(existing.clone());
        }

        let resolved = self.resolve_locator_uncached(&key)?;
        self.resolved.lock().unwrap().insert(key, resolved.clone());
        Ok(resolved)
    }

    fn resolve_locator_uncached(&self, locator: &str) -> Result<PathBuf, WorkspaceError> {
        if locator.starts_with("http://") || locator.starts_with("https://") {
            return Err(WorkspaceError::InvalidHandle {
                locator: locator.to_string(),
                message: "https locators are not supported for workspace resolution".into(),
            });
        }

        if let Some(handle) = WorkspaceHandle::parse(locator)? {
            return Ok(handle.to_path(self.paths()));
        }

        let candidate = Path::new(locator);
        let absolute = if candidate.is_absolute() {
            lexical_normalize(candidate)
        } else {
            lexical_normalize(&self.paths.root().join(candidate))
        };

        self.ensure_inside(&absolute)?;
        Ok(absolute)
    }

    fn ensure_inside(&self, candidate: &Path) -> Result<(), WorkspaceError> {
        if candidate.starts_with(self.paths.root()) {
            Ok(())
        } else {
            Err(WorkspaceError::OutsideWorkspace {
                candidate: candidate.to_path_buf(),
                workspace_root: self.paths.root().to_path_buf(),
            })
        }
    }
}

pub struct WorkspaceDiscovery;

impl WorkspaceDiscovery {
    /// Performs workspace discovery from an arbitrary starting path.
    pub fn initialize(start_path: impl Into<PathBuf>) -> Result<WorkspaceContext, WorkspaceError> {
        let start_dir = normalize_start(start_path.into())?;
        let paths = Self::locate_from(start_dir)?;
        Ok(WorkspaceContext::new(paths))
    }

    /// Validates an explicit workspace root, ensuring the `.specman` directory exists.
    pub fn from_explicit(
        workspace_root: impl Into<PathBuf>,
    ) -> Result<WorkspaceContext, WorkspaceError> {
        let root = absolutize(workspace_root.into())?;
        if !root.is_dir() {
            return Err(WorkspaceError::InvalidStart {
                start: root,
                message: "workspace root must be a directory".into(),
            });
        }

        let dot_specman = root.join(".specman");
        if !dot_specman.is_dir() {
            return Err(WorkspaceError::DotSpecmanMissing {
                workspace_root: root,
            });
        }

        Ok(WorkspaceContext::new(WorkspacePaths::new(
            root,
            dot_specman,
        )))
    }

    /// Creates a new workspace at the provided root, provisioning `.specman` and required subdirectories.
    pub fn create(workspace_root: impl Into<PathBuf>) -> Result<WorkspaceContext, WorkspaceError> {
        let root = absolutize(workspace_root.into())?;
        let dot_specman = root.join(".specman");

        for ancestor in root.ancestors().skip(1) {
            let existing = ancestor.join(".specman");
            if existing.is_dir() {
                return Err(WorkspaceError::NestedWorkspace {
                    existing,
                    requested: root,
                });
            }
        }

        fs::create_dir_all(&root)?;

        if dot_specman.exists() && !dot_specman.is_dir() {
            return Err(WorkspaceError::InvalidStart {
                start: dot_specman,
                message: ".specman exists but is not a directory".into(),
            });
        }

        fs::create_dir_all(&dot_specman)?;
        fs::create_dir_all(dot_specman.join("scratchpad"))?;
        fs::create_dir_all(dot_specman.join("cache"))?;

        Ok(WorkspaceContext::new(WorkspacePaths::new(
            root,
            dot_specman,
        )))
    }

    fn locate_from(start_dir: PathBuf) -> Result<WorkspacePaths, WorkspaceError> {
        let search_origin = start_dir.clone();
        for ancestor in start_dir.ancestors() {
            let candidate = ancestor.join(".specman");
            if candidate.is_dir() {
                let normalized_root = lexical_normalize(ancestor);
                let normalized_dot = normalized_root.join(".specman");
                return Ok(WorkspacePaths::new(normalized_root, normalized_dot));
            }
        }

        Err(WorkspaceError::NotFound {
            searched_from: search_origin,
        })
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
        WorkspaceDiscovery::initialize(self.start.clone())
            .map(WorkspaceContext::into_paths)
            .map_err(SpecmanError::from)
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

impl<L> WorkspaceLocator for Arc<L>
where
    L: WorkspaceLocator,
{
    fn workspace(&self) -> Result<WorkspacePaths, SpecmanError> {
        (**self).workspace()
    }
}

/// Performs one-off workspace discovery from an arbitrary starting path.
pub fn discover(start: impl AsRef<Path>) -> Result<WorkspacePaths, SpecmanError> {
    WorkspaceDiscovery::initialize(start.as_ref().to_path_buf())
        .map(WorkspaceContext::into_paths)
        .map_err(SpecmanError::from)
}

fn normalize_start(start: PathBuf) -> Result<PathBuf, WorkspaceError> {
    let mut cursor = absolutize(start.clone())?;
    let original = cursor.clone();

    // Walk up until a real path exists to avoid failures for not-yet-created files.
    while !cursor.exists() {
        if !cursor.pop() {
            return Err(WorkspaceError::InvalidStart {
                start: original,
                message: "unable to find existing ancestor".into(),
            });
        }
    }

    if cursor.is_file() {
        cursor = cursor
            .parent()
            .ok_or_else(|| WorkspaceError::InvalidStart {
                start: original.clone(),
                message: "file path has no parent directory".into(),
            })?
            .to_path_buf();
    }

    if !cursor.is_dir() {
        return Err(WorkspaceError::InvalidStart {
            start: original,
            message: "start path is not a directory".into(),
        });
    }

    Ok(cursor)
}

fn absolutize(path: PathBuf) -> Result<PathBuf, WorkspaceError> {
    let base = if path.is_absolute() {
        path
    } else {
        env::current_dir()?.join(path)
    };

    Ok(lexical_normalize(&base))
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut pending_parents: usize = 0;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized
                    .components()
                    .next_back()
                    .is_some_and(|c| matches!(c, Component::Normal(_)))
                {
                    normalized.pop();
                } else if normalized.is_absolute() {
                    // Ignore attempts to go above the root for absolute paths.
                } else {
                    pending_parents += 1;
                }
            }
            Component::Normal(part) => {
                while pending_parents > 0 {
                    normalized.push("..");
                    pending_parents -= 1;
                }
                normalized.push(part);
            }
        }
    }

    while pending_parents > 0 {
        normalized.push("..");
        pending_parents -= 1;
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkspaceHandleKind {
    Specification,
    Implementation,
    ScratchPad,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WorkspaceHandle {
    kind: WorkspaceHandleKind,
    slug: String,
}

impl WorkspaceHandle {
    fn parse(reference: &str) -> Result<Option<Self>, WorkspaceError> {
        if let Some(rest) = reference.strip_prefix("spec://") {
            return Self::new(WorkspaceHandleKind::Specification, rest).map(Some);
        }

        if let Some(rest) = reference.strip_prefix("impl://") {
            return Self::new(WorkspaceHandleKind::Implementation, rest).map(Some);
        }

        if let Some(rest) = reference.strip_prefix("scratch://") {
            return Self::new(WorkspaceHandleKind::ScratchPad, rest).map(Some);
        }

        if reference.contains("://")
            && !reference.starts_with("http://")
            && !reference.starts_with("https://")
        {
            let scheme = reference
                .split_once("://")
                .map(|(scheme, _)| scheme)
                .unwrap_or(reference);
            return Err(WorkspaceError::InvalidHandle {
                locator: reference.to_string(),
                message: format!(
                    "unsupported locator scheme {scheme}:// (expected spec://, impl://, scratch://, or workspace-relative path)"
                ),
            });
        }

        Ok(None)
    }

    fn new(kind: WorkspaceHandleKind, raw_slug: &str) -> Result<Self, WorkspaceError> {
        let slug = Self::canonical_slug(raw_slug)?;
        Ok(Self { kind, slug })
    }

    fn canonical_slug(raw: &str) -> Result<String, WorkspaceError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(WorkspaceError::InvalidHandle {
                locator: raw.to_string(),
                message: "resource handle must include a non-empty identifier".into(),
            });
        }

        if trimmed.contains('/') || trimmed.contains('\\') {
            return Err(WorkspaceError::InvalidHandle {
                locator: raw.to_string(),
                message: "resource handle identifiers cannot contain path separators".into(),
            });
        }

        let canonical = trimmed.to_ascii_lowercase();
        if !canonical
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
        {
            return Err(WorkspaceError::InvalidHandle {
                locator: raw.to_string(),
                message:
                    "resource handle identifiers may only contain letters, numbers, '-' or '_'"
                        .into(),
            });
        }

        Ok(canonical)
    }

    fn to_path(&self, workspace: &WorkspacePaths) -> PathBuf {
        match self.kind {
            WorkspaceHandleKind::Specification => {
                workspace.spec_dir().join(&self.slug).join("spec.md")
            }
            WorkspaceHandleKind::Implementation => {
                workspace.impl_dir().join(&self.slug).join("impl.md")
            }
            WorkspaceHandleKind::ScratchPad => workspace
                .scratchpad_dir()
                .join(&self.slug)
                .join("scratch.md"),
        }
    }
}

impl From<WorkspaceError> for SpecmanError {
    fn from(err: WorkspaceError) -> Self {
        SpecmanError::Workspace(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn discovery_prefers_nearest_dot_specman() {
        let temp = tempdir().unwrap();
        let top = temp.path().join("repo");
        let nested = top.join("nested");
        fs::create_dir_all(top.join(".specman")).unwrap();
        fs::create_dir_all(nested.join(".specman")).unwrap();
        fs::create_dir_all(nested.join("deep")).unwrap();

        let ctx = WorkspaceDiscovery::initialize(nested.join("deep")).unwrap();

        assert_eq!(ctx.paths().root(), nested.as_path());
        assert_eq!(ctx.paths().dot_specman(), nested.join(".specman").as_path());
    }

    #[test]
    fn discovery_errors_when_dot_specman_missing() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("orphaned");
        fs::create_dir_all(root.join("child")).unwrap();

        let err = WorkspaceDiscovery::initialize(root.join("child"))
            .expect_err("expected workspace error");
        assert!(matches!(err, WorkspaceError::NotFound { .. }));
    }

    #[test]
    fn explicit_path_requires_dot_specman() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("explicit");
        fs::create_dir_all(&root).unwrap();

        let err = WorkspaceDiscovery::from_explicit(&root).expect_err("missing .specman");
        assert!(matches!(err, WorkspaceError::DotSpecmanMissing { .. }));
    }

    #[test]
    fn context_resolves_handles_and_paths() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        let ctx = WorkspaceDiscovery::from_explicit(&root).unwrap();

        let spec_path = ctx.resolve_locator("spec://core").unwrap();
        assert_eq!(spec_path, root.join("spec/core/spec.md"));

        let rel = ctx.resolve_locator("docs/guide.md").unwrap();
        assert_eq!(rel, root.join("docs/guide.md"));

        let abs = ctx
            .resolve_locator(root.join("impl/core/impl.md").to_string_lossy())
            .unwrap();
        assert_eq!(abs, root.join("impl/core/impl.md"));
    }

    #[test]
    fn context_rejects_workspace_escape() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        let ctx = WorkspaceDiscovery::from_explicit(&root).unwrap();

        let err = ctx
            .resolve_locator("../outside.md")
            .expect_err("should reject escape");
        assert!(matches!(err, WorkspaceError::OutsideWorkspace { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn discovery_preserves_symlink_paths() {
        use std::os::unix::fs as unix_fs;

        let temp = tempdir().unwrap();
        let real_root = temp.path().join("real");
        fs::create_dir_all(real_root.join(".specman")).unwrap();
        let link_root = temp.path().join("link");
        unix_fs::symlink(&real_root, &link_root).unwrap();

        let start = link_root.join("nested");
        fs::create_dir_all(&start).unwrap();

        let ctx = WorkspaceDiscovery::initialize(&start).unwrap();
        assert_eq!(ctx.paths().root(), link_root.as_path());
        assert_eq!(
            ctx.paths().dot_specman(),
            link_root.join(".specman").as_path()
        );
    }

    #[test]
    fn filesystem_locator_revalidates_cache() {
        let temp = tempdir().unwrap();
        let workspace_root = temp.path().join("workspace");
        fs::create_dir_all(workspace_root.join(".specman")).unwrap();
        let locator = FilesystemWorkspaceLocator::new(workspace_root.join("sub"));

        let first = locator.workspace().expect("initial lookup succeeds");
        assert_eq!(first.root(), workspace_root.as_path());

        fs::remove_dir_all(first.dot_specman()).unwrap();

        let err = locator.workspace().expect_err("should error after removal");
        assert!(matches!(err, SpecmanError::Workspace(_)));
    }

    #[test]
    fn create_provisions_workspace_and_is_idempotent() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("new-workspace");

        let first = WorkspaceDiscovery::create(&root).expect("create succeeds");
        assert!(first.paths().dot_specman().is_dir());
        assert!(first.paths().scratchpad_dir().is_dir());
        assert!(first.paths().dot_specman().join("cache").is_dir());

        let second = WorkspaceDiscovery::create(&root).expect("idempotent");
        assert_eq!(second.paths().root(), first.paths().root());
    }

    #[test]
    fn create_rejects_nested_workspace() {
        let temp = tempdir().unwrap();
        let outer = temp.path().join("outer");
        fs::create_dir_all(outer.join(".specman")).unwrap();

        let inner = outer.join("inner");
        let err = WorkspaceDiscovery::create(&inner).expect_err("nested should fail");
        assert!(matches!(err, WorkspaceError::NestedWorkspace { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn create_preserves_symlink_paths() {
        use std::os::unix::fs as unix_fs;

        let temp = tempdir().unwrap();
        let real_root = temp.path().join("real-create");
        fs::create_dir_all(&real_root).unwrap();
        let link_root = temp.path().join("link-create");
        unix_fs::symlink(&real_root, &link_root).unwrap();

        let ctx = WorkspaceDiscovery::create(&link_root).expect("create via symlink");
        assert_eq!(ctx.paths().root(), link_root.as_path());
        assert_eq!(ctx.paths().dot_specman(), link_root.join(".specman"));
    }
}
