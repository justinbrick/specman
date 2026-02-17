use specman::{
    ArtifactId, ArtifactKind, StatusResult, WorkspacePaths, WorkspaceStatusConfig,
    validate_workspace_status,
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
    let (_dir, workspace) = make_workspace();
    let root = workspace.root();

    fs::create_dir_all(root.join("spec/a")).unwrap();
    fs::write(root.join("spec/a/spec.md"), 
        "---\nname: a\nversion: \"1.0.0\"\n---\n# A\n").unwrap();

    let config = WorkspaceStatusConfig::default();
    let report = validate_workspace_status(root.to_path_buf(), config).unwrap();

    assert_eq!(report.global_status, StatusResult::Pass);
    assert!(report.cycle_errors.is_empty());
    assert_eq!(report.artifacts.len(), 1);
}

#[test]
fn validation_detects_cycles() {
    let (_dir, workspace) = make_workspace();
    let root = workspace.root();

    fs::create_dir_all(root.join("spec/a")).unwrap();
    fs::create_dir_all(root.join("spec/b")).unwrap();

    // A -> B
    fs::write(root.join("spec/a/spec.md"), 
        "---\nname: a\nversion: \"1.0.0\"\ndependencies:\n  - ../b/spec.md\n---\n# A\n").unwrap();

    // B -> A
    fs::write(root.join("spec/b/spec.md"), 
        "---\nname: b\nversion: \"1.0.0\"\ndependencies:\n  - ../a/spec.md\n---\n# B\n").unwrap();

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
    fs::write(root.join("spec/a/spec.md"), 
        "---\nname: a\nversion: \"1.0.0\"\n---\n# A\nSee [missing](missing.md)\n").unwrap();

    let config = WorkspaceStatusConfig::default();
    let report = validate_workspace_status(root.to_path_buf(), config).unwrap();

    assert_eq!(report.global_status, StatusResult::Fail);
    
    let a_id = ArtifactId { kind: ArtifactKind::Specification, name: "a".into() };
    assert!(report.artifacts.contains_key(&a_id));
    assert!(!report.artifacts[&a_id].reference_errors.is_empty());
}

#[test]
fn validation_detects_structure_error() {
    let (_dir, workspace) = make_workspace();
    let root = workspace.root();

    fs::create_dir_all(root.join("spec/a")).unwrap();
    // Invalid YAML front matter
    fs::write(root.join("spec/a/spec.md"), 
        "---\nname: a\n version: broken indent\n---\n# A\n").unwrap();

    let config = WorkspaceStatusConfig::default();
    let report = validate_workspace_status(root.to_path_buf(), config).unwrap();

    // Even if front matter is invalid, it is gathered but with error metadata.
    
    // Note: If front matter is invalid, `infer_name` is used. Folder name is "a", so name is "a".
    let a_id = ArtifactId { kind: ArtifactKind::Specification, name: "a".into() };
    
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
    fs::write(root.join(".specman/scratchpad/fix/scratch.md"), 
        "---\nname: fix\nwork_type:\n  fix: {}\n---\n# Fix\n[broken](missing)\n").unwrap();

    let config = WorkspaceStatusConfig::default();
    let report = validate_workspace_status(root.to_path_buf(), config).unwrap();

    // Scratchpads affect global status if enabled.
    assert_eq!(report.global_status, StatusResult::Fail);
    assert_eq!(report.spec_impl_status, StatusResult::Pass);
    assert_eq!(report.scratchpad_status, StatusResult::Fail);
}
