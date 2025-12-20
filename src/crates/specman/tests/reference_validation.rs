use specman::{
    IssueSeverity, ReferenceValidationOptions, ReferenceValidationStatus, WorkspacePaths,
    validate_references,
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
fn reference_link_with_missing_target_reports_error() {
    let (_dir, workspace) = make_workspace();

    let doc_dir = workspace.spec_dir().join("a");
    fs::create_dir_all(&doc_dir).unwrap();

    let a_path = doc_dir.join("spec.md");
    fs::write(&a_path, "See [thing][ref].\n\n[ref]: missing.md\n").unwrap();

    let report = validate_references(
        a_path.to_string_lossy().as_ref(),
        &workspace,
        ReferenceValidationOptions::default(),
    )
    .unwrap();

    assert_eq!(report.status, ReferenceValidationStatus::Failure);
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Error)
    );
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Error && i.source.range.is_some())
    );
}

#[test]
fn cross_document_fragment_is_validated_transitively() {
    let (_dir, workspace) = make_workspace();

    let doc_dir = workspace.spec_dir().join("a");
    fs::create_dir_all(&doc_dir).unwrap();

    let a_path = doc_dir.join("spec.md");
    let b_path = doc_dir.join("b.md");

    fs::write(&a_path, "See [B](b.md#target).\n").unwrap();
    fs::write(&b_path, "# B\n\n## Target\n\nText\n").unwrap();

    let report = validate_references(
        a_path.to_string_lossy().as_ref(),
        &workspace,
        ReferenceValidationOptions::default(),
    )
    .unwrap();

    assert_eq!(report.status, ReferenceValidationStatus::Success);
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Error)
    );
    assert!(report.processed_documents >= 2);
}

#[test]
fn cross_document_fragment_missing_heading_is_error() {
    let (_dir, workspace) = make_workspace();

    let doc_dir = workspace.spec_dir().join("a");
    fs::create_dir_all(&doc_dir).unwrap();

    let a_path = doc_dir.join("spec.md");
    let b_path = doc_dir.join("b.md");

    fs::write(&a_path, "See [B](b.md#target).\n").unwrap();
    fs::write(&b_path, "# B\n\n## Other\n\nText\n").unwrap();

    let report = validate_references(
        a_path.to_string_lossy().as_ref(),
        &workspace,
        ReferenceValidationOptions::default(),
    )
    .unwrap();

    assert_eq!(report.status, ReferenceValidationStatus::Failure);
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Error
                && i.message.contains("does not match any heading slug in"))
    );
}

#[test]
fn escapes_workspace_is_error_even_if_missing() {
    let (_dir, workspace) = make_workspace();

    let doc_dir = workspace.spec_dir().join("a");
    fs::create_dir_all(&doc_dir).unwrap();

    let a_path = doc_dir.join("spec.md");
    fs::write(&a_path, "[x](../../../../outside.md)\n").unwrap();

    let report = validate_references(
        a_path.to_string_lossy().as_ref(),
        &workspace,
        ReferenceValidationOptions::default(),
    )
    .unwrap();

    assert_eq!(report.status, ReferenceValidationStatus::Failure);
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Error && i.message.contains("escapes workspace"))
    );
}

#[test]
fn images_are_ignored() {
    let (_dir, workspace) = make_workspace();

    let doc_dir = workspace.spec_dir().join("a");
    fs::create_dir_all(&doc_dir).unwrap();

    let a_path = doc_dir.join("spec.md");
    fs::write(&a_path, "![alt](b.md)\n").unwrap();

    let report = validate_references(
        a_path.to_string_lossy().as_ref(),
        &workspace,
        ReferenceValidationOptions::default(),
    )
    .unwrap();

    assert_eq!(report.status, ReferenceValidationStatus::Success);
    assert!(report.discovered.is_empty());
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Error)
    );
}
