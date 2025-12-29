use std::path::{Component, Path};

use specman::workspace_relative_path;

use crate::error::{CliError, ExitStatus};

#[derive(Clone, Copy, Debug, Default)]
pub struct Verbosity {
    pub json: bool,
    pub verbose: bool,
}

pub fn validate_locator(value: &str, field: &str) -> Result<(), CliError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CliError::new(
            format!("{field} locator must not be empty"),
            ExitStatus::Usage,
        ));
    }

    if trimmed.starts_with("https://") {
        return Ok(());
    }

    if trimmed.contains("://") {
        return Err(CliError::new(
            format!("{field} locator '{trimmed}' must be an HTTPS URL or workspace-relative path"),
            ExitStatus::Usage,
        ));
    }

    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(CliError::new(
            format!("{field} locator '{trimmed}' must be workspace-relative"),
            ExitStatus::Usage,
        ));
    }

    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(CliError::new(
            format!("{field} locator '{trimmed}' must not traverse parent directories (..)",),
            ExitStatus::Usage,
        ));
    }

    Ok(())
}
pub fn validate_slug(name: &str, kind: &str) -> Result<(), CliError> {
    if name.is_empty() {
        return Err(CliError::new(
            format!("{kind} name must not be empty"),
            ExitStatus::Usage,
        ));
    }

    if name
        .chars()
        .any(|ch| !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'))
    {
        return Err(CliError::new(
            format!("{kind} name '{name}' must be lowercase alphanumeric with hyphen separators"),
            ExitStatus::Usage,
        ));
    }

    let segments: Vec<_> = name.split('-').filter(|seg| !seg.is_empty()).collect();
    if segments.is_empty() {
        return Err(CliError::new(
            format!("{kind} name '{name}' is invalid"),
            ExitStatus::Usage,
        ));
    }

    if segments.len() > 4 {
        return Err(CliError::new(
            format!("{kind} name '{name}' must contain no more than four hyphenated words"),
            ExitStatus::Usage,
        ));
    }

    Ok(())
}

pub fn workspace_relative(root: &Path, path: &Path) -> String {
    workspace_relative_path(root, path)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}
