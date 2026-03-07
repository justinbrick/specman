use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use tempfile::TempDir;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_specman"))
}

fn init_workspace(root: &std::path::Path) {
    let mut cmd = cli();
    cmd.args(["init", root.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn status_empty_workspace_is_ok() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    init_workspace(temp.path());

    let mut cmd = cli();
    cmd.current_dir(temp.path()).arg("status");

    cmd.assert()
        .success()
        .stdout(contains("Global Status: PASS"));
    Ok(())
}

#[test]
fn status_with_local_broken_link_fails() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    init_workspace(temp.path());

    // Create a spec with broken link
    let spec_dir = temp.path().join("spec/my-spec");
    fs::create_dir_all(&spec_dir)?;

    let spec_content = r#"---
name: my-spec
brief: Test spec
version: 1.0.0
---
# My Spec

## Structure

Artifact structure details.

[Broken Link](missing.md)
"#;
    fs::write(spec_dir.join("spec.md"), spec_content)?;

    let mut cmd = cli();
    cmd.current_dir(temp.path()).arg("status").arg("--local");

    cmd.assert()
        .failure()
        .code(65)
        .stdout(contains("Global Status: FAIL"))
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
    let spec_content = r#"---
name: network-spec
brief: Network spec
version: 1.0.0
---
# Network Spec

## Structure

Artifact structure details.

[Example](https://example.invalid/resource)
"#;
    fs::write(spec_dir.join("spec.md"), spec_content)?;

    let mut cmd = cli();
    cmd.current_dir(temp.path()).arg("status").arg("--local");

    cmd.assert()
        .success()
        .stdout(contains("Global Status: PASS"));

    Ok(())
}

#[test]
fn status_includes_compliance_scan_root_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    init_workspace(temp.path());

    let spec_dir = temp.path().join("spec/core");
    fs::create_dir_all(&spec_dir)?;
    let spec_content = r#"---
name: core
version: 1.0.0
---
# Core

## Concept: SpecMan Structure

!concept-specman-structure.referencing.validation:
- Implementations that index relationships from inline links MUST provide a method to validate the referenced destinations and report any invalid references.
"#;
    fs::write(spec_dir.join("spec.md"), spec_content)?;

    let impl_dir = temp.path().join("impl/lib");
    fs::create_dir_all(&impl_dir)?;
    let impl_content = r#"---
name: lib
spec: spec://core
location: ../../src/lib
version: 1.0.0
---
# Lib
"#;
    fs::write(impl_dir.join("impl.md"), impl_content)?;

    let code_dir = temp.path().join("src/lib");
    fs::create_dir_all(&code_dir)?;
    fs::write(
        code_dir.join("indexer.rs"),
        "// [ENSURES: concept-specman-structure.referencing.validation:CHECK]\n",
    )?;

    let mut cmd = cli();
    cmd.current_dir(temp.path()).arg("status");

    cmd.assert()
        .success()
        .stdout(contains("Global Status: PASS"))
        .stdout(contains("[Compliance] Scan root:"));

    Ok(())
}
