use crate::core::error::SpecmanError;
use crate::metadata::frontmatter::{ArtifactFrontMatter, split_front_matter};
use std::fs;
use std::path::Path;

/// Writes the given front matter to the artifact at the given path, preserving the existing body.
pub fn write_artifact_front_matter(
    path: &Path,
    front_matter: &ArtifactFrontMatter,
) -> Result<(), SpecmanError> {
    let content = fs::read_to_string(path)
        .map_err(|e| SpecmanError::Workspace(format!("Failed to read artifact: {}", e)))?;

    let split = split_front_matter(&content)?;

    let yaml_str = serde_yaml::to_string(front_matter)
        .map_err(|e| SpecmanError::Serialization(e.to_string()))?;

    // Ensure we handle the leading --- from serde_yaml if present.
    let yaml_clean = yaml_str.trim_start_matches("---").trim_start();

    // Construct the new file content
    let to_write = format!("---\n{}\n---\n{}", yaml_clean.trim_end(), split.body);

    fs::write(path, to_write)
        .map_err(|e| SpecmanError::Workspace(format!("Failed to write artifact: {}", e)))?;

    Ok(())
}
