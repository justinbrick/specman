use specman::{
    HttpsMethod, HttpsValidationMode, IssueSeverity, ReferenceIssueKind,
    ReferenceValidationOptions, ReferenceValidationStatus, ReferenceValidator, ValidationMode,
    WorkspacePaths, validate_references,
};
use std::fs;
use std::time::Duration;

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
fn spec_handles_are_rejected_in_markdown_links() {
    let (_dir, workspace) = make_workspace();

    let doc_dir = workspace.spec_dir().join("a");
    fs::create_dir_all(&doc_dir).unwrap();

    let a_path = doc_dir.join("spec.md");
    fs::write(&a_path, "See [core](spec://core)\n").unwrap();

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
            .any(|i| i.kind == ReferenceIssueKind::DisallowedHandle)
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

#[test]
fn fragment_validation_respects_slug_rules() {
    let (_dir, workspace) = make_workspace();

    let doc_dir = workspace.spec_dir().join("a");
    fs::create_dir_all(&doc_dir).unwrap();

    let a_path = doc_dir.join("spec.md");
    fs::write(
        &a_path,
        "# Caf\u{00e9} World\n\nSee [accent](#cafe-world).\n\n## Overview\n\nSee [second](#overview-1).\n\n## Overview\n",
    )
    .unwrap();

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
}

#[test]
fn fragment_validation_can_be_disabled_via_mode() {
    let (_dir, workspace) = make_workspace();

    let doc_dir = workspace.spec_dir().join("a");
    fs::create_dir_all(&doc_dir).unwrap();

    let a_path = doc_dir.join("spec.md");
    fs::write(&a_path, "See [missing](#unknown-fragment).\n").unwrap();

    let mut mode = ValidationMode::default();
    mode.resolve_fragments = false;

    let validator = ReferenceValidator::with_mode(&workspace, mode);
    let report = validator
        .validate(a_path.to_string_lossy().as_ref())
        .unwrap();

    assert_eq!(report.status, ReferenceValidationStatus::Success);
    assert!(
        report
            .issues
            .iter()
            .all(|i| i.kind != ReferenceIssueKind::InvalidFragment)
    );
}

#[test]
fn unreachable_https_reports_diagnostic_not_error() {
    let (_dir, workspace) = make_workspace();

    let doc_dir = workspace.spec_dir().join("a");
    fs::create_dir_all(&doc_dir).unwrap();

    let a_path = doc_dir.join("spec.md");
    fs::write(&a_path, "See [bad](https://127.0.0.1:1/).\n").unwrap();

    let mut options = ReferenceValidationOptions::default();
    options.https.mode = HttpsValidationMode::Reachability {
        timeout: Duration::from_millis(200),
        max_redirects: 2,
        method: HttpsMethod::Head,
    };

    let report =
        validate_references(a_path.to_string_lossy().as_ref(), &workspace, options).unwrap();

    assert_eq!(report.status, ReferenceValidationStatus::Success);
    assert!(report.issues.iter().any(|i| {
        i.kind == ReferenceIssueKind::UnreachableUrl && i.severity == IssueSeverity::Diagnostic
    }));
}
