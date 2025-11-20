use std::fs;
use std::path::{Path, PathBuf};

use crate::dependency_tree::{ArtifactId, ArtifactKind};
use crate::error::SpecmanError;
use crate::template::RenderedTemplate;
use crate::workspace::{WorkspaceLocator, WorkspacePaths};

/// Result of persisting a rendered template to the workspace filesystem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistedArtifact {
    pub artifact: ArtifactId,
    pub path: PathBuf,
    pub workspace: WorkspacePaths,
}

/// Writes rendered templates into canonical workspace locations.
pub struct WorkspacePersistence<L: WorkspaceLocator> {
    locator: L,
}

impl<L: WorkspaceLocator> WorkspacePersistence<L> {
    pub fn new(locator: L) -> Self {
        Self { locator }
    }

    /// Persists the rendered template to the canonical path defined by the artifact kind.
    pub fn persist(
        &self,
        artifact: &ArtifactId,
        rendered: &RenderedTemplate,
    ) -> Result<PersistedArtifact, SpecmanError> {
        ensure_rendered_tokens_resolved(&rendered.body)?;
        ensure_safe_name(&artifact.name)?;

        let workspace = self.locator.workspace()?;
        let target_path = resolve_target_path(artifact, &workspace)?;
        write_body(&target_path, &rendered.body)?;

        Ok(PersistedArtifact {
            artifact: artifact.clone(),
            path: target_path,
            workspace,
        })
    }
}

fn resolve_target_path(
    artifact: &ArtifactId,
    workspace: &WorkspacePaths,
) -> Result<PathBuf, SpecmanError> {
    let (base_dir, file_name) = match artifact.kind {
        ArtifactKind::Specification => (workspace.spec_dir(), "spec.md"),
        ArtifactKind::Implementation => (workspace.impl_dir(), "impl.md"),
        ArtifactKind::ScratchPad => (workspace.scratchpad_dir(), "scratch.md"),
    };

    let folder = base_dir.join(&artifact.name);
    Ok(folder.join(file_name))
}

fn write_body(path: &Path, body: &str) -> Result<(), SpecmanError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, body)?;
    Ok(())
}

fn ensure_rendered_tokens_resolved(body: &str) -> Result<(), SpecmanError> {
    if body.contains("{{") {
        return Err(SpecmanError::Template(
            "rendered output still contains template tokens".into(),
        ));
    }
    Ok(())
}

fn ensure_safe_name(name: &str) -> Result<(), SpecmanError> {
    if name.is_empty() {
        return Err(SpecmanError::Workspace(
            "artifact name must not be empty".into(),
        ));
    }

    if name.contains('/') || name.contains('\\') {
        return Err(SpecmanError::Workspace(
            "artifact name must not contain path separators".into(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::TemplateDescriptor;
    use crate::workspace::FilesystemWorkspaceLocator;
    use tempfile::tempdir;

    fn setup_workspace() -> (
        tempfile::TempDir,
        WorkspacePersistence<FilesystemWorkspaceLocator>,
    ) {
        let temp = tempdir().unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir_all(root.join(".specman")).unwrap();
        let start = root.join("impl").join("services");
        fs::create_dir_all(&start).unwrap();
        let locator = FilesystemWorkspaceLocator::new(start);
        (temp, WorkspacePersistence::new(locator))
    }

    fn rendered(body: &str) -> RenderedTemplate {
        RenderedTemplate {
            body: body.to_string(),
            metadata: TemplateDescriptor::default(),
        }
    }

    fn artifact(kind: ArtifactKind, name: &str) -> ArtifactId {
        ArtifactId {
            kind,
            name: name.to_string(),
        }
    }

    #[test]
    fn persist_specification_creates_directories_and_writes_file() {
        let (_temp, persistence) = setup_workspace();
        let target = artifact(ArtifactKind::Specification, "feature-one");
        let rendered = rendered("---\nname: feature\n---\nbody");

        let result = persistence.persist(&target, &rendered).unwrap();

        assert!(result.path.exists());
        let contents = fs::read_to_string(&result.path).unwrap();
        assert_eq!(contents, rendered.body);
        assert!(
            result
                .path
                .ends_with(std::path::Path::new("spec/feature-one/spec.md"))
        );
    }

    #[test]
    fn persist_scratchpad_targets_dot_folder() {
        let (_temp, persistence) = setup_workspace();
        let target = artifact(ArtifactKind::ScratchPad, "workspace-template-persist");
        let rendered = rendered("scratch content");

        let result = persistence.persist(&target, &rendered).unwrap();
        assert!(result.path.ends_with(std::path::Path::new(
            ".specman/scratchpad/workspace-template-persist/scratch.md"
        )));
    }

    #[test]
    fn persist_rejects_unresolved_tokens() {
        let (_temp, persistence) = setup_workspace();
        let target = artifact(ArtifactKind::Implementation, "specman-library");
        let rendered = rendered("value: {{missing}}");

        let err = persistence.persist(&target, &rendered).unwrap_err();
        assert!(matches!(err, SpecmanError::Template(_)));
    }
}
