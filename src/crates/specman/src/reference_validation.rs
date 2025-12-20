use crate::{SpecmanError, WorkspacePaths};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use unicode_normalization::UnicodeNormalization;
use url::Url;

#[derive(Clone, Debug, Default)]
pub struct ReferenceValidationOptions {
    pub https: HttpsValidationOptions,
    pub transitive: TransitiveOptions,
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_nfkd_filters_combining_marks() {
        // "café" (U+00E9) -> NFKD: 'e' + combining acute; combining mark is removed by filtering.
        assert_eq!(slugify_heading("café"), Some("cafe".to_string()));
    }

    #[test]
    fn slugify_removes_punctuation_and_hyphenates_spaces() {
        assert_eq!(
            slugify_heading("Hello, world!"),
            Some("hello-world".to_string())
        );
    }

    #[test]
    fn slugify_collapses_hyphens_and_trims() {
        assert_eq!(slugify_heading("-- A  --  B --"), Some("a-b".to_string()));
    }

    #[test]
    fn heading_slug_deduplication_appends_suffix_starting_at_1() {
        let md = "# Overview\n\n## Overview\n\n## Overview\n";
        let root = markdown::to_mdast(md, &markdown::ParseOptions::default()).unwrap();
        let mut issues = Vec::new();
        let slugs = collect_heading_slugs(&root, &mut issues, "doc");
        assert!(issues.is_empty());
        assert!(slugs.contains("overview"));
        assert!(slugs.contains("overview-1"));
        assert!(slugs.contains("overview-2"));
    }

    #[test]
    fn heading_plain_text_includes_link_reference_text() {
        let md = "## Hello [World][x]\n\n[x]: https://example.com\n";
        let root = markdown::to_mdast(md, &markdown::ParseOptions::default()).unwrap();
        let mut issues = Vec::new();
        let slugs = collect_heading_slugs(&root, &mut issues, "doc");
        assert!(issues.is_empty());
        assert!(slugs.contains("hello-world"));
    }
}

#[derive(Clone, Debug)]
pub struct HttpsValidationOptions {
    pub mode: HttpsValidationMode,
}

impl Default for HttpsValidationOptions {
    fn default() -> Self {
        Self {
            mode: HttpsValidationMode::SyntaxOnly,
        }
    }
}

#[derive(Clone, Debug)]
pub enum HttpsValidationMode {
    /// Parse + validate `https://` syntax; do not perform network I/O.
    SyntaxOnly,
    /// Perform reachability checks using `ureq`.
    Reachability {
        timeout: Duration,
        max_redirects: u32,
        method: HttpsMethod,
    },
}

#[derive(Clone, Debug)]
pub enum HttpsMethod {
    Head,
    Get,
}

impl Default for HttpsValidationMode {
    fn default() -> Self {
        Self::SyntaxOnly
    }
}

#[derive(Clone, Debug)]
pub struct TransitiveOptions {
    /// Whether to validate linked markdown documents transitively.
    pub enabled: bool,
    /// Maximum number of markdown documents to validate including the root.
    pub max_documents: usize,
}

impl Default for TransitiveOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            max_documents: 64,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReferenceValidationStatus {
    Success,
    Failure,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReferenceValidationReport {
    pub status: ReferenceValidationStatus,
    /// All references discovered across processed documents.
    pub discovered: Vec<DiscoveredReference>,
    /// Issues across processed documents.
    pub issues: Vec<ReferenceValidationIssue>,
    /// How many documents were processed.
    pub processed_documents: usize,
}

impl ReferenceValidationReport {
    fn new() -> Self {
        Self {
            status: ReferenceValidationStatus::Success,
            discovered: Vec::new(),
            issues: Vec::new(),
            processed_documents: 0,
        }
    }

    fn finalize(&mut self) {
        if self
            .issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Error)
        {
            self.status = ReferenceValidationStatus::Failure;
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum IssueSeverity {
    Error,
    Diagnostic,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReferenceValidationIssue {
    pub severity: IssueSeverity,
    pub message: String,
    pub source: ReferenceSource,
    pub destination: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredReference {
    pub source: ReferenceSource,
    pub destination: String,
    pub kind: DestinationKind,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DestinationKind {
    WorkspaceFilesystem,
    HttpsUrl,
    FragmentOnly,
    UnsupportedOrUnknown,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReferenceSource {
    /// A stable key identifying the document.
    pub document: String,
    pub range: Option<SourceRange>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceRange {
    pub start: SourcePoint,
    pub end: SourcePoint,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourcePoint {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DocumentKey {
    File(PathBuf),
    Url(String),
}

/// Validate Markdown link destinations in the given artifact.
///
/// - `locator` may be: workspace path (absolute or workspace-relative), `https://...`, or a
///   SpecMan handle (`spec://`, `impl://`, `scratch://`).
/// - SpecMan handles inside Markdown destinations are invalid and are reported as errors.
pub fn validate_references(
    locator: &str,
    workspace: &WorkspacePaths,
    options: ReferenceValidationOptions,
) -> Result<ReferenceValidationReport, SpecmanError> {
    let mut report = ReferenceValidationReport::new();

    let root = resolve_input_locator(locator, workspace);

    let mut queue = VecDeque::new();
    queue.push_back(root);

    let mut visited = BTreeSet::new();
    let mut pending_fragments: BTreeMap<DocumentKey, BTreeSet<PendingFragmentCheck>> =
        BTreeMap::new();

    while let Some(target) = queue.pop_front() {
        let key = match &target {
            ResolvedDocument::File { path, .. } => DocumentKey::File(path.clone()),
            ResolvedDocument::Url { url } => DocumentKey::Url(url.as_str().to_string()),
        };

        if !visited.insert(key.clone()) {
            continue;
        }

        report.processed_documents += 1;
        if options.transitive.enabled
            && report.processed_documents > options.transitive.max_documents
        {
            report.issues.push(ReferenceValidationIssue {
                severity: IssueSeverity::Diagnostic,
                message: format!(
                    "transitive traversal stopped after {} documents (max_documents)",
                    options.transitive.max_documents
                ),
                source: ReferenceSource {
                    document: target.describe(),
                    range: None,
                },
                destination: None,
            });
            break;
        }

        let content = match load_document_content(&target, &options.https.mode) {
            Ok(Some(content)) => content,
            Ok(None) => continue,
            Err(issue) => {
                report.issues.push(issue);
                continue;
            }
        };

        let (discovered, issues, next_docs, heading_slugs) = validate_document(
            &target,
            &content,
            workspace,
            &options,
            &mut pending_fragments,
        );
        report.discovered.extend(discovered);
        report.issues.extend(issues);

        // Validate any fragments discovered previously that target this document.
        if let Some(checks) = pending_fragments.remove(&key) {
            for check in checks {
                if !heading_slugs.contains(&check.fragment) {
                    report.issues.push(ReferenceValidationIssue {
                        severity: IssueSeverity::Error,
                        message: format!(
                            "fragment '#{}' does not match any heading slug in {}",
                            check.fragment,
                            target.describe()
                        ),
                        source: check.source,
                        destination: Some(check.destination),
                    });
                }
            }
        }

        if options.transitive.enabled {
            // Enqueue in deterministic order.
            for doc in next_docs {
                queue.push_back(doc);
            }
        }
    }

    report.finalize();
    Ok(report)
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PendingFragmentCheck {
    source: ReferenceSource,
    fragment: String,
    destination: String,
}

#[derive(Clone, Debug)]
enum ResolvedDocument {
    File {
        path: PathBuf,
        /// Base directory used to resolve relative links.
        base_dir: PathBuf,
    },
    Url {
        url: Url,
    },
}

impl ResolvedDocument {
    fn describe(&self) -> String {
        match self {
            ResolvedDocument::File { path, .. } => path_to_forward_slashes(path),
            ResolvedDocument::Url { url } => url.as_str().to_string(),
        }
    }
}

fn resolve_input_locator(locator: &str, workspace: &WorkspacePaths) -> ResolvedDocument {
    if let Ok(url) = parse_https_url(locator) {
        return ResolvedDocument::Url { url };
    }

    let path = if let Some(rest) = locator.strip_prefix("spec://") {
        resolve_handle_path(workspace.spec_dir(), rest, "spec.md")
    } else if let Some(rest) = locator.strip_prefix("impl://") {
        resolve_handle_path(workspace.impl_dir(), rest, "impl.md")
    } else if let Some(rest) = locator.strip_prefix("scratch://") {
        resolve_handle_path(workspace.scratchpad_dir(), rest, "scratch.md")
    } else {
        // workspace-relative or absolute path
        if Path::new(locator).is_absolute() {
            PathBuf::from(locator)
        } else {
            workspace.root().join(locator)
        }
    };

    let (path, base_dir) = if path.exists() {
        let canonical = fs::canonicalize(&path).unwrap_or(path);
        let base_dir = canonical
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        (canonical, base_dir)
    } else {
        let base_dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        (path, base_dir)
    };

    ResolvedDocument::File { path, base_dir }
}

fn resolve_handle_path(base: PathBuf, raw_slug: &str, leaf: &str) -> PathBuf {
    // Canonicalize using the same constraints as `ResourceHandle::canonical_slug`.
    // Any error is surfaced later as a validation error during loading.
    let slug = raw_slug.trim().to_ascii_lowercase();
    base.join(slug).join(leaf)
}

fn load_document_content(
    target: &ResolvedDocument,
    https_mode: &HttpsValidationMode,
) -> Result<Option<String>, ReferenceValidationIssue> {
    match target {
        ResolvedDocument::File { path, .. } => match fs::read_to_string(path) {
            Ok(s) => Ok(Some(s)),
            Err(err) => Err(ReferenceValidationIssue {
                severity: IssueSeverity::Error,
                message: format!("failed to read file {}: {err}", path.display()),
                source: ReferenceSource {
                    document: path_to_forward_slashes(path),
                    range: None,
                },
                destination: None,
            }),
        },
        ResolvedDocument::Url { url } => match https_mode {
            HttpsValidationMode::SyntaxOnly => Ok(None),
            HttpsValidationMode::Reachability { .. } => {
                // We only fetch content if reachability mode is enabled.
                match fetch_url(url, https_mode) {
                    Ok(text) => Ok(Some(text)),
                    Err(issue) => Err(issue),
                }
            }
        },
    }
}

fn fetch_url(url: &Url, mode: &HttpsValidationMode) -> Result<String, ReferenceValidationIssue> {
    let (timeout, max_redirects, method) = match mode {
        HttpsValidationMode::Reachability {
            timeout,
            max_redirects,
            method,
        } => (*timeout, *max_redirects, method.clone()),
        HttpsValidationMode::SyntaxOnly => {
            return Err(ReferenceValidationIssue {
                severity: IssueSeverity::Diagnostic,
                message: "fetch_url called in syntax-only mode".into(),
                source: ReferenceSource {
                    document: url.as_str().to_string(),
                    range: None,
                },
                destination: Some(url.as_str().to_string()),
            });
        }
    };

    let agent = ureq::AgentBuilder::new()
        .timeout_read(timeout)
        .timeout_write(timeout)
        .redirects(max_redirects)
        .build();

    let request = match method {
        HttpsMethod::Head => agent.head(url.as_str()),
        HttpsMethod::Get => agent.get(url.as_str()),
    };

    match request.call() {
        Ok(response) => match response.into_string() {
            Ok(s) => Ok(s),
            Err(err) => Err(ReferenceValidationIssue {
                severity: IssueSeverity::Diagnostic,
                message: format!("failed reading response body: {err}"),
                source: ReferenceSource {
                    document: url.as_str().to_string(),
                    range: None,
                },
                destination: Some(url.as_str().to_string()),
            }),
        },
        Err(ureq::Error::Status(code, response)) => {
            let severity = if (400..500).contains(&code) {
                IssueSeverity::Error
            } else {
                IssueSeverity::Diagnostic
            };

            let hint = response.status_text().to_string();
            Err(ReferenceValidationIssue {
                severity,
                message: format!("https reachability failed with status {code} ({hint})"),
                source: ReferenceSource {
                    document: url.as_str().to_string(),
                    range: None,
                },
                destination: Some(url.as_str().to_string()),
            })
        }
        Err(err) => Err(ReferenceValidationIssue {
            severity: IssueSeverity::Diagnostic,
            message: format!("https request failed: {err}"),
            source: ReferenceSource {
                document: url.as_str().to_string(),
                range: None,
            },
            destination: Some(url.as_str().to_string()),
        }),
    }
}

fn validate_document(
    target: &ResolvedDocument,
    markdown_source: &str,
    workspace: &WorkspacePaths,
    options: &ReferenceValidationOptions,
    pending_fragments: &mut BTreeMap<DocumentKey, BTreeSet<PendingFragmentCheck>>,
) -> (
    Vec<DiscoveredReference>,
    Vec<ReferenceValidationIssue>,
    Vec<ResolvedDocument>,
    BTreeSet<String>,
) {
    let document_id = target.describe();

    let mut discovered = Vec::new();
    let mut issues = Vec::new();

    let mut markdown_targets = BTreeMap::<String, ResolvedDocument>::new();

    // Parse mdast with positions enabled.
    let root = match markdown::to_mdast(markdown_source, &markdown::ParseOptions::default()) {
        Ok(node) => node,
        Err(message) => {
            issues.push(ReferenceValidationIssue {
                severity: IssueSeverity::Error,
                message: format!("markdown parse error: {message}"),
                source: ReferenceSource {
                    document: document_id,
                    range: None,
                },
                destination: None,
            });
            return (discovered, issues, Vec::new(), BTreeSet::new());
        }
    };

    let definition_map = collect_definitions(&root);
    let heading_slugs = collect_heading_slugs(&root, &mut issues, &document_id);

    for extracted in extract_destinations(&root, &definition_map) {
        if let Some(missing) = extracted.unresolved_reference_identifier {
            issues.push(ReferenceValidationIssue {
                severity: IssueSeverity::Error,
                message: format!("unresolved link reference identifier: {missing:?}"),
                source: ReferenceSource {
                    document: document_id.clone(),
                    range: extracted.range,
                },
                destination: None,
            });
            continue;
        }

        let destination = extracted.destination;
        let range = extracted.range;
        let source = ReferenceSource {
            document: document_id.clone(),
            range,
        };

        let kind = classify_destination(&destination);
        discovered.push(DiscoveredReference {
            source: source.clone(),
            destination: destination.clone(),
            kind: kind.clone(),
        });

        match kind {
            DestinationKind::FragmentOnly => {
                let fragment = destination.trim_start_matches('#');
                let fragment = fragment.split('?').next().unwrap_or(fragment);
                if fragment.is_empty() {
                    issues.push(ReferenceValidationIssue {
                        severity: IssueSeverity::Error,
                        message: "empty fragment is invalid".into(),
                        source,
                        destination: Some(destination),
                    });
                } else if !heading_slugs.contains(fragment) {
                    issues.push(ReferenceValidationIssue {
                        severity: IssueSeverity::Error,
                        message: format!("fragment '#{fragment}' does not match any heading slug"),
                        source,
                        destination: Some(destination),
                    });
                }
            }
            DestinationKind::HttpsUrl => {
                if parse_https_url(&destination).is_err() {
                    issues.push(ReferenceValidationIssue {
                        severity: IssueSeverity::Error,
                        message: "invalid https url".into(),
                        source,
                        destination: Some(destination),
                    });
                } else if let HttpsValidationMode::Reachability { .. } = options.https.mode {
                    // Reachability is validated at extraction time only for the target document.
                    // For discovered references, we optionally validate now.
                    if let Ok(url) = Url::parse(&destination) {
                        if let Err(issue) = fetch_url(&url, &options.https.mode).map(|_| ()) {
                            issues.push(ReferenceValidationIssue {
                                source: ReferenceSource {
                                    document: source.document.clone(),
                                    range: source.range.clone(),
                                },
                                ..issue
                            });
                        }
                    }
                }
            }
            DestinationKind::WorkspaceFilesystem => {
                let (path_part, fragment_opt) = split_path_and_fragment(&destination);

                match resolve_markdown_destination_path(target, &path_part, workspace) {
                    Ok(resolved) => {
                        if !resolved.exists() {
                            issues.push(ReferenceValidationIssue {
                                severity: IssueSeverity::Error,
                                message: format!(
                                    "missing filesystem target {}",
                                    path_to_forward_slashes(&resolved)
                                ),
                                source: source.clone(),
                                destination: Some(destination.clone()),
                            });
                        }

                        // Fragment validation only for markdown targets.
                        if is_markdown_path(&resolved) {
                            if let Some(fragment) = fragment_opt {
                                if fragment.is_empty() {
                                    issues.push(ReferenceValidationIssue {
                                        severity: IssueSeverity::Error,
                                        message: "empty fragment is invalid".into(),
                                        source: source.clone(),
                                        destination: Some(destination.clone()),
                                    });
                                } else {
                                    // If this resolves to the current document, validate now.
                                    if let ResolvedDocument::File { path, .. } = target {
                                        if fs::canonicalize(path).ok().as_ref() == Some(&resolved) {
                                            if !heading_slugs.contains(&fragment) {
                                                issues.push(ReferenceValidationIssue {
                                                    severity: IssueSeverity::Error,
                                                    message: format!(
                                                        "fragment '#{fragment}' does not match any heading slug"
                                                    ),
                                                    source: source.clone(),
                                                    destination: Some(destination.clone()),
                                                });
                                            }
                                        } else if options.transitive.enabled {
                                            pending_fragments
                                                .entry(DocumentKey::File(resolved.clone()))
                                                .or_default()
                                                .insert(PendingFragmentCheck {
                                                    source: source.clone(),
                                                    fragment: fragment.clone(),
                                                    destination: destination.clone(),
                                                });
                                        } else {
                                            issues.push(ReferenceValidationIssue {
                                                severity: IssueSeverity::Diagnostic,
                                                message: "cross-document fragment validation skipped (transitive disabled)".into(),
                                                source: source.clone(),
                                                destination: Some(destination.clone()),
                                            });
                                        }
                                    }
                                }
                            }

                            if options.transitive.enabled {
                                markdown_targets.insert(
                                    path_to_forward_slashes(&resolved),
                                    ResolvedDocument::File {
                                        base_dir: resolved
                                            .parent()
                                            .unwrap_or_else(|| Path::new("."))
                                            .to_path_buf(),
                                        path: resolved,
                                    },
                                );
                            }
                        }
                    }
                    Err(msg) => issues.push(ReferenceValidationIssue {
                        severity: IssueSeverity::Error,
                        message: msg,
                        source,
                        destination: Some(destination),
                    }),
                }
            }
            DestinationKind::UnsupportedOrUnknown => {
                issues.push(ReferenceValidationIssue {
                    severity: IssueSeverity::Error,
                    message: "unsupported or invalid destination".into(),
                    source,
                    destination: Some(destination),
                });
            }
        }
    }

    // Deterministic next-doc list.
    let next_docs = markdown_targets.into_values().collect::<Vec<_>>();
    (discovered, issues, next_docs, heading_slugs)
}

fn parse_https_url(input: &str) -> Result<Url, ()> {
    if input.starts_with("https://") {
        Url::parse(input).map_err(|_| ())
    } else {
        Err(())
    }
}

fn classify_destination(destination: &str) -> DestinationKind {
    let trimmed = destination.trim();

    if trimmed.starts_with("spec://")
        || trimmed.starts_with("impl://")
        || trimmed.starts_with("scratch://")
        || trimmed.starts_with("http://")
    {
        return DestinationKind::UnsupportedOrUnknown;
    }

    if trimmed.starts_with('#') {
        return DestinationKind::FragmentOnly;
    }

    if trimmed.starts_with("https://") {
        return DestinationKind::HttpsUrl;
    }

    // Windows path-ish forms are explicitly invalid/unsupported.
    if trimmed.contains('\\') {
        return DestinationKind::UnsupportedOrUnknown;
    }
    if looks_like_windows_drive_path(trimmed) {
        return DestinationKind::UnsupportedOrUnknown;
    }

    // We treat all other bare/relative paths as workspace filesystem references.
    if trimmed.contains("://") {
        return DestinationKind::UnsupportedOrUnknown;
    }

    DestinationKind::WorkspaceFilesystem
}

fn looks_like_windows_drive_path(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' {
        let c0 = bytes[0] as char;
        return c0.is_ascii_alphabetic();
    }
    false
}

fn path_to_forward_slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn is_markdown_path(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()), Some("md"))
}

fn split_path_and_fragment(dest: &str) -> (String, Option<String>) {
    match dest.split_once('#') {
        Some((before_hash, after_hash)) => {
            let path_part = before_hash
                .split_once('?')
                .map(|(p, _)| p)
                .unwrap_or(before_hash)
                .to_string();
            let fragment = after_hash
                .split_once('?')
                .map(|(f, _)| f)
                .unwrap_or(after_hash)
                .to_string();
            (path_part, Some(fragment))
        }
        None => {
            let path_part = dest
                .split_once('?')
                .map(|(p, _)| p)
                .unwrap_or(dest)
                .to_string();
            (path_part, None)
        }
    }
}

fn resolve_markdown_destination_path(
    source: &ResolvedDocument,
    dest: &str,
    workspace: &WorkspacePaths,
) -> Result<PathBuf, String> {
    let dest = dest.trim();
    if dest.is_empty() {
        return Err("empty path destination".into());
    }
    if dest.starts_with('/') {
        return Err("absolute paths are not supported in markdown destinations".into());
    }

    let candidate = Path::new(dest);

    let (path, base_dir) = match source {
        ResolvedDocument::File { base_dir, .. } => (candidate.to_path_buf(), base_dir.as_path()),
        ResolvedDocument::Url { .. } => {
            return Err(
                "filesystem destinations cannot be resolved relative to a URL source".into(),
            );
        }
    };

    let workspace_root =
        fs::canonicalize(workspace.root()).unwrap_or_else(|_| workspace.root().to_path_buf());

    // Use canonical base_dir for consistent comparisons (normalizes 8.3 names and casing on Windows).
    let base_dir = fs::canonicalize(base_dir).unwrap_or_else(|_| base_dir.to_path_buf());

    // Resolve relative to the source artifact directory.
    let resolved = if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    };

    // If the target exists, canonicalize and compare canonical paths.
    if let Ok(canonical) = fs::canonicalize(&resolved) {
        if !canonical.starts_with(&workspace_root) {
            return Err(format!(
                "filesystem destination escapes workspace: {}",
                path_to_forward_slashes(&canonical)
            ));
        }
        return Ok(canonical);
    }

    // Missing target: apply lexical normalization and compare against canonical workspace root.
    let normalized = normalize_path_lexical(&resolved);
    if !normalized.starts_with(&workspace_root) {
        return Err(format!(
            "filesystem destination escapes workspace: {}",
            path_to_forward_slashes(&normalized)
        ));
    }

    Ok(normalized)
}

fn normalize_path_lexical(path: &Path) -> PathBuf {
    use std::ffi::OsString;
    use std::path::Component;

    let mut anchored = PathBuf::new();
    let mut parts: Vec<OsString> = Vec::new();
    let mut is_anchored = false;

    for component in path.components() {
        match component {
            component @ (Component::Prefix(_) | Component::RootDir) => {
                anchored.push(component.as_os_str());
                is_anchored = true;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.pop().is_none() && !is_anchored {
                    parts.push(OsString::from(".."));
                }
            }
            Component::Normal(s) => parts.push(s.to_os_string()),
        }
    }

    for part in parts {
        anchored.push(part);
    }

    anchored
}

fn collect_definitions(root: &markdown::mdast::Node) -> BTreeMap<String, String> {
    use markdown::mdast::Node;

    let mut defs = BTreeMap::new();

    fn walk(node: &Node, defs: &mut BTreeMap<String, String>) {
        match node {
            Node::Definition(def) => {
                defs.insert(def.identifier.clone(), def.url.clone());
            }
            _ => {
                if let Some(children) = node.children() {
                    for child in children {
                        walk(child, defs);
                    }
                }
            }
        }
    }

    walk(root, &mut defs);
    defs
}

struct ExtractedDestination {
    destination: String,
    range: Option<SourceRange>,
    unresolved_reference_identifier: Option<String>,
}

fn extract_destinations(
    root: &markdown::mdast::Node,
    defs: &BTreeMap<String, String>,
) -> Vec<ExtractedDestination> {
    use markdown::mdast::{Node, ReferenceKind};

    let mut out: Vec<ExtractedDestination> = Vec::new();

    fn pos_to_range(pos: &Option<markdown::unist::Position>) -> Option<SourceRange> {
        let pos = pos.as_ref()?;
        Some(SourceRange {
            start: SourcePoint {
                line: pos.start.line as usize,
                column: pos.start.column as usize,
            },
            end: SourcePoint {
                line: pos.end.line as usize,
                column: pos.end.column as usize,
            },
        })
    }

    fn walk(node: &Node, defs: &BTreeMap<String, String>, out: &mut Vec<ExtractedDestination>) {
        match node {
            Node::Link(link) => {
                out.push(ExtractedDestination {
                    destination: link.url.clone(),
                    range: pos_to_range(&link.position),
                    unresolved_reference_identifier: None,
                });
            }
            Node::LinkReference(lr) => {
                let dest = match lr.reference_kind {
                    ReferenceKind::Full | ReferenceKind::Collapsed | ReferenceKind::Shortcut => {
                        defs.get(&lr.identifier).cloned()
                    }
                };

                match dest {
                    Some(url) => out.push(ExtractedDestination {
                        destination: url,
                        range: pos_to_range(&lr.position),
                        unresolved_reference_identifier: None,
                    }),
                    None => out.push(ExtractedDestination {
                        destination: String::new(),
                        range: pos_to_range(&lr.position),
                        unresolved_reference_identifier: Some(lr.identifier.clone()),
                    }),
                }
            }
            Node::Image(_) | Node::ImageReference(_) => {
                // Ignore images.
            }
            _ => {
                if let Some(children) = node.children() {
                    for child in children {
                        walk(child, defs, out);
                    }
                }
            }
        }
    }

    walk(root, defs, &mut out);

    out
}

fn collect_heading_slugs(
    root: &markdown::mdast::Node,
    issues: &mut Vec<ReferenceValidationIssue>,
    document_id: &str,
) -> BTreeSet<String> {
    use markdown::mdast::Node;

    let mut slugs = BTreeSet::new();
    let mut occurrences = BTreeMap::<String, usize>::new();

    fn plain_text(node: &Node, out: &mut String) {
        use markdown::mdast::Node;
        match node {
            Node::Text(t) => out.push_str(&t.value),
            Node::InlineCode(c) => out.push_str(&c.value),
            Node::Link(l) => {
                for child in &l.children {
                    plain_text(child, out);
                }
            }
            Node::LinkReference(lr) => {
                for child in &lr.children {
                    plain_text(child, out);
                }
            }
            Node::Emphasis(e) => {
                for child in &e.children {
                    plain_text(child, out);
                }
            }
            Node::Strong(s) => {
                for child in &s.children {
                    plain_text(child, out);
                }
            }
            Node::Delete(d) => {
                for child in &d.children {
                    plain_text(child, out);
                }
            }
            _ => {
                if let Some(children) = node.children() {
                    for child in children {
                        plain_text(child, out);
                    }
                }
            }
        }
    }

    fn walk(
        node: &Node,
        slugs: &mut BTreeSet<String>,
        occurrences: &mut BTreeMap<String, usize>,
        issues: &mut Vec<ReferenceValidationIssue>,
        document_id: &str,
    ) {
        match node {
            Node::Heading(h) => {
                let mut title = String::new();
                for child in &h.children {
                    plain_text(child, &mut title);
                }

                match slugify_heading(&title) {
                    Some(base) => {
                        let count = occurrences.entry(base.clone()).or_insert(0);
                        let slug = if *count == 0 {
                            base.clone()
                        } else {
                            format!("{base}-{count}")
                        };
                        *count += 1;
                        slugs.insert(slug);
                    }
                    None => {
                        issues.push(ReferenceValidationIssue {
                            severity: IssueSeverity::Diagnostic,
                            message: format!("heading produces empty slug (title: {title:?})"),
                            source: ReferenceSource {
                                document: document_id.to_string(),
                                range: None,
                            },
                            destination: None,
                        });
                    }
                }
            }
            _ => {
                if let Some(children) = node.children() {
                    for child in children {
                        walk(child, slugs, occurrences, issues, document_id);
                    }
                }
            }
        }
    }

    walk(root, &mut slugs, &mut occurrences, issues, document_id);
    slugs
}

fn slugify_heading(title: &str) -> Option<String> {
    // Spec: NFKD -> case fold (approx via lowercase) -> whitespace to spaces -> filter -> hyphenate -> cleanup.
    let normalized = title.nfkd().collect::<String>();
    let lowered = normalized.to_lowercase();

    let mut filtered = String::new();
    for ch in lowered.chars() {
        let mapped = if ch.is_whitespace() { ' ' } else { ch };
        if mapped.is_alphabetic() || mapped.is_numeric() || mapped == ' ' || mapped == '-' {
            filtered.push(mapped);
        }
    }

    // Replace contiguous spaces with hyphen.
    let mut hyphenated = String::new();
    let mut in_space = false;
    for ch in filtered.chars() {
        if ch == ' ' {
            if !in_space {
                hyphenated.push('-');
                in_space = true;
            }
        } else {
            hyphenated.push(ch);
            in_space = false;
        }
    }

    // Collapse contiguous hyphens and trim.
    let mut cleaned = String::new();
    let mut prev_hyphen = false;
    for ch in hyphenated.chars() {
        if ch == '-' {
            if !prev_hyphen {
                cleaned.push('-');
                prev_hyphen = true;
            }
        } else {
            cleaned.push(ch);
            prev_hyphen = false;
        }
    }

    let cleaned = cleaned.trim_matches('-').to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}
