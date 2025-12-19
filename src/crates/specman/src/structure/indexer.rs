use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use unicode_normalization::UnicodeNormalization;

use crate::dependency_tree::ArtifactKind;
use crate::error::SpecmanError;
use crate::front_matter::{ArtifactFrontMatter, optional_front_matter};
use crate::workspace::{WorkspaceLocator, WorkspacePaths};

use super::index::{
    ArtifactKey, ArtifactRecord, ConstraintIdentifier, ConstraintRecord, HeadingIdentifier,
    HeadingRecord, RelationshipEdge, RelationshipKind, WORKSPACE_INDEX_SCHEMA_VERSION,
    WorkspaceIndex,
};

pub trait StructureIndexing: Send + Sync {
    fn build_once(&self) -> Result<WorkspaceIndex, SpecmanError>;
}

/// Filesystem-backed one-shot structure indexer.
#[derive(Clone)]
pub struct FilesystemStructureIndexer<L: WorkspaceLocator> {
    locator: L,
}

impl<L: WorkspaceLocator> FilesystemStructureIndexer<L> {
    pub fn new(locator: L) -> Self {
        Self { locator }
    }

    pub fn build_once_with_workspace(
        &self,
        workspace: &WorkspacePaths,
    ) -> Result<WorkspaceIndex, SpecmanError> {
        build_workspace_index(workspace)
    }
}

impl<L: WorkspaceLocator> StructureIndexing for FilesystemStructureIndexer<L> {
    fn build_once(&self) -> Result<WorkspaceIndex, SpecmanError> {
        let workspace = self.locator.workspace()?;
        build_workspace_index(&workspace)
    }
}

impl<L> StructureIndexing for Arc<L>
where
    L: StructureIndexing,
{
    fn build_once(&self) -> Result<WorkspaceIndex, SpecmanError> {
        (**self).build_once()
    }
}

pub fn build_workspace_index(workspace: &WorkspacePaths) -> Result<WorkspaceIndex, SpecmanError> {
    let mut index = WorkspaceIndex {
        schema_version: WORKSPACE_INDEX_SCHEMA_VERSION,
        workspace_root: workspace.root().to_path_buf(),
        ..Default::default()
    };

    let artifacts = enumerate_canonical_artifact_files(workspace)?;

    let mut artifact_by_path: HashMap<String, ArtifactKey> = HashMap::new();
    let mut pending_heading_refs: Vec<PendingHeadingRef> = Vec::new();
    let mut relationships: Vec<RelationshipEdge> = Vec::new();

    for (kind, path) in artifacts {
        let parsed = parse_artifact(kind, &path, workspace)?;

        relationships.extend(parsed.relationships);
        pending_heading_refs.extend(parsed.pending_heading_refs);

        artifact_by_path.insert(
            parsed.artifact.key.workspace_path.clone(),
            parsed.artifact.key.clone(),
        );

        index
            .artifacts
            .insert(parsed.artifact.key.clone(), parsed.artifact);
        for (id, record) in parsed.headings {
            index.headings.insert(id, record);
        }
        for (id, record) in parsed.constraints {
            index.constraints.insert(id, record);
        }
    }

    // Resolve any references that required the full workspace index.
    let mut resolved_relationships: Vec<RelationshipEdge> = Vec::new();
    for pending in pending_heading_refs {
        match pending.target {
            PendingTarget::IntraDoc { slug } => {
                let to = HeadingIdentifier {
                    artifact: pending.from.artifact.clone(),
                    slug,
                };
                if index.headings.contains_key(&to) {
                    attach_heading_reference(&mut index, &pending.from, &to);
                    resolved_relationships.push(RelationshipEdge {
                        kind: RelationshipKind::HeadingToHeading,
                        from: heading_ref_string(&pending.from),
                        to: heading_ref_string(&to),
                    });
                }
            }
            PendingTarget::InterDoc {
                workspace_path,
                slug,
            } => {
                let Some(artifact_key) = artifact_by_path.get(&workspace_path).cloned() else {
                    continue;
                };
                let to = HeadingIdentifier {
                    artifact: artifact_key,
                    slug,
                };
                if index.headings.contains_key(&to) {
                    attach_heading_reference(&mut index, &pending.from, &to);
                    resolved_relationships.push(RelationshipEdge {
                        kind: RelationshipKind::HeadingToHeading,
                        from: heading_ref_string(&pending.from),
                        to: heading_ref_string(&to),
                    });
                }
            }
            PendingTarget::File { workspace_path } => {
                resolved_relationships.push(RelationshipEdge {
                    kind: RelationshipKind::HeadingToFile,
                    from: heading_ref_string(&pending.from),
                    to: workspace_path,
                });
            }
        }
    }

    relationships.extend(resolved_relationships);
    index.relationships = relationships;

    Ok(index)
}

fn attach_heading_reference(
    index: &mut WorkspaceIndex,
    from: &HeadingIdentifier,
    to: &HeadingIdentifier,
) {
    if let Some(record) = index.headings.get_mut(from) {
        record.referenced_headings.push(to.clone());
    }
}

fn heading_ref_string(id: &HeadingIdentifier) -> String {
    format!("{}#{}", id.artifact.workspace_path, id.slug)
}

fn enumerate_canonical_artifact_files(
    workspace: &WorkspacePaths,
) -> Result<Vec<(ArtifactKind, PathBuf)>, SpecmanError> {
    let mut out: Vec<(ArtifactKind, PathBuf)> = Vec::new();

    // spec/*/spec.md
    let spec_dir = workspace.spec_dir();
    if spec_dir.is_dir() {
        let mut entries = read_dir_sorted(&spec_dir)?;
        for entry in entries.drain(..) {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let file = dir.join("spec.md");
            if file.is_file() {
                out.push((ArtifactKind::Specification, file));
            }
        }
    }

    // impl/*/impl.md
    let impl_dir = workspace.impl_dir();
    if impl_dir.is_dir() {
        let mut entries = read_dir_sorted(&impl_dir)?;
        for entry in entries.drain(..) {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let file = dir.join("impl.md");
            if file.is_file() {
                out.push((ArtifactKind::Implementation, file));
            }
        }
    }

    // .specman/scratchpad/*/scratch.md
    let scratch_dir = workspace.scratchpad_dir();
    if scratch_dir.is_dir() {
        let mut entries = read_dir_sorted(&scratch_dir)?;
        for entry in entries.drain(..) {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let file = dir.join("scratch.md");
            if file.is_file() {
                out.push((ArtifactKind::ScratchPad, file));
            }
        }
    }

    // Canonicalize + enforce workspace boundary deterministically.
    let mut canon: Vec<(ArtifactKind, PathBuf)> = Vec::with_capacity(out.len());
    for (kind, file) in out {
        let canonical = fs::canonicalize(&file)?;
        if !canonical.starts_with(workspace.root()) {
            return Err(SpecmanError::Workspace(format!(
                "indexed artifact {} escapes workspace {}",
                canonical.display(),
                workspace.root().display()
            )));
        }
        canon.push((kind, canonical));
    }

    canon.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(canon)
}

fn read_dir_sorted(dir: &Path) -> Result<Vec<fs::DirEntry>, SpecmanError> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.filter_map(Result::ok).collect();
    entries.sort_by_key(|entry| entry.file_name());
    Ok(entries)
}

struct ParsedArtifact {
    artifact: ArtifactRecord,
    headings: BTreeMap<HeadingIdentifier, HeadingRecord>,
    constraints: BTreeMap<ConstraintIdentifier, ConstraintRecord>,
    relationships: Vec<RelationshipEdge>,
    pending_heading_refs: Vec<PendingHeadingRef>,
}

#[derive(Clone, Debug)]
struct PendingHeadingRef {
    from: HeadingIdentifier,
    target: PendingTarget,
}

#[derive(Clone, Debug)]
enum PendingTarget {
    IntraDoc {
        slug: String,
    },
    InterDoc {
        workspace_path: String,
        slug: String,
    },
    File {
        workspace_path: String,
    },
}

fn parse_artifact(
    kind: ArtifactKind,
    canonical_path: &Path,
    workspace: &WorkspacePaths,
) -> Result<ParsedArtifact, SpecmanError> {
    if !canonical_path.starts_with(workspace.root()) {
        return Err(SpecmanError::Workspace(format!(
            "artifact {} escapes workspace {}",
            canonical_path.display(),
            workspace.root().display()
        )));
    }

    let workspace_path =
        workspace_relative_path(workspace.root(), canonical_path).ok_or_else(|| {
            SpecmanError::Workspace(format!(
                "unable to compute workspace-relative path for {}",
                canonical_path.display()
            ))
        })?;

    let artifact_key = ArtifactKey {
        kind,
        workspace_path: workspace_path.clone(),
    };

    let raw_bytes = fs::read(canonical_path)?;
    let raw = String::from_utf8(raw_bytes).map_err(|err| {
        SpecmanError::Workspace(format!(
            "invalid UTF-8 in {}: {err}",
            canonical_path.display()
        ))
    })?;

    let normalized = normalize_newlines(&raw);

    let (yaml_opt, _missing) = optional_front_matter(&normalized);
    let (front_matter, body) = if let Some(yaml) = yaml_opt {
        let front = ArtifactFrontMatter::from_yaml_str(yaml).map_err(|err| {
            err.context(format!("front matter parse failed for {workspace_path}"))
        })?;
        (
            Some(front),
            body_after_front_matter(&normalized).unwrap_or(&normalized),
        )
    } else {
        (None, normalized.as_str())
    };

    let mut relationships: Vec<RelationshipEdge> = Vec::new();
    let mut pending_heading_refs: Vec<PendingHeadingRef> = Vec::new();

    let artifact = ArtifactRecord {
        key: artifact_key.clone(),
        absolute_path: canonical_path.to_path_buf(),
        front_matter,
    };

    // Parse headings + content.
    let (mut headings, constraints, mut local_relationships, mut pending) =
        parse_markdown_structure(&artifact_key, canonical_path, workspace, body)?;

    relationships.append(&mut local_relationships);
    pending_heading_refs.append(&mut pending);

    // Derive constraint->heading relationships.
    for record in constraints.values() {
        relationships.push(RelationshipEdge {
            kind: RelationshipKind::ConstraintToHeading,
            from: format!("{}!{}", record.id.artifact.workspace_path, record.id.group),
            to: heading_ref_string(&record.heading),
        });
    }

    // Add explicit heading->artifact edges and parent/child edges.
    for record in headings.values() {
        relationships.push(RelationshipEdge {
            kind: RelationshipKind::HeadingToArtifact,
            from: heading_ref_string(&record.id),
            to: artifact_key.workspace_path.clone(),
        });
        if let Some(parent) = &record.parent {
            relationships.push(RelationshipEdge {
                kind: RelationshipKind::ParentToChild,
                from: heading_ref_string(parent),
                to: heading_ref_string(&record.id),
            });
        }
    }

    // Attach children vectors.
    let mut children: HashMap<HeadingIdentifier, Vec<HeadingIdentifier>> = HashMap::new();
    for record in headings.values() {
        if let Some(parent) = &record.parent {
            children
                .entry(parent.clone())
                .or_default()
                .push(record.id.clone());
        }
    }
    for (parent, mut kids) in children {
        kids.sort_by_key(|id| headings.get(id).map(|h| h.order).unwrap_or(usize::MAX));
        if let Some(parent_record) = headings.get_mut(&parent) {
            parent_record.children = kids;
        }
    }

    Ok(ParsedArtifact {
        artifact,
        headings,
        constraints,
        relationships,
        pending_heading_refs,
    })
}

fn body_after_front_matter<'a>(full: &'a str) -> Option<&'a str> {
    // Re-use the splitter but tolerate missing.
    match crate::front_matter::split_front_matter(full) {
        Ok(split) => Some(split.body),
        Err(_) => None,
    }
}

fn normalize_newlines(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

fn workspace_relative_path(root: &Path, absolute: &Path) -> Option<String> {
    let relative = absolute.strip_prefix(root).ok()?;
    Some(relative.to_string_lossy().replace('\\', "/"))
}

fn parse_markdown_structure(
    artifact: &ArtifactKey,
    canonical_path: &Path,
    workspace: &WorkspacePaths,
    body: &str,
) -> Result<
    (
        BTreeMap<HeadingIdentifier, HeadingRecord>,
        BTreeMap<ConstraintIdentifier, ConstraintRecord>,
        Vec<RelationshipEdge>,
        Vec<PendingHeadingRef>,
    ),
    SpecmanError,
> {
    let mut headings: BTreeMap<HeadingIdentifier, HeadingRecord> = BTreeMap::new();
    let mut constraints: BTreeMap<ConstraintIdentifier, ConstraintRecord> = BTreeMap::new();
    let mut relationships: Vec<RelationshipEdge> = Vec::new();
    let mut pending_refs: Vec<PendingHeadingRef> = Vec::new();

    let mut slug_seen: HashMap<String, (usize, String)> = HashMap::new();
    let mut constraint_seen: HashSet<String> = HashSet::new();

    let mut fence: Option<FenceState> = None;

    let mut order: usize = 0;
    let mut heading_stack: Vec<(u8, HeadingIdentifier)> = Vec::new();
    let mut current: Option<HeadingIdentifier> = None;

    // We build content incrementally for the active heading.
    let mut content_buffers: HashMap<HeadingIdentifier, Vec<String>> = HashMap::new();

    let dir = canonical_path
        .parent()
        .ok_or_else(|| SpecmanError::Workspace("artifact has no parent directory".into()))?;

    for (idx, raw_line) in body.lines().enumerate() {
        let line_no = idx + 1;

        let line = raw_line;

        if let Some(updated) = fence_update(fence.as_ref(), line) {
            fence = updated;
        }

        if fence.is_none() {
            if let Some((level, title)) = parse_atx_heading(line) {
                let slug = heading_slug(&title).map_err(|err| {
                    SpecmanError::Workspace(format!(
                        "invalid heading slug in {} at line {line_no}: {err}",
                        artifact.workspace_path
                    ))
                })?;

                if let Some((prev_line, prev_title)) = slug_seen.get(&slug).cloned() {
                    return Err(SpecmanError::Workspace(format!(
                        "duplicate heading slug '{slug}' in {}: line {prev_line} ('{prev_title}') and line {line_no} ('{title}')",
                        artifact.workspace_path
                    )));
                }
                slug_seen.insert(slug.clone(), (line_no, title.clone()));

                let id = HeadingIdentifier {
                    artifact: artifact.clone(),
                    slug,
                };

                // Determine parent based on heading levels.
                while let Some((lvl, _)) = heading_stack.last() {
                    if *lvl < level {
                        break;
                    }
                    heading_stack.pop();
                }
                let parent = heading_stack.last().map(|(_, id)| id.clone());

                let record = HeadingRecord {
                    id: id.clone(),
                    level,
                    title: title.clone(),
                    order,
                    parent,
                    children: Vec::new(),
                    content: String::new(),
                    referenced_headings: Vec::new(),
                };
                order += 1;

                headings.insert(id.clone(), record);
                content_buffers.insert(id.clone(), Vec::new());

                heading_stack.push((level, id.clone()));
                current = Some(id);

                continue;
            }
        }

        // Non-heading line: attach to current heading content.
        if let Some(current_id) = current.clone() {
            content_buffers
                .entry(current_id)
                .or_default()
                .push(line.to_string());
        }
    }

    // Finalize content strings + extract constraints/links per heading.
    let mut heading_order: Vec<HeadingIdentifier> = headings.keys().cloned().collect();
    heading_order.sort_by_key(|id| headings.get(id).map(|h| h.order).unwrap_or(usize::MAX));

    for id in heading_order {
        let Some(record) = headings.get_mut(&id) else {
            continue;
        };
        let content_lines = content_buffers.remove(&id).unwrap_or_default();
        record.content = join_lines(&content_lines);

        let (mut heading_constraints, mut refs, mut rels) =
            extract_constraints_and_links(&id, &content_lines, dir, workspace)?;

        for (group, line_no, first_group_slug, nearest_heading) in heading_constraints.drain(..) {
            if !constraint_seen.insert(group.clone()) {
                return Err(SpecmanError::Workspace(format!(
                    "duplicate constraint group '{group}' in {}",
                    artifact.workspace_path
                )));
            }

            let owning_heading = if headings.contains_key(&HeadingIdentifier {
                artifact: artifact.clone(),
                slug: first_group_slug.clone(),
            }) {
                HeadingIdentifier {
                    artifact: artifact.clone(),
                    slug: first_group_slug,
                }
            } else {
                nearest_heading
            };

            let cid = ConstraintIdentifier {
                artifact: artifact.clone(),
                group: group.clone(),
            };

            let cref = ConstraintRecord {
                id: cid.clone(),
                heading: owning_heading,
                line: line_no,
            };
            constraints.insert(cid, cref);
        }

        pending_refs.append(&mut refs);
        relationships.append(&mut rels);
    }

    Ok((headings, constraints, relationships, pending_refs))
}

fn join_lines(lines: &[String]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[derive(Clone, Debug)]
struct FenceState {
    ch: char,
    len: usize,
}

fn fence_update(current: Option<&FenceState>, line: &str) -> Option<Option<FenceState>> {
    let trimmed = line.strip_prefix("   ").unwrap_or(line);
    let (ch, run) = if trimmed.starts_with("```") {
        ('`', count_run(trimmed, '`'))
    } else if trimmed.starts_with("~~~") {
        ('~', count_run(trimmed, '~'))
    } else {
        return None;
    };

    if current.is_none() {
        return Some(Some(FenceState {
            ch,
            len: run.max(3),
        }));
    }

    let cur = current.unwrap();
    if cur.ch == ch && count_run(trimmed, ch) >= cur.len {
        return Some(None);
    }

    Some(Some(cur.clone()))
}

fn count_run(s: &str, ch: char) -> usize {
    s.chars().take_while(|c| *c == ch).count()
}

fn parse_atx_heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.strip_prefix("   ").unwrap_or(line);
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }

    let after = trimmed.get(hashes..)?;
    let after = after.strip_prefix(' ')?;
    let title = after.trim_end().to_string();
    Some((hashes as u8, title))
}

fn extract_constraints_and_links(
    heading: &HeadingIdentifier,
    content_lines: &[String],
    base_dir: &Path,
    workspace: &WorkspacePaths,
) -> Result<
    (
        Vec<(String, usize, String, HeadingIdentifier)>,
        Vec<PendingHeadingRef>,
        Vec<RelationshipEdge>,
    ),
    SpecmanError,
> {
    let mut constraints: Vec<(String, usize, String, HeadingIdentifier)> = Vec::new();
    let mut pending: Vec<PendingHeadingRef> = Vec::new();
    let mut relationships: Vec<RelationshipEdge> = Vec::new();

    let mut fence: Option<FenceState> = None;

    for (idx, raw) in content_lines.iter().enumerate() {
        let line_no = idx + 1;

        if let Some(updated) = fence_update(fence.as_ref(), raw) {
            fence = updated;
        }

        if fence.is_none() {
            let trimmed = raw.trim();
            if is_constraint_identifier_line(trimmed) {
                let group = trimmed
                    .trim_start_matches('!')
                    .trim_end_matches(':')
                    .to_string();

                let first_group = group.split('.').next().unwrap_or("").to_string();
                constraints.push((group, line_no, first_group, heading.clone()));
            }

            for dest in extract_inline_link_destinations(raw) {
                if let Some(rest) = dest.strip_prefix('#') {
                    pending.push(PendingHeadingRef {
                        from: heading.clone(),
                        target: PendingTarget::IntraDoc {
                            slug: rest.to_string(),
                        },
                    });
                    continue;
                }

                if let Some((path_part, frag)) = dest.split_once('#') {
                    if let Some(workspace_path) =
                        resolve_workspace_link_path(path_part, base_dir, workspace)?
                    {
                        pending.push(PendingHeadingRef {
                            from: heading.clone(),
                            target: PendingTarget::InterDoc {
                                workspace_path,
                                slug: frag.to_string(),
                            },
                        });
                    }
                    continue;
                }

                // Bare file link.
                if let Some(workspace_path) =
                    resolve_workspace_link_path(dest.as_str(), base_dir, workspace)?
                {
                    pending.push(PendingHeadingRef {
                        from: heading.clone(),
                        target: PendingTarget::File { workspace_path },
                    });
                }
            }
        }
    }

    // Also emit local edges for now (resolved later into index.headings).
    for pend in &pending {
        match &pend.target {
            PendingTarget::IntraDoc { slug } => relationships.push(RelationshipEdge {
                kind: RelationshipKind::HeadingToHeading,
                from: heading_ref_string(&pend.from),
                to: format!("{}#{}", pend.from.artifact.workspace_path, slug),
            }),
            PendingTarget::InterDoc {
                workspace_path,
                slug,
            } => relationships.push(RelationshipEdge {
                kind: RelationshipKind::HeadingToHeading,
                from: heading_ref_string(&pend.from),
                to: format!("{workspace_path}#{slug}"),
            }),
            PendingTarget::File { workspace_path } => relationships.push(RelationshipEdge {
                kind: RelationshipKind::HeadingToFile,
                from: heading_ref_string(&pend.from),
                to: workspace_path.clone(),
            }),
        }
    }

    Ok((constraints, pending, relationships))
}

fn is_constraint_identifier_line(trimmed: &str) -> bool {
    if !trimmed.starts_with('!') || !trimmed.ends_with(':') {
        return false;
    }
    if trimmed.contains(' ') || trimmed.contains('\t') {
        return false;
    }
    let core = trimmed.trim_start_matches('!').trim_end_matches(':');
    let groups: Vec<_> = core.split('.').filter(|s| !s.is_empty()).collect();
    groups.len() >= 2
}

fn extract_inline_link_destinations(line: &str) -> Vec<String> {
    // Minimal CommonMark inline link extractor: [text](dest)
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            if let Some(close_bracket) = memchr(bytes, b']', i + 1) {
                if close_bracket + 1 < bytes.len() && bytes[close_bracket + 1] == b'(' {
                    if let Some(close_paren) = memchr(bytes, b')', close_bracket + 2) {
                        let dest = &line[close_bracket + 2..close_paren];
                        let dest = dest.trim();
                        if !dest.is_empty() {
                            out.push(dest.to_string());
                        }
                        i = close_paren + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    out
}

fn memchr(haystack: &[u8], needle: u8, start: usize) -> Option<usize> {
    haystack.iter().enumerate().skip(start).find_map(
        |(i, b)| {
            if *b == needle { Some(i) } else { None }
        },
    )
}

fn resolve_workspace_link_path(
    reference: &str,
    base_dir: &Path,
    workspace: &WorkspacePaths,
) -> Result<Option<String>, SpecmanError> {
    let trimmed = reference.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    // Remote links are ignored by this parsing-only index.
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(None);
    }

    // Unknown schemes are ignored.
    if trimmed.contains("://") {
        return Ok(None);
    }

    let candidate = Path::new(trimmed);

    let absolute = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base_dir.join(candidate)
    };

    let normalized = lexical_normalize(&absolute);
    if !normalized.starts_with(workspace.root()) {
        return Err(SpecmanError::Workspace(format!(
            "link {} escapes workspace {}",
            normalized.display(),
            workspace.root().display()
        )));
    }

    // If the target exists, prefer canonical path for stable workspace-relative output.
    let stable = if normalized.exists() {
        fs::canonicalize(&normalized).unwrap_or(normalized)
    } else {
        normalized
    };

    Ok(workspace_relative_path(workspace.root(), &stable))
}

fn lexical_normalize(path: &Path) -> PathBuf {
    // Lexically resolves '.' and '..' without requiring filesystem access.
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn heading_slug(title_markdown: &str) -> Result<String, String> {
    let plain = heading_inline_to_plain_text(title_markdown);

    let normalized: String = plain.nfkd().collect::<String>();
    let lower = normalized.to_lowercase();

    let mut filtered = String::new();
    for ch in lower.chars() {
        let mapped = if ch.is_whitespace() { ' ' } else { ch };
        if mapped == '-' || mapped == ' ' || mapped.is_alphanumeric() {
            filtered.push(mapped);
        }
    }

    let mut out = String::new();
    let mut last_was_hyphen = false;
    let mut in_space = false;

    for ch in filtered.chars() {
        if ch == ' ' {
            in_space = true;
            continue;
        }

        if in_space {
            if !out.is_empty() {
                out.push('-');
                last_was_hyphen = true;
            }
            in_space = false;
        }

        if ch == '-' {
            if !out.is_empty() && !last_was_hyphen {
                out.push('-');
                last_was_hyphen = true;
            }
            continue;
        }

        out.push(ch);
        last_was_hyphen = false;
    }

    while out.starts_with('-') {
        out.remove(0);
    }
    while out.ends_with('-') {
        out.pop();
    }

    if out.is_empty() {
        return Err(format!(
            "heading title '{title_markdown}' produced an empty slug"
        ));
    }

    Ok(out)
}

fn heading_inline_to_plain_text(input: &str) -> String {
    // Minimal inline-stripping:
    // - Convert [text](url) -> text
    // - Remove backticks and emphasis markers
    let mut out = String::new();
    let bytes = input.as_bytes();
    let mut i: usize = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'[' => {
                if let Some(close_bracket) = memchr(bytes, b']', i + 1) {
                    if close_bracket + 1 < bytes.len() && bytes[close_bracket + 1] == b'(' {
                        if let Some(close_paren) = memchr(bytes, b')', close_bracket + 2) {
                            out.push_str(&input[i + 1..close_bracket]);
                            i = close_paren + 1;
                            continue;
                        }
                    }
                }
                // Fallback: treat '[' as literal.
                out.push('[');
                i += 1;
            }
            b'`' | b'*' | b'_' => {
                i += 1;
            }
            _ => {
                let ch = input[i..].chars().next().unwrap();
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    out
}
