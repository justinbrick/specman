use std::fs;
use std::io::Write;
use std::path::Path;

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn spec_dependencies_defaults_to_downstream() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = scaffold_workspace()?;
    let mut cmd = cli();
    cmd.arg("--workspace")
        .arg(workspace.path())
        .args(["spec", "dependencies", "alpha-spec"]);

    cmd.assert()
        .success()
        .stdout(contains(
            "Dependency tree (downstream) for specification 'alpha-spec'",
        ))
        .stdout(contains("scratch alpha-notes"))
        .stdout(contains("impl alpha-spec-rust"));
    Ok(())
}

#[test]
fn impl_dependencies_honors_upstream_flag() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = scaffold_workspace()?;
    let mut cmd = cli();
    cmd.arg("--workspace").arg(workspace.path()).args([
        "impl",
        "dependencies",
        "alpha-spec-rust",
        "--upstream",
    ]);

    cmd.assert()
        .success()
        .stdout(contains(
            "Dependency tree (upstream) for implementation 'alpha-spec-rust'",
        ))
        .stdout(contains("spec alpha-spec"));
    Ok(())
}

#[test]
fn spec_dependencies_rejects_conflicting_flags() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = scaffold_workspace()?;
    let mut cmd = cli();
    cmd.arg("--workspace").arg(workspace.path()).args([
        "spec",
        "dependencies",
        "alpha-spec",
        "--upstream",
        "--downstream",
    ]);

    cmd.assert().failure().code(64).stderr(contains(
        "use only one of --downstream, --upstream, or --all",
    ));
    Ok(())
}

#[test]
fn scratch_dependencies_renders_all_sections() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = scaffold_workspace()?;
    let mut cmd = cli();
    cmd.arg("--workspace").arg(workspace.path()).args([
        "scratch",
        "dependencies",
        "alpha-notes",
        "--all",
    ]);

    cmd.assert()
        .success()
        .stdout(contains(
            "Dependency tree (complete) for scratch pad 'alpha-notes'",
        ))
        .stdout(contains("Downstream"))
        .stdout(contains("Upstream"));
    Ok(())
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_specman"))
}

fn scaffold_workspace() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let root = temp.path();

    fs::create_dir_all(root.join(".specman/scratchpad"))?;
    fs::create_dir_all(root.join("spec/alpha-spec"))?;
    fs::create_dir_all(root.join("spec/beta-spec"))?;
    fs::create_dir_all(root.join("impl/alpha-spec-rust"))?;
    fs::create_dir_all(root.join(".specman/scratchpad/alpha-notes"))?;

    write_file(
        root.join("spec/alpha-spec/spec.md"),
        r#"---
name: alpha-spec
version: "1.0.0"
dependencies:
  - ../beta-spec/spec.md
---
Alpha specification.
"#,
    )?;

    write_file(
        root.join("spec/beta-spec/spec.md"),
        r#"---
name: beta-spec
version: "1.0.0"
dependencies: []
---
Beta specification.
"#,
    )?;

    write_file(
        root.join("impl/alpha-spec-rust/impl.md"),
        r#"---
name: alpha-spec-rust
version: "0.1.0"
spec: ../../spec/alpha-spec/spec.md
---
Implementation body.
"#,
    )?;

    write_file(
        root.join(".specman/scratchpad/alpha-notes/scratch.md"),
        r#"---
name: alpha-notes
target: spec/alpha-spec/spec.md
branch: alpha-spec/feat/alpha-notes
work_type:
  feat: {}
dependencies:
  - spec/alpha-spec/spec.md
---
Scratch content.
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
