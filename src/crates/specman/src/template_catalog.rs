use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::error::SpecmanError;
use crate::scratchpad::{ScratchPadProfile, ScratchPadProfileKind};
use crate::template::{
    TemplateDescriptor, TemplateLocator, TemplateProvenance, TemplateScenario, TemplateTier,
};
use crate::workspace::WorkspacePaths;

const EMBEDDED_SPEC: &str = include_str!("../templates/spec/spec.md");
const EMBEDDED_IMPL: &str = include_str!("../templates/impl/impl.md");
const EMBEDDED_SCRATCH: &str = include_str!("../templates/scratch/scratch.md");

/// Canonical template catalog implementation backed by workspace overrides,
/// pointer files, remote caches, and embedded defaults.
pub struct TemplateCatalog {
    workspace: WorkspacePaths,
}

/// Result of resolving a template with provenance metadata for persistence.
#[derive(Clone, Debug)]
pub struct ResolvedTemplate {
    pub descriptor: TemplateDescriptor,
    pub provenance: TemplateProvenance,
}

impl TemplateCatalog {
    pub fn new(workspace: WorkspacePaths) -> Self {
        Self { workspace }
    }

    /// Resolves a template descriptor for the given scenario following the
    /// override → pointer → embedded order mandated by SpecMan Core.
    pub fn resolve(&self, scenario: TemplateScenario) -> Result<ResolvedTemplate, SpecmanError> {
        if let Some(resolved) = self.try_workspace_override(&scenario)? {
            return Ok(resolved);
        }

        if let Some(resolved) = self.try_pointer(&scenario)? {
            return Ok(resolved);
        }

        self.embedded_default(&scenario)
    }

    /// Sets or updates the pointer file for the provided scenario and returns the
    /// refreshed template provenance.
    pub fn set_pointer(
        &self,
        scenario: TemplateScenario,
        locator: impl AsRef<str>,
    ) -> Result<ResolvedTemplate, SpecmanError> {
        let pointer_name = pointer_name(&scenario);
        let templates_dir = self.templates_dir();
        let lock = PointerLock::acquire(&templates_dir, pointer_name)?;
        let destination = self.normalize_pointer_locator(locator.as_ref())?;
        if let PointerDestination::Remote(url) = &destination {
            let cache = TemplateCache::new(&self.workspace);
            cache.fetch_url(url)?;
        }

        self.write_pointer_file(pointer_name, destination.contents())?;

        drop(lock);
        self.resolve(scenario)
    }

    /// Removes the pointer file for the scenario and reports the fallback
    /// template provenance after re-running resolution.
    pub fn remove_pointer(
        &self,
        scenario: TemplateScenario,
    ) -> Result<ResolvedTemplate, SpecmanError> {
        let pointer_name = pointer_name(&scenario);
        let templates_dir = self.templates_dir();
        let lock = PointerLock::acquire(&templates_dir, pointer_name)?;
        let pointer_path = templates_dir.join(pointer_name);
        if !pointer_path.is_file() {
            return Err(SpecmanError::Template(format!(
                "pointer {} does not exist under {}",
                pointer_name,
                pointer_path.display()
            )));
        }

        let raw = fs::read_to_string(&pointer_path).map_err(|err| {
            SpecmanError::Template(format!(
                "failed to read pointer {}: {}",
                pointer_path.display(),
                err
            ))
        })?;
        fs::remove_file(&pointer_path).map_err(|err| {
            SpecmanError::Template(format!(
                "failed to remove pointer {}: {}",
                pointer_path.display(),
                err
            ))
        })?;

        let trimmed = raw.trim();
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            let url = Url::parse(trimmed).map_err(|err| {
                SpecmanError::Template(format!(
                    "pointer {} referenced invalid URL {}: {err}",
                    pointer_name, trimmed
                ))
            })?;
            let cache = TemplateCache::new(&self.workspace);
            cache.invalidate_url(&url)?;
        }

        self.refresh_embedded_cache(&scenario)?;
        drop(lock);
        self.resolve(scenario)
    }

    /// Convenience helper for describing scratch pad profiles with catalog
    /// managed templates and provenance metadata.
    pub fn scratch_profile(
        &self,
        kind: ScratchPadProfileKind,
    ) -> Result<ScratchPadProfile, SpecmanError> {
        let scenario = TemplateScenario::WorkType(kind.slug().to_string());
        let resolved = self.resolve(scenario)?;
        Ok(ScratchPadProfile {
            kind,
            name: String::new(),
            template: resolved.descriptor,
            provenance: Some(resolved.provenance),
            configuration: Default::default(),
        })
    }

    /// Returns the `.specman/templates` directory inside the active workspace.
    fn templates_dir(&self) -> PathBuf {
        self.workspace.dot_specman().join("templates")
    }

    /// Normalizes user-supplied pointer locators into remote URLs or workspace-bound file paths.
    fn normalize_pointer_locator(&self, raw: &str) -> Result<PointerDestination, SpecmanError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(SpecmanError::Template(
                "pointer locator must not be empty".to_string(),
            ));
        }

        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            let url = Url::parse(trimmed).map_err(|err| {
                SpecmanError::Template(format!(
                    "pointer locator {trimmed} is not a valid URL: {err}"
                ))
            })?;
            return Ok(PointerDestination::Remote(url));
        }

        let candidate = PathBuf::from(trimmed);
        let resolved = if candidate.is_absolute() {
            candidate
        } else {
            self.workspace.root().join(&candidate)
        };

        if !resolved.starts_with(self.workspace.root()) {
            return Err(SpecmanError::Template(format!(
                "pointer locator escapes the workspace: {}",
                resolved.display()
            )));
        }

        if !resolved.is_file() {
            return Err(SpecmanError::Template(format!(
                "pointer locator does not exist: {}",
                resolved.display()
            )));
        }

        Ok(PointerDestination::FilePath(workspace_relative(
            self.workspace.root(),
            &resolved,
        )))
    }

    /// Writes the uppercase pointer file via a temp file + rename so editors see atomic updates.
    fn write_pointer_file(&self, pointer: &str, contents: String) -> Result<(), SpecmanError> {
        let dir = self.templates_dir();
        fs::create_dir_all(&dir).map_err(|err| {
            SpecmanError::Template(format!(
                "failed to ensure template directory {}: {}",
                dir.display(),
                err
            ))
        })?;
        let pointer_path = dir.join(pointer);
        let tmp_path = pointer_path.with_extension("tmp");
        fs::write(&tmp_path, format!("{}\n", contents)).map_err(|err| {
            let _ = fs::remove_file(&tmp_path);
            SpecmanError::Template(format!(
                "failed to write temporary pointer {}: {}",
                tmp_path.display(),
                err
            ))
        })?;
        if pointer_path.is_file() {
            fs::remove_file(&pointer_path).map_err(|err| {
                SpecmanError::Template(format!(
                    "failed to remove previous pointer {}: {}",
                    pointer_path.display(),
                    err
                ))
            })?;
        }
        fs::rename(&tmp_path, &pointer_path).map_err(|err| {
            let _ = fs::remove_file(&tmp_path);
            SpecmanError::Template(format!(
                "failed to publish pointer {}: {}",
                pointer_path.display(),
                err
            ))
        })
    }

    /// Rewrites the embedded fallback cache copy immediately after pointer mutations.
    fn refresh_embedded_cache(&self, scenario: &TemplateScenario) -> Result<(), SpecmanError> {
        let (key, body) = embedded_assets(scenario);
        let cache = TemplateCache::new(&self.workspace);
        cache.write_embedded(key, body).map(|_| ())
    }

    fn try_workspace_override(
        &self,
        scenario: &TemplateScenario,
    ) -> Result<Option<ResolvedTemplate>, SpecmanError> {
        for candidate in self.override_candidates(scenario) {
            if candidate.is_file() {
                return Ok(Some(self.resolved_from_path(
                    scenario,
                    candidate,
                    TemplateTier::WorkspaceOverride,
                    None,
                    None,
                    None,
                    None,
                )));
            }
        }
        Ok(None)
    }

    fn try_pointer(
        &self,
        scenario: &TemplateScenario,
    ) -> Result<Option<ResolvedTemplate>, SpecmanError> {
        let pointer_name = pointer_name(scenario);
        let pointer_path = self.templates_dir().join(pointer_name);
        if !pointer_path.is_file() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&pointer_path).map_err(|err| {
            SpecmanError::Template(format!(
                "failed to read template pointer {}: {err}",
                pointer_path.display()
            ))
        })?;
        let trimmed = contents.trim();
        if trimmed.is_empty() {
            return Err(SpecmanError::Template(format!(
                "template pointer {} has no content",
                pointer_path.display()
            )));
        }

        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            let url = Url::parse(trimmed).map_err(|err| {
                SpecmanError::Template(format!("invalid template pointer URL {}: {err}", trimmed))
            })?;
            let cache = TemplateCache::new(&self.workspace);
            match cache.fetch_url(&url) {
                Ok(hit) => {
                    let cache_path = workspace_relative(self.workspace.root(), &hit.path);
                    return Ok(Some(self.resolved_from_path(
                        scenario,
                        hit.path,
                        TemplateTier::PointerUrl,
                        Some(pointer_name.to_string()),
                        Some(url.to_string()),
                        Some(cache_path),
                        hit.last_modified,
                    )));
                }
                Err(_err) => {
                    // Spec requires falling back to embedded defaults when the remote
                    // pointer cannot be refreshed and no cache exists.
                    return Ok(None);
                }
            }
        }

        let file_path = self.resolve_pointer_path(trimmed, pointer_name)?;
        Ok(Some(self.resolved_from_path(
            scenario,
            file_path,
            TemplateTier::PointerFile,
            Some(pointer_name.to_string()),
            None,
            None,
            None,
        )))
    }

    fn embedded_default(
        &self,
        scenario: &TemplateScenario,
    ) -> Result<ResolvedTemplate, SpecmanError> {
        let (key, body) = embedded_assets(scenario);

        let cache = TemplateCache::new(&self.workspace);
        let path = cache.write_embedded(key, body)?;
        let cache_path = workspace_relative(self.workspace.root(), &path);
        Ok(self.resolved_from_path(
            scenario,
            path,
            TemplateTier::EmbeddedDefault,
            None,
            Some(format!("embedded://{key}")),
            Some(cache_path),
            None,
        ))
    }

    fn override_candidates(&self, scenario: &TemplateScenario) -> Vec<PathBuf> {
        let base = self.templates_dir();
        match scenario {
            TemplateScenario::Specification => vec![base.join("spec.md")],
            TemplateScenario::Implementation => vec![base.join("impl.md")],
            TemplateScenario::ScratchPad => vec![base.join("scratch.md")],
            TemplateScenario::WorkType(kind) => {
                let slug = sanitize_key(kind);
                vec![
                    base.join("scratch").join(format!("{slug}.md")),
                    base.join(format!("scratch-{slug}.md")),
                    base.join("scratch.md"),
                ]
            }
        }
    }

    fn resolve_pointer_path(&self, raw: &str, pointer_name: &str) -> Result<PathBuf, SpecmanError> {
        let candidate = PathBuf::from(raw);
        let resolved = if candidate.is_absolute() {
            candidate
        } else {
            self.workspace.root().join(candidate)
        };

        if !resolved.starts_with(self.workspace.root()) {
            return Err(SpecmanError::Template(format!(
                "pointer {} resolved outside the workspace: {}",
                pointer_name,
                resolved.display()
            )));
        }

        if !resolved.is_file() {
            return Err(SpecmanError::Template(format!(
                "pointer {} references missing file: {}",
                pointer_name,
                resolved.display()
            )));
        }

        Ok(resolved)
    }

    fn resolved_from_path(
        &self,
        scenario: &TemplateScenario,
        path: PathBuf,
        tier: TemplateTier,
        pointer: Option<String>,
        locator_override: Option<String>,
        cache_override: Option<String>,
        last_modified: Option<String>,
    ) -> ResolvedTemplate {
        let locator = TemplateLocator::FilePath(path.clone());
        let provenance = TemplateProvenance {
            tier,
            locator: locator_override
                .unwrap_or_else(|| workspace_relative(self.workspace.root(), &path)),
            pointer,
            cache_path: cache_override,
            last_modified,
        };
        ResolvedTemplate {
            descriptor: TemplateDescriptor {
                locator,
                scenario: scenario.clone(),
                required_tokens: Vec::new(),
            },
            provenance,
        }
    }
}

fn pointer_name(scenario: &TemplateScenario) -> &'static str {
    match scenario {
        TemplateScenario::Specification => "SPEC",
        TemplateScenario::Implementation => "IMPL",
        TemplateScenario::ScratchPad | TemplateScenario::WorkType(_) => "SCRATCH",
    }
}

fn embedded_assets(scenario: &TemplateScenario) -> (&'static str, &'static str) {
    match scenario {
        TemplateScenario::Specification => ("spec", EMBEDDED_SPEC),
        TemplateScenario::Implementation => ("impl", EMBEDDED_IMPL),
        TemplateScenario::ScratchPad | TemplateScenario::WorkType(_) => {
            ("scratch", EMBEDDED_SCRATCH)
        }
    }
}

fn sanitize_key(raw: &str) -> String {
    raw.chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect::<String>()
        .to_lowercase()
}

fn workspace_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

struct TemplateCache {
    root: PathBuf,
}

impl TemplateCache {
    fn new(workspace: &WorkspacePaths) -> Self {
        Self {
            root: workspace.dot_specman().join("cache").join("templates"),
        }
    }

    fn ensure_root(&self) -> Result<(), SpecmanError> {
        fs::create_dir_all(&self.root).map_err(SpecmanError::from)
    }

    fn write_embedded(&self, key: &str, contents: &str) -> Result<PathBuf, SpecmanError> {
        self.ensure_root()?;
        let path = self.root.join(format!("embedded-{key}.md"));
        fs::write(&path, contents)?;
        Ok(path)
    }

    fn fetch_url(&self, url: &Url) -> Result<CacheHit, SpecmanError> {
        self.ensure_root()?;
        let key = hash_url(url);
        let path = self.root.join(format!("url-{key}.md"));
        let meta_path = self.root.join(format!("url-{key}.json"));

        match ureq::get(url.as_str()).call() {
            Ok(response) => {
                if response.status() >= 400 {
                    return Err(SpecmanError::Template(format!(
                        "failed to download template {}; status {}",
                        url,
                        response.status()
                    )));
                }
                let last_modified = response
                    .header("Last-Modified")
                    .map(|value| value.to_string());
                let body = response
                    .into_string()
                    .map_err(|err| SpecmanError::Template(err.to_string()))?;
                fs::write(&path, body)?;
                let metadata = TemplateCacheMetadata {
                    locator: url.to_string(),
                    last_modified: last_modified.clone(),
                };
                fs::write(&meta_path, serde_json::to_string_pretty(&metadata)?)?;
                Ok(CacheHit {
                    path,
                    last_modified,
                })
            }
            Err(err) => {
                if path.is_file() {
                    let metadata = read_metadata(&meta_path)?;
                    return Ok(CacheHit {
                        path,
                        last_modified: metadata.and_then(|m| m.last_modified),
                    });
                }
                Err(SpecmanError::Template(format!(
                    "failed to download template {}: {}",
                    url, err
                )))
            }
        }
    }

    /// Deletes cached remote template artifacts tied to the provided URL, if present.
    fn invalidate_url(&self, url: &Url) -> Result<(), SpecmanError> {
        if !self.root.exists() {
            return Ok(());
        }
        let key = hash_url(url);
        let path = self.root.join(format!("url-{key}.md"));
        let meta_path = self.root.join(format!("url-{key}.json"));
        if path.is_file() {
            fs::remove_file(&path)?;
        }
        if meta_path.is_file() {
            fs::remove_file(&meta_path)?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct TemplateCacheMetadata {
    locator: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_modified: Option<String>,
}

struct CacheHit {
    path: PathBuf,
    last_modified: Option<String>,
}

fn read_metadata(path: &Path) -> Result<Option<TemplateCacheMetadata>, SpecmanError> {
    if !path.is_file() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    let metadata = serde_json::from_str(&content).map_err(|err| {
        SpecmanError::Serialization(format!("invalid template cache metadata: {err}"))
    })?;
    Ok(Some(metadata))
}

fn hash_url(url: &Url) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.as_str().as_bytes());
    let digest = hasher.finalize();
    hex::encode(digest)
}

/// Normalized representation of pointer destinations.
enum PointerDestination {
    Remote(Url),
    FilePath(String),
}

impl PointerDestination {
    /// Returns the string persisted inside the pointer file for this destination.
    fn contents(&self) -> String {
        match self {
            PointerDestination::Remote(url) => url.as_str().to_string(),
            PointerDestination::FilePath(path) => path.clone(),
        }
    }
}

/// Filesystem-backed lock guard that serializes pointer mutations across processes.
struct PointerLock {
    path: PathBuf,
}

impl PointerLock {
    /// Attempts to create a `.lock-{pointer}` file, retrying briefly before surfacing a timeout.
    fn acquire(dir: &Path, pointer: &str) -> Result<Self, SpecmanError> {
        fs::create_dir_all(dir).map_err(|err| {
            SpecmanError::Template(format!(
                "failed to prepare template directory {}: {}",
                dir.display(),
                err
            ))
        })?;
        let lock_path = dir.join(format!(".lock-{pointer}"));
        let start = Instant::now();
        loop {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(_) => return Ok(Self { path: lock_path }),
                Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                    if start.elapsed() >= Duration::from_secs(5) {
                        return Err(SpecmanError::Template(format!(
                            "timed out acquiring pointer lock {}",
                            lock_path.display()
                        )));
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(err) => {
                    return Err(SpecmanError::Template(format!(
                        "failed to create pointer lock {}: {}",
                        lock_path.display(),
                        err
                    )));
                }
            }
        }
    }
}

impl Drop for PointerLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::WorkspacePaths;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn sets_pointer_to_workspace_file() {
        let (_tempdir, workspace) = workspace_fixture();
        let catalog = TemplateCatalog::new(workspace.clone());
        let custom_path = workspace.root().join("custom-spec.md");
        fs::write(&custom_path, "# spec template").unwrap();

        let result = catalog
            .set_pointer(TemplateScenario::Specification, "custom-spec.md")
            .expect("pointer set should succeed");

        assert!(matches!(result.provenance.tier, TemplateTier::PointerFile));
        let pointer_contents = fs::read_to_string(catalog.templates_dir().join("SPEC"))
            .expect("pointer file readable");
        assert_eq!(pointer_contents.trim(), "custom-spec.md");
        assert!(
            result
                .descriptor
                .locator
                .matches_path(custom_path.as_path())
        );
        assert!(!catalog.templates_dir().join(".lock-SPEC").exists());
    }

    #[test]
    fn removing_pointer_rewrites_embedded_cache() {
        let (_tempdir, workspace) = workspace_fixture();
        let catalog = TemplateCatalog::new(workspace.clone());
        let custom_path = workspace.root().join("custom-impl.md");
        fs::write(&custom_path, "# impl template").unwrap();

        catalog
            .set_pointer(TemplateScenario::Implementation, "custom-impl.md")
            .expect("pointer set");

        let embedded_path = workspace
            .dot_specman()
            .join("cache/templates/embedded-impl.md");
        if embedded_path.is_file() {
            fs::remove_file(&embedded_path).unwrap();
        }

        let result = catalog
            .remove_pointer(TemplateScenario::Implementation)
            .expect("pointer removal succeeds");

        assert!(matches!(
            result.provenance.tier,
            TemplateTier::EmbeddedDefault
        ));
        assert!(embedded_path.is_file());
        assert_eq!(fs::read_to_string(&embedded_path).unwrap(), EMBEDDED_IMPL);
        assert!(!catalog.templates_dir().join("IMPL").exists());
    }

    #[test]
    fn set_pointer_downloads_remote_cache_and_removal_purges_it() {
        let (_tempdir, workspace) = workspace_fixture();
        let catalog = TemplateCatalog::new(workspace.clone());
        let (url, handle) = serve_once("# remote impl template");

        let set_result = catalog
            .set_pointer(TemplateScenario::Implementation, &url)
            .expect("remote pointer set");
        handle.join().unwrap();

        assert!(matches!(
            set_result.provenance.tier,
            TemplateTier::PointerUrl
        ));
        let normalized_url = Url::parse(&url).unwrap().to_string();
        let pointer_contents =
            fs::read_to_string(catalog.templates_dir().join("IMPL")).expect("pointer file present");
        assert_eq!(pointer_contents.trim(), normalized_url);
        let cache_file = remote_cache_path(workspace.dot_specman(), &normalized_url);
        assert!(cache_file.is_file());
        assert!(
            fs::read_to_string(&cache_file)
                .unwrap()
                .contains("remote impl")
        );

        let removal_result = catalog
            .remove_pointer(TemplateScenario::Implementation)
            .expect("remote pointer removal");
        assert!(matches!(
            removal_result.provenance.tier,
            TemplateTier::EmbeddedDefault
        ));
        assert!(!cache_file.exists());
    }

    fn workspace_fixture() -> (tempfile::TempDir, WorkspacePaths) {
        let tempdir = tempfile::tempdir().unwrap();
        let root = tempdir.path().to_path_buf();
        let dot_specman = root.join(".specman");
        fs::create_dir_all(dot_specman.join("templates")).unwrap();
        fs::create_dir_all(dot_specman.join("cache/templates")).unwrap();
        fs::create_dir_all(root.join("spec")).unwrap();
        fs::create_dir_all(root.join("impl")).unwrap();
        (tempdir, WorkspacePaths::new(root, dot_specman))
    }

    fn serve_once(body: &str) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let body = body.to_string();
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        (format!("http://{}", addr), handle)
    }

    fn remote_cache_path(dot_specman: &Path, url: &str) -> PathBuf {
        let parsed = Url::parse(url).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(parsed.as_str().as_bytes());
        let digest = hasher.finalize();
        let key = hex::encode(digest);
        dot_specman
            .join("cache")
            .join("templates")
            .join(format!("url-{key}.md"))
    }

    trait LocatorExt {
        fn matches_path(&self, expected: &Path) -> bool;
    }

    impl LocatorExt for TemplateLocator {
        fn matches_path(&self, expected: &Path) -> bool {
            match self {
                TemplateLocator::FilePath(path) => path == expected,
                _ => false,
            }
        }
    }
}
