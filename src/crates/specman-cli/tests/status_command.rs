use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use tempfile::TempDir;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_specman"))
}

fn init_workspace(root: &std::path::Path) {
    let mut cmd = cli();
    cmd.args(["init", root.to_str().unwrap()]).assert().success();
}

#[test]
fn status_empty_workspace_is_ok() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    init_workspace(temp.path());

    let mut cmd = cli();
    cmd.current_dir(temp.path()).arg("status");

    cmd.assert()
        .success()
        .stdout(contains("Workspace status: OK"));
    Ok(())
}

#[test]
fn status_with_local_broken_link_fails() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    init_workspace(temp.path());

    // Create a spec with broken link
    let spec_dir = temp.path().join("spec/my-spec");
    fs::create_dir_all(&spec_dir)?;

    let spec_content = r#"
---
name: my-spec
brief: Test spec
version: 1.0.0
---
# My Spec

[Broken Link](missing.md)
"#;
    fs::write(spec_dir.join("spec.md"), spec_content)?;

    let mut cmd = cli();
    cmd.current_dir(temp.path()).arg("status").arg("--local");

    cmd.assert()
        .failure()
        .code(65)
        .stdout(contains("Workspace status: FAIL"))
        .stdout(contains("missing filesystem target"));
    Ok(())
}

#[test]
fn status_local_skips_network() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    init_workspace(temp.path());

    let spec_dir = temp.path().join("spec/network-spec");
    fs::create_dir_all(&spec_dir)?;

    // We rely on the fact that SyntaxOnly will accept any syntactically valid URL
    // without checking reachability.
    let spec_content = r#"
---
name: network-spec
brief: Network spec
version: 1.0.0
---
# Network Spec

[Example](https://example.invalid/resource)
"#;
    fs::write(spec_dir.join("spec.md"), spec_content)?;

    let mut cmd = cli();
    cmd.current_dir(temp.path()).arg("status").arg("--local");

    cmd.assert()
        .success()
        .stdout(contains("Workspace status: OK"));

    Ok(())
}
