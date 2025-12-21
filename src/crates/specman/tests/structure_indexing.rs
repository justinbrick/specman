use std::fs;

use specman::{
    FilesystemStructureIndexer, FilesystemWorkspaceLocator, StructureQuery, WorkspacePaths,
    discover_workspace,
};
use tempfile::tempdir;

fn make_workspace() -> WorkspacePaths {
    let temp = tempdir().unwrap();
    let root = temp.path().join("workspace");
    fs::create_dir_all(root.join(".specman").join("scratchpad")).unwrap();
    fs::create_dir_all(root.join("spec")).unwrap();
    fs::create_dir_all(root.join("impl")).unwrap();

    // Persist tempdir for the duration of the test by leaking it.
    // This matches existing test patterns in the repo that use tempdir but keep paths alive.
    let root = root.canonicalize().unwrap();
    let dot = root.join(".specman");
    std::mem::forget(temp);
    WorkspacePaths::new(root, dot)
}

#[test]
fn indexing_fails_on_duplicate_heading_slugs_within_document() {
    let workspace = make_workspace();
    fs::create_dir_all(workspace.spec_dir().join("dup")).unwrap();
    fs::write(
        workspace.spec_dir().join("dup").join("spec.md"),
        "---\nname: dup\nversion: '1.0.0'\n---\n# Title\n\n## Overview\nText\n\n## Overview\nMore\n",
    )
    .unwrap();

    let indexer = FilesystemStructureIndexer::new(FilesystemWorkspaceLocator::new(
        workspace.dot_specman().to_path_buf(),
    ));

    let err = indexer
        .build_once_with_workspace(&workspace)
        .expect_err("should fail fast on duplicate slugs");

    let msg = err.to_string();
    assert!(msg.contains("duplicate heading slug"), "{msg}");
}

#[test]
fn render_heading_includes_referenced_headings_deduped_in_order() {
    let workspace = make_workspace();
    fs::create_dir_all(workspace.spec_dir().join("refs")).unwrap();
    fs::write(
        workspace.spec_dir().join("refs").join("spec.md"),
        "---\nname: refs\nversion: '1.0.0'\n---\n# Doc\n\n## A\nIntro. [B](#b) then [C](#c) then [B again](#b).\n\n## B\nBee.\n\n## C\nSee.\n",
    )
    .unwrap();

    let indexer = FilesystemStructureIndexer::new(FilesystemWorkspaceLocator::new(
        workspace.dot_specman().to_path_buf(),
    ));
    let index = indexer.build_once_with_workspace(&workspace).unwrap();

    let rendered = index.render_heading_by_slug("a").unwrap();

    let a_pos = rendered.find("## A").unwrap();
    let b_pos = rendered.find("## B").unwrap();
    let c_pos = rendered.find("## C").unwrap();

    assert!(a_pos < b_pos, "expected A before B\n{rendered}");
    assert!(b_pos < c_pos, "expected B before C\n{rendered}");

    assert_eq!(rendered.matches("## B").count(), 1, "{rendered}");
    assert_eq!(rendered.matches("## C").count(), 1, "{rendered}");
}

#[test]
fn constraint_groups_map_to_first_group_heading_or_nearest() {
    let workspace = make_workspace();
    fs::create_dir_all(workspace.spec_dir().join("constraints")).unwrap();
    fs::write(
        workspace.spec_dir().join("constraints").join("spec.md"),
        "---\nname: constraints\nversion: '1.0.0'\n---\n# Doc\n\n## Foo\n!foo.formatting:\n- MUST do foo.\n\n## Bar\n!missing.formatting:\n- MUST do bar.\n",
    )
    .unwrap();

    let indexer = FilesystemStructureIndexer::new(FilesystemWorkspaceLocator::new(
        workspace.dot_specman().to_path_buf(),
    ));
    let index = indexer.build_once_with_workspace(&workspace).unwrap();

    let foo_group = index
        .list_constraint_groups()
        .into_iter()
        .find(|id| id.group == "foo.formatting")
        .unwrap();
    let missing_group = index
        .list_constraint_groups()
        .into_iter()
        .find(|id| id.group == "missing.formatting")
        .unwrap();

    let foo_record = index.constraints.get(&foo_group).unwrap();
    assert_eq!(foo_record.heading.slug, "foo");

    let missing_record = index.constraints.get(&missing_group).unwrap();
    assert_eq!(missing_record.heading.slug, "bar");
}

#[test]
fn indexing_rejects_links_that_escape_workspace() {
    let workspace = make_workspace();
    fs::create_dir_all(workspace.spec_dir().join("escape")).unwrap();
    fs::write(
        workspace.spec_dir().join("escape").join("spec.md"),
        "---\nname: escape\nversion: '1.0.0'\n---\n# Doc\n\n## A\nSee [outside](../../../outside.md).\n",
    )
    .unwrap();

    let indexer = FilesystemStructureIndexer::new(FilesystemWorkspaceLocator::new(
        workspace.dot_specman().to_path_buf(),
    ));

    let err = indexer
        .build_once_with_workspace(&workspace)
        .expect_err("workspace escape should error");

    let msg = err.to_string();
    assert!(msg.contains("escapes workspace"), "{msg}");
}

#[test]
fn discover_workspace_still_works_with_indexer() {
    let workspace = make_workspace();
    let discovered = discover_workspace(workspace.dot_specman()).unwrap();
    assert_eq!(discovered.root(), workspace.root());
}
