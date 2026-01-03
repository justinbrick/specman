use std::fs;
use std::io::Write;
use std::path::Path;

use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn scratch_delete_accepts_unquoted_colon_headings() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = scaffold_workspace_with_colon_headings()?;
    let scratch_dir = workspace.path().join(".specman/scratchpad/revision-notes");

    let mut cmd = cli();
    cmd.arg("--workspace")
        .arg(workspace.path())
        .args(["scratch", "delete", "revision-notes"]);

    cmd.assert().success();

    assert!(!scratch_dir.exists());
    Ok(())
}

#[test]
fn scratch_delete_handles_dependency_objects() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = scaffold_workspace_with_dependency_object()?;
    let scratch_dir = workspace.path().join(".specman/scratchpad/child-notes");

    let mut cmd = cli();
    cmd.arg("--workspace")
        .arg(workspace.path())
        .args(["scratch", "delete", "child-notes"]);

    cmd.assert().success();

    assert!(!scratch_dir.exists());
    Ok(())
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_specman"))
}

fn scaffold_workspace_with_colon_headings() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let root = temp.path();

    fs::create_dir_all(root.join(".specman/scratchpad"))?;
    fs::create_dir_all(root.join("spec/demo-spec"))?;

    write_file(
        root.join("spec/demo-spec/spec.md"),
        r#"---
name: demo-spec
version: "1.0.0"
dependencies: []
---
Spec body.
"#,
    )?;

    write_file(
        root.join(".specman/scratchpad/revision-notes/scratch.md"),
        r#"---
name: revision-notes
target: spec/demo-spec/spec.md
branch: demo-spec/revision/revision-notes
work_type:
  revision:
    revised_headings:
      - Concept: Prompt Catalog
      - Concept: SpecMan Capability Parity
dependencies: []
---
Scratch body.
"#,
    )?;

    Ok(temp)
}

fn scaffold_workspace_with_dependency_object() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let root = temp.path();

    fs::create_dir_all(root.join(".specman/scratchpad"))?;
    fs::create_dir_all(root.join("spec/demo-spec"))?;

    write_file(
        root.join("spec/demo-spec/spec.md"),
        r#"---
name: demo-spec
version: "1.0.0"
dependencies: []
---
Spec body.
"#,
    )?;

    write_file(
        root.join(".specman/scratchpad/parent-notes/scratch.md"),
        r#"---
name: parent-notes
target: spec/demo-spec/spec.md
branch: demo-spec/feat/parent-notes
work_type:
  feat: {}
dependencies: []
---
Parent scratch body.
"#,
    )?;

    write_file(
        root.join(".specman/scratchpad/child-notes/scratch.md"),
        r#"---
name: child-notes
target: spec/demo-spec/spec.md
branch: demo-spec/revision/child-notes
work_type:
  revision:
    revised_headings:
      - Concept: Prompt Catalog
dependencies:
  - ref: scratch://parent-notes
    optional: true
---
Child scratch body.
"#,
    )?;

    Ok(temp)
}

fn write_file(path: impl AsRef<Path>, contents: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(path)?;
    file.write_all(contents.as_bytes())?;
    Ok(())
}
