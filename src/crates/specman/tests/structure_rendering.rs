use std::fs;
use specman::{
    FilesystemStructureIndexer, FilesystemWorkspaceLocator, StructureQuery, WorkspacePaths,
};
use tempfile::tempdir;

fn make_workspace() -> WorkspacePaths {
    let temp = tempdir().unwrap();
    let root = temp.path().join("workspace");
    fs::create_dir_all(root.join(".specman").join("scratchpad")).unwrap();
    fs::create_dir_all(root.join("spec")).unwrap();
    fs::create_dir_all(root.join("impl")).unwrap();

    let root = root.canonicalize().unwrap();
    let dot = root.join(".specman");
    std::mem::forget(temp);
    WorkspacePaths::new(root, dot)
}

#[test]
fn render_constraint_group_includes_inline_refs() {
    let workspace = make_workspace();
    fs::create_dir_all(workspace.spec_dir().join("render_refs")).unwrap();
    fs::write(
        workspace.spec_dir().join("render_refs").join("spec.md"),
        r#"---
name: render_refs
version: '1.0.0'
---
# Concept: A
Context for A.
!constraints.a:
- Must do X (see [Concept: B](#concept-b)).

# Concept: B
Details of B.
"#,
    )
    .unwrap();

    let indexer = FilesystemStructureIndexer::new(FilesystemWorkspaceLocator::new(
        workspace.dot_specman().to_path_buf(),
    ));
    let index = indexer.build_once_with_workspace(&workspace).unwrap();

    let group_id = index
        .list_constraint_groups()
        .into_iter()
        .find(|id| id.group == "constraints.a")
        .expect("constraints.a not found");
    
    let rendered = index.render_constraint_group(&group_id).unwrap();

    assert!(rendered.contains("# Concept: A"), "Should contain parent heading A\n{rendered}");
    assert!(rendered.contains("Context for A"), "Should contain parent content\n{rendered}");
    assert!(rendered.contains("# Concept: B"), "Should contain referenced heading B\n{rendered}");
    assert!(rendered.contains("Details of B"), "Should contain referenced content B\n{rendered}");
}

#[test]
fn render_constraint_group_is_transitive() {
    let workspace = make_workspace();
    fs::create_dir_all(workspace.spec_dir().join("transitive")).unwrap();
    fs::write(
        workspace.spec_dir().join("transitive").join("spec.md"),
        r#"---
name: transitive
version: '1.0.0'
---
# Root
!c.root:
- Link to [Child](#child).

# Child
Link to [Grandchild](#grandchild).

# Grandchild
Deep content.
"#,
    )
    .unwrap();

    let indexer = FilesystemStructureIndexer::new(FilesystemWorkspaceLocator::new(
        workspace.dot_specman().to_path_buf(),
    ));
    let index = indexer.build_once_with_workspace(&workspace).unwrap();

    let group_id = index
        .list_constraint_groups()
        .into_iter()
        .find(|id| id.group == "c.root")
        .unwrap();

    let rendered = index.render_constraint_group(&group_id).unwrap();

    assert!(rendered.contains("# Root"), "{rendered}");
    assert!(rendered.contains("# Child"), "{rendered}");
    assert!(rendered.contains("# Grandchild"), "Should contain recursive reference Grandchild\n{rendered}");
}
