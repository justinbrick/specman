use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use specman::WorkspacePaths;

use crate::completion::matching::fuzzy_rank_strings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArtifactClass {
    Spec,
    Impl,
}

#[derive(Debug, Default)]
pub(crate) struct CompletionIndex {
    pub(crate) specs: Vec<String>,
    pub(crate) impls: Vec<String>,
    pub(crate) warnings: Vec<String>,
}

pub(crate) fn build_index(workspace: &WorkspacePaths) -> CompletionIndex {
    let mut index = CompletionIndex::default();
    index.specs = collect_handles(
        workspace.spec_dir().as_path(),
        "spec.md",
        ArtifactClass::Spec,
        &mut index.warnings,
    );
    index.impls = collect_handles(
        workspace.impl_dir().as_path(),
        "impl.md",
        ArtifactClass::Impl,
        &mut index.warnings,
    );
    index
}

pub(crate) fn complete_constraint_ids(
    workspace: &WorkspacePaths,
    artifact_name: &str,
    current: &str,
) -> Vec<String> {
    if artifact_name.trim().is_empty() {
        return Vec::new();
    }

    let path = workspace
        .spec_dir()
        .join(artifact_name.trim())
        .join("spec.md");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return Vec::new(),
    };

    let mut ids = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('!') {
            continue;
        }

        let Some((raw, _)) = trimmed[1..].split_once(':') else {
            continue;
        };
        let id = raw.trim();
        if id.is_empty() || id.contains('/') || id.chars().any(char::is_whitespace) {
            continue;
        }
        ids.insert(id.to_string());
    }

    fuzzy_rank_strings(ids.into_iter().collect(), current)
}

fn collect_handles(
    root: &Path,
    artifact_file: &str,
    class: ArtifactClass,
    warnings: &mut Vec<String>,
) -> Vec<String> {
    let mut out = Vec::new();

    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) => {
            warnings.push(format!(
                "completion index degraded: failed to read '{}': {err}",
                root.display()
            ));
            return out;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if !is_slug_like(&name) {
            warnings.push(format!(
                "completion index degraded: skipped invalid artifact directory '{}'; expected lowercase slug",
                path.display()
            ));
            continue;
        }

        let marker = path.join(artifact_file);
        if !marker.exists() {
            warnings.push(format!(
                "completion index degraded: skipped '{}' because '{}' is missing",
                path.display(),
                artifact_file
            ));
            continue;
        }

        let handle = match class {
            ArtifactClass::Spec => format!("spec://{name}"),
            ArtifactClass::Impl => format!("impl://{name}"),
        };
        out.push(handle);
    }

    out.sort_unstable();
    out.dedup();
    out
}

fn is_slug_like(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}
