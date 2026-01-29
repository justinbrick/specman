use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use ignore::WalkBuilder;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::dependency_tree::{
    ArtifactId, ArtifactKind, DependencyMapping, FilesystemDependencyMapper,
};
use crate::error::SpecmanError;
use crate::metadata::frontmatter::{ImplementationFrontMatter, split_front_matter};
use crate::structure::build_workspace_index_for_artifacts;
use crate::workspace::{
    FilesystemWorkspaceLocator, WorkspaceLocator, normalize_workspace_path, workspace_relative_path,
};

const BINARY_CHECK_BYTES: usize = 8192;

/// Check first few bytes for nulls to determine if file is binary
fn is_binary(path: &Path) -> bool {
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut buffer = [0; BINARY_CHECK_BYTES];
    let n = match file.read(&mut buffer) {
        Ok(n) => n,
        Err(_) => return false,
    };

    buffer[..n].contains(&0)
}

/// Represents a discovered validation anchor.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct ValidationTag {
    // [ENSURES: entity-validation-tag.types:CHECK]
    pub identifier: String, // e.g., "concept-slug.category"
    pub tag_type: ValidationType,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub enum ValidationType {
    Test,
    Check,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct SourceLocation {
    pub file_path: PathBuf, // Relative to the implementation root
    pub line_number: usize, // 1-based
}

/// The result of a compliance check.
// [ENSURES: concept-compliance-reporting.interface:CHECK]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ComplianceReport {
    pub specification: ArtifactId,
    pub implementation: ArtifactId,
    /// Maps constraint group IDs to the tags that cover them.
    pub coverage: BTreeMap<String, Vec<ValidationTag>>,
    /// List of constraint group IDs that have no coverage.
    pub missing: Vec<String>,
    /// Tags that reference non-existent constraints.
    pub orphans: Vec<ValidationTag>,
}

pub fn parse_tags(line: &str, line_idx: usize, file_path: &Path) -> Vec<ValidationTag> {
    // Regex: \[ENSURES:\s*([a-zA-Z0-9.-]+)(?::(TEST|CHECK|MANUAL))?\s*\]
    // [ENSURES: entity-validation-tag.syntax:CHECK]
    // Case insensitive matching for ENSURES
    static TAG_REGEX: OnceLock<Regex> = OnceLock::new();
    let re = TAG_REGEX.get_or_init(|| {
        Regex::new(r"(?i)\[ENSURES:\s*([a-zA-Z0-9.\-_]+)(?::(TEST|CHECK|MANUAL))?\s*\]")
            .expect("Invalid regex")
    });

    let mut tags = Vec::new();

    for cap in re.captures_iter(line) {
        if let Some(id_match) = cap.get(1) {
            let identifier = id_match.as_str().to_string();

            let tag_type = if let Some(type_match) = cap.get(2) {
                match type_match.as_str().to_uppercase().as_str() {
                    "TEST" => ValidationType::Test,
                    "CHECK" => ValidationType::Check,
                    "MANUAL" => ValidationType::Manual,
                    _ => ValidationType::Test, // Should be unreachable given regex
                }
            } else {
                ValidationType::Test
            };

            tags.push(ValidationTag {
                identifier,
                tag_type,
                location: SourceLocation {
                    file_path: file_path.to_path_buf(),
                    line_number: line_idx + 1,
                },
            });
        }
    }

    tags
}

pub fn generate_report(
    spec_id: ArtifactId,
    impl_id: ArtifactId,
    spec_constraints: &[String],
    mut tags: Vec<ValidationTag>,
) -> ComplianceReport {
    // Sort tags first to ensure stable inputs if processing order matters,
    // or sort later in outputs.
    tags.sort();

    // [ENSURES: concept-compliance-reporting.coverage:CHECK]
    let mut coverage: BTreeMap<String, Vec<ValidationTag>> = BTreeMap::new();
    let mut orphans: Vec<ValidationTag> = Vec::new();

    let known_constraints: HashSet<&String> = spec_constraints.iter().collect();

    for tag in tags {
        if known_constraints.contains(&tag.identifier) {
            coverage
                .entry(tag.identifier.clone())
                .or_default()
                .push(tag);
        } else {
            orphans.push(tag);
        }
    }

    // [ENSURES: concept-compliance-reporting.semantics:CHECK]
    let mut missing = Vec::new();
    for constraint in spec_constraints {
        if !coverage.contains_key(constraint) {
            missing.push(constraint.clone());
        }
    }
    // Sort missing for determinism
    missing.sort();

    ComplianceReport {
        specification: spec_id,
        implementation: impl_id,
        coverage,
        missing,
        orphans,
    }
}

pub fn validate_compliance(
    workspace_root: &Path,
    impl_id: &ArtifactId,
) -> Result<ComplianceReport, SpecmanError> {
    // [ENSURES: concept-compliance-resources.scope.schemes:CHECK]
    if impl_id.kind != ArtifactKind::Implementation {
        return Err(SpecmanError::Workspace(
            "Compliance reporting is only available for implementation artifacts".into(),
        ));
    }

    let locator = Arc::new(FilesystemWorkspaceLocator::new(
        workspace_root.to_path_buf(),
    ));
    let workspace = locator.workspace()?;

    let mapper = FilesystemDependencyMapper::new(locator.clone());
    let tree = mapper.dependency_tree(impl_id)?;

    let impl_path = if let Some(p) = &tree.root.resolved_path {
        PathBuf::from(p)
    } else {
        workspace.impl_dir().join(&impl_id.name).join("impl.md")
    };

    let impl_root = impl_path.parent().ok_or_else(|| {
        SpecmanError::Workspace(format!(
            "invalid implementation path: {}",
            impl_path.display()
        ))
    })?;

    let impl_body = fs::read_to_string(&impl_path)?;
    let split = split_front_matter(&impl_body)?;
    let front: ImplementationFrontMatter = serde_yaml::from_str(split.yaml)
        .map_err(|err| SpecmanError::Serialization(err.to_string()))?;

    let spec_ref = front.spec.ok_or_else(|| {
        SpecmanError::Dependency(format!(
            "Implementation {} has no spec metadata for compliance reporting",
            impl_id.name
        ))
    })?;

    let spec_path = if let Some(rest) = spec_ref.strip_prefix("spec://") {
        if rest.trim().is_empty() {
            return Err(SpecmanError::Dependency(
                "spec locator must not be empty".into(),
            ));
        }
        workspace.spec_dir().join(rest).join("spec.md")
    } else if spec_ref.starts_with("https://") {
        return Err(SpecmanError::Dependency(
            "compliance reporting does not support remote specifications".into(),
        ));
    } else if spec_ref.contains("://") {
        return Err(SpecmanError::Dependency(format!(
            "unsupported spec locator scheme: {spec_ref}"
        )));
    } else {
        let candidate = Path::new(&spec_ref);
        if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            let from_impl = impl_root.join(candidate);
            if from_impl.is_file() {
                from_impl
            } else {
                workspace.root().join(candidate)
            }
        }
    };

    if !spec_path.is_file() {
        return Err(SpecmanError::Dependency(format!(
            "spec locator does not resolve to a file: {}",
            spec_ref
        )));
    }

    let canonical_spec = fs::canonicalize(&spec_path).unwrap_or(spec_path.clone());

    let spec_tree = mapper.dependency_tree_from_path(&canonical_spec)?;
    let spec_id = spec_tree.root.id.clone();

    let mut spec_summaries = Vec::new();
    spec_summaries.push(spec_tree.root.clone());
    for edge in &spec_tree.upstream {
        if edge.to.id.kind == ArtifactKind::Specification {
            spec_summaries.push(edge.to.clone());
        }
    }

    let mut spec_artifacts: Vec<(ArtifactKind, PathBuf)> = Vec::new();
    let mut spec_workspace_paths: HashSet<String> = HashSet::new();

    for summary in spec_summaries {
        let mut path = if let Some(resolved) = &summary.resolved_path {
            let resolved = resolved.trim();
            if resolved.contains("://") {
                // TODO: Support remote specification dependencies in compliance reports.
                continue;
            }
            let mut path = PathBuf::from(resolved);
            if path.is_relative() {
                path = workspace.root().join(path);
            }
            path
        } else {
            workspace.spec_dir().join(&summary.id.name).join("spec.md")
        };

        if !path.is_file() {
            return Err(SpecmanError::Dependency(format!(
                "spec locator does not resolve to a file: {}",
                path.display()
            )));
        }

        path = normalize_workspace_path(&path);
        let workspace_path = workspace_relative_path(workspace.root(), &path).ok_or_else(|| {
            SpecmanError::Workspace(format!(
                "failed to resolve workspace-relative path for '{}'",
                path.display()
            ))
        })?;

        spec_workspace_paths.insert(workspace_path);
        spec_artifacts.push((ArtifactKind::Specification, path));
    }

    // 2. Load spec constraints from the structure index (scoped to spec + dependencies only)
    let index = build_workspace_index_for_artifacts(&workspace, &spec_artifacts)?;

    let mut spec_constraints = Vec::new();
    for (key, _) in &index.constraints {
        if key.artifact.kind == ArtifactKind::Specification
            && spec_workspace_paths.contains(&key.artifact.workspace_path)
        {
            spec_constraints.push(key.group.clone());
        }
    }
    spec_constraints.sort();

    // 3. Scan implementation
    let tags = scan_source_root(impl_root)?;
    Ok(generate_report(
        spec_id,
        impl_id.clone(),
        &spec_constraints,
        tags,
    ))
}

pub fn scan_source_root(root: &Path) -> Result<Vec<ValidationTag>, SpecmanError> {
    let walker = WalkBuilder::new(root)
        .hidden(true) // skip hidden
        .ignore(true) // respect .ignore
        .git_ignore(true) // respect .gitignore (default)
        .build();

    let mut all_tags = Vec::new();

    for result in walker {
        match result {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file() {
                    // Check if binary
                    if is_binary(path) {
                        continue;
                    }

                    // Try reading as UTF-8 string
                    match fs::read_to_string(path) {
                        Ok(content) => {
                            for (idx, line) in content.lines().enumerate() {
                                let mut line_tags = parse_tags(line, idx, path);

                                // Relativize path
                                if let Ok(rel_path) = path.strip_prefix(root) {
                                    for tag in &mut line_tags {
                                        tag.location.file_path = rel_path.to_path_buf();
                                    }
                                }

                                all_tags.extend(line_tags);
                            }
                        }
                        Err(_) => {
                            // Skip file if read error
                            continue;
                        }
                    }
                }
            }
            Err(err) => {
                // Log warning but continue?
                // For now, we just skip errors.
                eprintln!("Scanner warning: {}", err);
            }
        }
    }

    Ok(all_tags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_scanner_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create a source file
        let src_file = root.join("main.rs");
        let mut f = fs::File::create(&src_file).unwrap();
        writeln!(f, "// [ENSURES: feature.a:TEST]").unwrap();

        // Create a nested file
        let nested_dir = root.join("subdir");
        fs::create_dir(&nested_dir).unwrap();
        let nested_file = nested_dir.join("lib.rs");
        let mut f2 = fs::File::create(&nested_file).unwrap();
        writeln!(f2, "// [ENSURES: feature.b:CHECK]").unwrap();

        // Create a binary file (simulated with null byte)
        let bin_file = root.join("binary.bin");
        let mut f3 = fs::File::create(&bin_file).unwrap();
        f3.write_all(b"some binary data \0 inside").unwrap();

        // Run scanner
        let tags = scan_source_root(root).unwrap();

        // Should find 2 tags
        // Note: order might vary depending on WalkBuilder, so we check existence
        // [ENSURES: concept-validation-scanning.scope]
        // [ENSURES: concept-validation-scanning.filtering]
        assert_eq!(tags.len(), 2);

        let tag_ids: Vec<String> = tags.iter().map(|t| t.identifier.clone()).collect();
        assert!(tag_ids.contains(&"feature.a".to_string()));
        assert!(tag_ids.contains(&"feature.b".to_string()));
    }

    #[test]
    fn test_compliance_report() {
        use crate::dependency_tree::ArtifactKind;

        let spec_id = ArtifactId {
            kind: ArtifactKind::Specification,
            name: "my-spec".into(),
        };
        let impl_id = ArtifactId {
            kind: ArtifactKind::Implementation,
            name: "my-impl".into(),
        };

        let constraints = vec![
            "req.1".to_string(),
            "req.2".to_string(),
            "req.3".to_string(),
        ];

        let tags = vec![
            ValidationTag {
                // req.1 covered
                identifier: "req.1".to_string(),
                tag_type: ValidationType::Test,
                location: SourceLocation {
                    file_path: PathBuf::from("a.rs"),
                    line_number: 1,
                },
            },
            ValidationTag {
                // orphan
                identifier: "req.orphan".to_string(),
                tag_type: ValidationType::Manual,
                location: SourceLocation {
                    file_path: PathBuf::from("b.rs"),
                    line_number: 1,
                },
            },
        ];

        let report = generate_report(spec_id.clone(), impl_id.clone(), &constraints, tags);

        assert_eq!(report.specification, spec_id);
        assert_eq!(report.implementation, impl_id);

        // Check coverage
        assert!(report.coverage.contains_key("req.1"));
        assert!(!report.coverage.contains_key("req.2"));
        assert!(!report.coverage.contains_key("req.3"));

        // Check missing
        assert_eq!(report.missing.len(), 2);
        assert!(report.missing.contains(&"req.2".to_string()));
        assert!(report.missing.contains(&"req.3".to_string()));

        // Check orphans
        assert_eq!(report.orphans.len(), 1);
        assert_eq!(report.orphans[0].identifier, "req.orphan");
    }

    #[test]
    fn test_parse_simple_tag() {
        let line = " // [ENSURES: my-concept.req:TEST] ";
        let path = Path::new("src/main.rs");
        let tags = parse_tags(line, 10, path);

        // [ENSURES: concept-validation-anchors.definition]
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].identifier, "my-concept.req");
        assert_eq!(tags[0].tag_type, ValidationType::Test);
        assert_eq!(tags[0].location.line_number, 11);
    }

    #[test]
    fn test_parse_multiple_tags() {
        let line = "# [ENSURES: req1:CHECK] [ENSURES: req2:MANUAL]";
        let path = Path::new("README.md");
        let tags = parse_tags(line, 0, path);

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].identifier, "req1");
        assert_eq!(tags[0].tag_type, ValidationType::Check);
        assert_eq!(tags[1].identifier, "req2");
        assert_eq!(tags[1].tag_type, ValidationType::Manual);
    }

    #[test]
    fn test_parse_optional_type() {
        let line = "// [ENSURES: req1]";
        let path = Path::new("file.rs");
        let tags = parse_tags(line, 0, path);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].identifier, "req1");
        // Using Manual as default for now per my decision, verifying behaviour
        assert_eq!(tags[0].tag_type, ValidationType::Test); // wait, I put Test in the code above.
    }

    #[test]
    fn test_case_insensitive_keyword() {
        let line = "// [Ensures: req1:Check]";
        let path = Path::new("file.rs");
        let tags = parse_tags(line, 0, path);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].identifier, "req1");
        assert_eq!(tags[0].tag_type, ValidationType::Check);
    }
}
