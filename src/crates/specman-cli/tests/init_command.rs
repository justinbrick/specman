use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn init_creates_workspace() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let target = temp.path().join("workspace");

    let mut cmd = cli();
    cmd.args(["init", target.to_str().unwrap()]);

    cmd.assert()
        .success()
        .stdout(contains("Initialized workspace at"));

    assert!(target.join(".specman").is_dir());
    assert!(target.join(".specman/cache").is_dir());
    assert!(target.join(".specman/scratchpad").is_dir());
    Ok(())
}

#[test]
fn init_dry_run_does_not_write() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let target = temp.path().join("workspace-dry-run");

    let mut cmd = cli();
    cmd.args(["init", target.to_str().unwrap(), "--dry-run"]);

    cmd.assert()
        .success()
        .stdout(contains("Dry run: would initialize workspace"));

    assert!(!target.join(".specman").exists());
    Ok(())
}

#[test]
fn init_rejects_nested_workspace() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let parent = temp.path().join("parent");
    let child = parent.join("child");
    std::fs::create_dir_all(parent.join(".specman"))?;

    let mut cmd = cli();
    cmd.args(["init", child.to_str().unwrap()]);

    cmd.assert()
        .failure()
        .code(64)
        .stderr(contains("ancestor workspace"));
    Ok(())
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_specman"))
}
