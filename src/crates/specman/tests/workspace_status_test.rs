use schemars::schema_for;
use specman::{
    ArtifactId, ArtifactKind, ArtifactStatus, StatusResult, WorkspacePaths, WorkspaceStatusConfig,
    WorkspaceStatusReport, validate_workspace_status,
};
use std::fs;

fn make_workspace() -> (tempfile::TempDir, WorkspacePaths) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();

    let dot_specman = root.join(".specman");
    fs::create_dir_all(dot_specman.join("scratchpad")).expect("create scratchpad");
    fs::create_dir_all(root.join("spec")).expect("create spec");
    fs::create_dir_all(root.join("impl")).expect("create impl");

    let workspace = WorkspacePaths::new(root, dot_specman);
    (dir, workspace)
}

#[test]
fn clean_workspace_passes() {
    // [ENSURES: concept-specifications.metadata.frontmatter:TEST]
    let (_dir, workspace) = make_workspace();
    let root = workspace.root();

    fs::create_dir_all(root.join("spec/a")).unwrap();
    fs::write(
        root.join("spec/a/spec.md"),
        "---\nname: a\nversion: \"1.0.0\"\n---\n# A\n",
    )
    .unwrap();

    let config = WorkspaceStatusConfig::default();
    let report = validate_workspace_status(root.to_path_buf(), config).unwrap();

    assert_eq!(report.global_status, StatusResult::Pass);
    assert!(report.cycle_errors.is_empty());
    assert_eq!(report.artifacts.len(), 1);
    assert_eq!(report.artifact_count, 1);
}

#[test]
fn validation_detects_cycles() {
    // [ENSURES: concept-specifications.dependencies:TEST]
    let (_dir, workspace) = make_workspace();
    let root = workspace.root();

    fs::create_dir_all(root.join("spec/a")).unwrap();
    fs::create_dir_all(root.join("spec/b")).unwrap();

    // A -> B
    fs::write(
        root.join("spec/a/spec.md"),
        "---\nname: a\nversion: \"1.0.0\"\ndependencies:\n  - ../b/spec.md\n---\n# A\n",
    )
    .unwrap();

    // B -> A
    fs::write(
        root.join("spec/b/spec.md"),
        "---\nname: b\nversion: \"1.0.0\"\ndependencies:\n  - ../a/spec.md\n---\n# B\n",
    )
    .unwrap();

    let config = WorkspaceStatusConfig::default();
    let report = validate_workspace_status(root.to_path_buf(), config).unwrap();

    assert_eq!(report.global_status, StatusResult::Fail);
    assert!(!report.cycle_errors.is_empty());
    // Depending on traversal order, we might get one or two cycle reports.
}

#[test]
fn validation_detects_broken_reference() {
    let (_dir, workspace) = make_workspace();
    let root = workspace.root();

    fs::create_dir_all(root.join("spec/a")).unwrap();
    fs::write(
        root.join("spec/a/spec.md"),
        "---\nname: a\nversion: \"1.0.0\"\n---\n# A\nSee [missing](missing.md)\n",
    )
    .unwrap();

    let config = WorkspaceStatusConfig::default();
    let report = validate_workspace_status(root.to_path_buf(), config).unwrap();

    assert_eq!(report.global_status, StatusResult::Fail);

    let a_id = ArtifactId {
        kind: ArtifactKind::Specification,
        name: "a".into(),
    };
    assert!(report.artifacts.contains_key(&a_id));
    assert!(!report.artifacts[&a_id].reference_errors.is_empty());
}

#[test]
fn validation_detects_structure_error() {
    let (_dir, workspace) = make_workspace();
    let root = workspace.root();

    fs::create_dir_all(root.join("spec/a")).unwrap();
    // Invalid YAML front matter
    fs::write(
        root.join("spec/a/spec.md"),
        "---\nname: a\n version: broken indent\n---\n# A\n",
    )
    .unwrap();

    let config = WorkspaceStatusConfig::default();
    let report = validate_workspace_status(root.to_path_buf(), config).unwrap();

    // Even if front matter is invalid, it is gathered but with error metadata.

    // Note: If front matter is invalid, `infer_name` is used. Folder name is "a", so name is "a".
    let a_id = ArtifactId {
        kind: ArtifactKind::Specification,
        name: "a".into(),
    };

    // The report might contain the artifact but with structure errors.
    // However, if parsing fails completely, `gather_workspace_artifacts` still finds the file.
    // `ArtifactDocument::load` returns metadata_status="invalid...".

    assert!(report.artifacts.contains_key(&a_id));
    assert!(!report.artifacts[&a_id].structure_errors.is_empty());
    assert_eq!(report.global_status, StatusResult::Fail);
}

#[test]
fn scratchpad_separation() {
    let (_dir, workspace) = make_workspace();
    let root = workspace.root();

    // Valid spec
    fs::create_dir_all(root.join("spec/a")).unwrap();
    fs::write(root.join("spec/a/spec.md"), "---\nname: a\n---\n# A\n").unwrap();

    // Failing scratchpad (broken ref)
    fs::create_dir_all(root.join(".specman/scratchpad/fix")).unwrap();
    fs::write(
        root.join(".specman/scratchpad/fix/scratch.md"),
        "---\nname: fix\nwork_type:\n  fix: {}\n---\n# Fix\n[broken](missing)\n",
    )
    .unwrap();

    let config = WorkspaceStatusConfig::default();
    let report = validate_workspace_status(root.to_path_buf(), config).unwrap();

    // Scratchpads are reported separately and do not affect global status.
    assert_eq!(report.global_status, StatusResult::Pass);
    assert_eq!(report.spec_impl_status, StatusResult::Pass);
    assert_eq!(report.scratchpad_status, StatusResult::Fail);
    assert_eq!(report.artifact_count, 2);
}

#[test]
fn compliance_reports_resolved_scan_root() {
    let (_dir, workspace) = make_workspace();
    let root = workspace.root();

    fs::create_dir_all(root.join("spec/core")).unwrap();
    fs::write(
        root.join("spec/core/spec.md"),
        "---\nname: core\nversion: \"1.0.0\"\n---\n# Core\n## Concept: SpecMan Structure\n!concept-specman-structure.referencing.validation:\n- Implementations that index relationships from inline links MUST provide a method to validate the referenced destinations and report any invalid references.\n",
    )
    .unwrap();

    fs::create_dir_all(root.join("impl/lib")).unwrap();
    fs::write(
        root.join("impl/lib/impl.md"),
        "---\nname: lib\nspec: spec://core\nlocation: ../../src/lib\nversion: \"1.0.0\"\n---\n# Lib\n",
    )
    .unwrap();

    fs::create_dir_all(root.join("src/lib")).unwrap();
    fs::write(
        root.join("src/lib/indexer.rs"),
        "// [ENSURES: concept-specman-structure.referencing.validation:CHECK]\n",
    )
    .unwrap();

    let report =
        validate_workspace_status(root.to_path_buf(), WorkspaceStatusConfig::default()).unwrap();

    let impl_id = ArtifactId {
        kind: ArtifactKind::Implementation,
        name: "lib".into(),
    };
    let impl_status = report
        .artifacts
        .get(&impl_id)
        .expect("impl artifact status");
    assert_eq!(impl_status.compliance_missing, Vec::<String>::new());
    assert_eq!(
        impl_status.compliance_scan_root.as_deref(),
        Some(root.join("src/lib").to_string_lossy().as_ref())
    );
}

#[test]
fn compliance_fails_when_location_metadata_is_omitted() {
    let (_dir, workspace) = make_workspace();
    let root = workspace.root();

    fs::create_dir_all(root.join("spec/core")).unwrap();
    fs::write(
        root.join("spec/core/spec.md"),
        "---\nname: core\nversion: \"1.0.0\"\n---\n# Core\n## Concept: SpecMan Structure\n!concept-specman-structure.referencing.validation:\n- Implementations that index relationships from inline links MUST provide a method to validate the referenced destinations and report any invalid references.\n",
    )
    .unwrap();

    fs::create_dir_all(root.join("impl/lib")).unwrap();
    fs::write(
        root.join("impl/lib/impl.md"),
        "---\nname: lib\nspec: spec://core\nversion: \"1.0.0\"\n---\n# Lib\n",
    )
    .unwrap();

    let report =
        validate_workspace_status(root.to_path_buf(), WorkspaceStatusConfig::default()).unwrap();

    assert_eq!(report.global_status, StatusResult::Fail);

    let impl_id = ArtifactId {
        kind: ArtifactKind::Implementation,
        name: "lib".into(),
    };
    let impl_status = report
        .artifacts
        .get(&impl_id)
        .expect("impl artifact status");
    assert!(
        impl_status
            .compliance_missing
            .iter()
            .any(|m| m.contains("missing required `location` metadata"))
    );
}

#[test]
fn workspace_status_config_defaults_match_required_categories() {
    // [ENSURES: concept-workspace-status.requirements:TEST]
    // [ENSURES: entity-workspacestatusconfig.schema:TEST]
    let config = WorkspaceStatusConfig::default();
    assert!(config.structure);
    assert!(config.references);
    assert!(config.cycles);
    assert!(config.compliance);
    assert!(config.scratchpads);
    assert!(config.reference_options.is_none());
}

#[test]
fn workspace_status_report_schema_declares_required_fields() {
    // [ENSURES: entity-workspacestatusreport.schema:TEST]
    // [ENSURES: entity-artifactstatus.schema:TEST]
    let report_schema = schema_for!(WorkspaceStatusReport);
    let report_json = serde_json::to_string(&report_schema).expect("serialize report schema");
    assert!(report_json.contains("\"global_status\""));
    assert!(report_json.contains("\"spec_impl_status\""));
    assert!(report_json.contains("\"scratchpad_status\""));
    assert!(report_json.contains("\"artifacts\""));
    assert!(report_json.contains("\"cycle_errors\""));
    assert!(report_json.contains("\"structure_errors\""));
    assert!(report_json.contains("\"artifact_count\""));

    let artifact_schema = schema_for!(ArtifactStatus);
    let artifact_json = serde_json::to_string(&artifact_schema).expect("serialize artifact schema");
    assert!(artifact_json.contains("\"structure_errors\""));
    assert!(artifact_json.contains("\"reference_errors\""));
    assert!(artifact_json.contains("\"compliance_missing\""));
    assert!(artifact_json.contains("\"compliance_orphans\""));
}
