use std::path::Path;

use crate::dependency_tree::{ArtifactId, ArtifactKind, normalize_persisted_reference};
use crate::error::SpecmanError;
use crate::metadata::frontmatter::{
    self, ArtifactFrontMatter, ArtifactIdentityFields, DependencyEntry, FrontMatterKind,
    ImplementationFrontMatter, ReferenceEntry, ScratchFrontMatter, SpecificationFrontMatter,
};
use crate::metadata::update_model::{
    FrontMatterUpdate, IdentityUpdate, ImplementationUpdate, ScratchUpdate, SpecificationUpdate,
};
use crate::workspace::WorkspacePaths;

/// Applies a FrontMatterUpdate to an existing Markdown document.
pub fn apply_front_matter_update(
    artifact: &ArtifactId,
    artifact_path: &Path,
    workspace: &WorkspacePaths,
    raw_document: &str,
    update: &FrontMatterUpdate,
    persist: bool,
) -> Result<(String, bool), SpecmanError> {
    let parent_dir = artifact_path.parent().ok_or_else(|| {
        SpecmanError::Workspace(format!(
            "artifact {} has no parent directory",
            artifact_path.display()
        ))
    })?;

    // Parse existing
    let split = frontmatter::split_front_matter(raw_document)?;
    let mut front: ArtifactFrontMatter = ArtifactFrontMatter::from_yaml_str(split.yaml)?;

    // Ensure we are working on expected kind
    ensure_kind_matches(artifact, &front)?;

    let mut mutated = false;

    // Apply Updates
    match (&mut front, update) {
        (ArtifactFrontMatter::Specification(fm), FrontMatterUpdate::Specification(up)) => {
            mutated |= update_spec(fm, up, parent_dir, workspace)?;
        }
        (ArtifactFrontMatter::Implementation(fm), FrontMatterUpdate::Implementation(up)) => {
            mutated |= update_impl(fm, up, parent_dir, workspace)?;
        }
        (ArtifactFrontMatter::Scratch(fm), FrontMatterUpdate::Scratch(up)) => {
            mutated |= update_scratch(fm, up, parent_dir, workspace)?;
        }
        // If we get here, it means there is a mismatch between formatting and update kind.
        // E.g. trying to apply ScratchUpdate to SpecificationFrontMatter.
        _ => {
            return Err(SpecmanError::Template(
                "FrontMatterUpdate kind does not match artifact kind".into(),
            ));
        }
    }

    if mutated || persist {
        if persist {
            crate::metadata::mutation::write_artifact_front_matter(artifact_path, &front)?;
        }

        // Reconstruct document string
        let yaml_str = serde_yaml::to_string(&front)
            .map_err(|e| SpecmanError::Serialization(e.to_string()))?;
        let yaml_clean = yaml_str.trim_start_matches("---").trim_start();
        let updated_doc = format!("---\n{}\n---\n{}", yaml_clean.trim_end(), split.body);

        return Ok((updated_doc, mutated));
    }

    Ok((raw_document.to_string(), false))
}

fn update_identity(front: &mut ArtifactIdentityFields, update: &IdentityUpdate) -> bool {
    let mut changed = false;
    if let Some(val) = &update.name {
        if front.name.as_ref() != Some(val) {
            front.name = Some(val.clone());
            changed = true;
        }
    }
    if let Some(val) = &update.title {
        if front.title.as_ref() != Some(val) {
            front.title = Some(val.clone());
            changed = true;
        }
    }
    if let Some(val) = &update.description {
        if front.description.as_ref() != Some(val) {
            front.description = Some(val.clone());
            changed = true;
        }
    }
    if let Some(val) = &update.version {
        if front.version.as_ref() != Some(val) {
            front.version = Some(val.clone());
            changed = true;
        }
    }
    if let Some(val) = &update.tags {
        if &front.tags != val {
            front.tags = val.clone();
            changed = true;
        }
    }
    changed
}

fn update_spec(
    front: &mut SpecificationFrontMatter,
    update: &SpecificationUpdate,
    parent: &Path,
    workspace: &WorkspacePaths,
) -> Result<bool, SpecmanError> {
    let mut changed = update_identity(&mut front.identity, &update.identity);

    if let Some(val) = update.requires_implementation {
        if front.requires_implementation != Some(val) {
            front.requires_implementation = Some(val);
            changed = true;
        }
    }

    if let Some(deps) = &update.dependencies {
        let normalized = normalize_dependencies(deps, parent, workspace)?;
        if dependencies_changed(&front.dependencies, &normalized) {
            front.dependencies = normalized;
            changed = true;
        }
    }

    Ok(changed)
}

fn update_impl(
    front: &mut ImplementationFrontMatter,
    update: &ImplementationUpdate,
    parent: &Path,
    workspace: &WorkspacePaths,
) -> Result<bool, SpecmanError> {
    let mut changed = update_identity(&mut front.identity, &update.identity);

    if let Some(val) = &update.spec {
        let normalized = normalize_persisted_reference(val, parent, workspace)?;
        if front.spec.as_ref() != Some(&normalized) {
            front.spec = Some(normalized);
            changed = true;
        }
    }

    if let Some(val) = &update.location {
        if front.location.as_ref() != Some(val) {
            front.location = Some(val.clone());
            changed = true;
        }
    }

    if let Some(deps) = &update.dependencies {
        let normalized = normalize_dependencies(deps, parent, workspace)?;
        if dependencies_changed(&front.dependencies, &normalized) {
            front.dependencies = normalized;
            changed = true;
        }
    }

    if let Some(refs) = &update.references {
        let normalized = normalize_references(refs, parent, workspace)?;
        if references_changed(&front.references, &normalized) {
            front.references = normalized;
            changed = true;
        }
    }

    Ok(changed)
}

fn update_scratch(
    front: &mut ScratchFrontMatter,
    update: &ScratchUpdate,
    _parent: &Path,
    workspace: &WorkspacePaths,
) -> Result<bool, SpecmanError> {
    let mut changed = update_identity(&mut front.identity, &update.identity);

    if let Some(val) = &update.branch {
        if front.branch.as_ref() != Some(val) {
            front.branch = Some(val.clone());
            changed = true;
        }
    }

    if let Some(val) = &update.work_type {
        let old_json = serde_json::to_value(&front.work_type).unwrap_or_default();
        let new_json = serde_json::to_value(&Some(val.clone())).unwrap_or_default();
        if old_json != new_json {
            front.work_type = Some(val.clone());
            changed = true;
        }
    }

    if let Some(deps) = &update.dependencies {
        let normalized = normalize_dependencies(deps, workspace.root(), workspace)?;
        if dependencies_changed(&front.dependencies, &normalized) {
            front.dependencies = normalized;
            changed = true;
        }
    }

    Ok(changed)
}

fn normalize_dependencies(
    deps: &[DependencyEntry],
    base: &Path,
    workspace: &WorkspacePaths,
) -> Result<Vec<DependencyEntry>, SpecmanError> {
    let mut normalized = Vec::with_capacity(deps.len());
    for dep in deps {
        match dep {
            DependencyEntry::Simple(s) => {
                let r = normalize_persisted_reference(s, base, workspace)?;
                normalized.push(DependencyEntry::Simple(r));
            }
            DependencyEntry::Detailed(obj) => {
                let r = normalize_persisted_reference(&obj.reference, base, workspace)?;
                let mut new_obj = obj.clone();
                new_obj.reference = r;
                normalized.push(DependencyEntry::Detailed(new_obj));
            }
        }
    }
    Ok(normalized)
}

fn normalize_references(
    refs: &[ReferenceEntry],
    base: &Path,
    workspace: &WorkspacePaths,
) -> Result<Vec<ReferenceEntry>, SpecmanError> {
    let mut normalized = Vec::with_capacity(refs.len());
    for r in refs {
        let ref_path = normalize_persisted_reference(&r.reference, base, workspace)?;
        let mut new_r = r.clone();
        new_r.reference = ref_path;
        normalized.push(new_r);
    }
    Ok(normalized)
}

fn dependencies_changed(old: &[DependencyEntry], new: &[DependencyEntry]) -> bool {
    serde_json::to_string(old).unwrap_or_default() != serde_json::to_string(new).unwrap_or_default()
}

fn references_changed(old: &[ReferenceEntry], new: &[ReferenceEntry]) -> bool {
    serde_json::to_string(old).unwrap_or_default() != serde_json::to_string(new).unwrap_or_default()
}

fn ensure_kind_matches(
    artifact: &ArtifactId,
    front: &ArtifactFrontMatter,
) -> Result<(), SpecmanError> {
    let fm_kind = front.kind();
    if fm_kind != extract_front_matter_kind(&artifact.kind) {
        return Err(SpecmanError::Template(format!(
            "artifact kind mismatch: id={:?}, front_matter={:?}",
            artifact.kind, fm_kind
        )));
    }
    Ok(())
}

fn extract_front_matter_kind(kind: &ArtifactKind) -> FrontMatterKind {
    match kind {
        ArtifactKind::Specification => FrontMatterKind::Specification,
        ArtifactKind::Implementation => FrontMatterKind::Implementation,
        ArtifactKind::ScratchPad => FrontMatterKind::ScratchPad,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dependency_tree::{ArtifactId, ArtifactKind};
    use crate::metadata::update_model::{IdentityUpdate, SpecificationUpdate};
    use std::path::PathBuf;

    #[test]
    fn test_update_spec_identity() {
        let doc = "---\nname: old-name\n---\nbody content";
        let artifact_id = ArtifactId {
            kind: ArtifactKind::Specification,
            name: "old-name".to_string(),
        };
        // Fake path
        let path = PathBuf::from("/tmp/specman/spec/old-name/spec.md");
        let root = PathBuf::from("/tmp/specman");
        let workspace = WorkspacePaths::new(root.clone(), root.join(".specman"));

        let update = FrontMatterUpdate::Specification(SpecificationUpdate {
            identity: IdentityUpdate {
                name: Some("new-name".to_string()),
                ..Default::default()
            },
            ..Default::default()
        });

        let (new_doc, mutated) = apply_front_matter_update(
            &artifact_id,
            &path,
            &workspace, // fake
            doc,
            &update,
            false
        ).expect("update");

        assert!(mutated);
        assert!(new_doc.contains("name: new-name"));
        assert!(!new_doc.contains("name: old-name"));
        assert!(new_doc.contains("body content"));
    }

    #[test]
    fn test_update_mismatch() {
        let doc = "---\nname: spec\n---\nbody";
        let artifact_id = ArtifactId {
            kind: ArtifactKind::Specification,
            name: "spec".to_string(),
        };
        let path = PathBuf::from("/tmp/specman/spec/spec.md");
        let root = PathBuf::from("/tmp/specman");
        let workspace = WorkspacePaths::new(root.clone(), root.join(".specman"));

        let update = FrontMatterUpdate::Scratch(Default::default());

        let err = apply_front_matter_update(
            &artifact_id,
            &path,
            &workspace, // fake
            doc,
            &update,
            false
        ).unwrap_err();

        match err {
            SpecmanError::Template(msg) => assert!(msg.contains("kind does not match")),
            _ => panic!("unexpected error: {:?}", err),
        }
    }
}
