use std::fs;
use std::thread;
use std::time::Duration;

use specman::{FilesystemStructureIndexer, FilesystemWorkspaceLocator, WorkspacePaths};
use tempfile::tempdir;

fn make_workspace() -> WorkspacePaths {
    let temp = tempdir().unwrap();
    let root = temp.path().join("workspace");
    fs::create_dir_all(root.join(".specman").join("scratchpad")).unwrap();
    fs::create_dir_all(root.join("spec")).unwrap();
    fs::create_dir_all(root.join("impl")).unwrap();

    // Persist tempdir for the duration of the test by leaking it.
    let root = root.canonicalize().unwrap();
    let dot = root.join(".specman");
    std::mem::forget(temp);
    WorkspacePaths::new(root, dot)
}

fn write_minimal_spec(workspace: &WorkspacePaths, name: &str) {
    let dir = workspace.spec_dir().join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("spec.md"),
        format!(
            "---\nname: {name}\nversion: '1.0.0'\n---\n# {name}\n\n## A\nHello\n"
        ),
    )
    .unwrap();
}

fn write_minimal_impl(workspace: &WorkspacePaths, name: &str) {
    let dir = workspace.impl_dir().join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("impl.md"),
        format!(
            "---\nname: {name}\nversion: '1.0.0'\nspec: spec://demo\n---\n# {name}\n\n## A\nHello\n"
        ),
    )
    .unwrap();
}

fn read_json(path: &std::path::Path) -> serde_json::Value {
    let content = fs::read_to_string(path).unwrap();
    serde_json::from_str(&content).unwrap()
}

#[test]
fn cached_index_creates_expected_files() {
    let workspace = make_workspace();
    write_minimal_spec(&workspace, "demo");

    let indexer = FilesystemStructureIndexer::new(FilesystemWorkspaceLocator::new(
        workspace.dot_specman().to_path_buf(),
    ));

    indexer.build_cached_with_workspace(&workspace).unwrap();

    assert!(workspace.dot_specman().join("root_fingerprint").is_file());
    assert!(
        workspace
            .dot_specman()
            .join("cache/index/manifest.json")
            .is_file()
    );
    assert!(
        workspace
            .dot_specman()
            .join("cache/index/index.v1.json")
            .is_file()
    );
}

#[test]
fn cache_hit_does_not_rewrite_manifest() {
    let workspace = make_workspace();
    write_minimal_spec(&workspace, "demo");
    write_minimal_impl(&workspace, "demo-impl");

    let indexer = FilesystemStructureIndexer::new(FilesystemWorkspaceLocator::new(
        workspace.dot_specman().to_path_buf(),
    ));

    indexer.build_cached_with_workspace(&workspace).unwrap();
    let manifest_path = workspace.dot_specman().join("cache/index/manifest.json");
    let before = read_json(&manifest_path);
    let before_ts = before["generated_at_unix_ms"].as_u64().unwrap();

    thread::sleep(Duration::from_millis(10));

    indexer.build_cached_with_workspace(&workspace).unwrap();
    let after = read_json(&manifest_path);
    let after_ts = after["generated_at_unix_ms"].as_u64().unwrap();

    assert_eq!(before_ts, after_ts);
}

#[test]
fn cache_invalidates_on_artifact_change() {
    let workspace = make_workspace();
    write_minimal_spec(&workspace, "demo");

    let indexer = FilesystemStructureIndexer::new(FilesystemWorkspaceLocator::new(
        workspace.dot_specman().to_path_buf(),
    ));

    indexer.build_cached_with_workspace(&workspace).unwrap();
    let manifest_path = workspace.dot_specman().join("cache/index/manifest.json");
    let before = read_json(&manifest_path);
    let before_ts = before["generated_at_unix_ms"].as_u64().unwrap();

    thread::sleep(Duration::from_millis(10));

    let spec_path = workspace.spec_dir().join("demo").join("spec.md");
    let mut content = fs::read_to_string(&spec_path).unwrap();
    content.push_str("\nMore\n");
    fs::write(&spec_path, content).unwrap();

    indexer.build_cached_with_workspace(&workspace).unwrap();
    let after = read_json(&manifest_path);
    let after_ts = after["generated_at_unix_ms"].as_u64().unwrap();

    assert!(after_ts >= before_ts);
    assert_ne!(after_ts, before_ts);
}

#[test]
fn corrupt_index_json_triggers_rebuild() {
    let workspace = make_workspace();
    write_minimal_spec(&workspace, "demo");

    let indexer = FilesystemStructureIndexer::new(FilesystemWorkspaceLocator::new(
        workspace.dot_specman().to_path_buf(),
    ));

    indexer.build_cached_with_workspace(&workspace).unwrap();

    let index_path = workspace.dot_specman().join("cache/index/index.v1.json");
    fs::write(&index_path, "{not valid json").unwrap();

    indexer.build_cached_with_workspace(&workspace).unwrap();

    let content = fs::read_to_string(&index_path).unwrap();
    serde_json::from_str::<serde_json::Value>(&content).unwrap();
}

#[test]
fn scratch_pads_are_excluded_from_persisted_cache() {
    let workspace = make_workspace();
    write_minimal_spec(&workspace, "demo");

    let scratch_dir = workspace
        .dot_specman()
        .join("scratchpad")
        .join("persist-test");
    fs::create_dir_all(&scratch_dir).unwrap();
    fs::write(
        scratch_dir.join("scratch.md"),
        "---\ntarget: spec/demo/spec.md\n---\n# Scratch\n\n## Notes\nHi\n",
    )
    .unwrap();

    let indexer = FilesystemStructureIndexer::new(FilesystemWorkspaceLocator::new(
        workspace.dot_specman().to_path_buf(),
    ));

    let index = indexer.build_cached_with_workspace(&workspace).unwrap();

    // Returned index still includes scratch artifacts.
    assert!(
        index
            .artifacts
            .keys()
            .any(|k| k.kind == specman::ArtifactKind::ScratchPad)
    );

    // Persisted manifest excludes scratch.
    let manifest_path = workspace.dot_specman().join("cache/index/manifest.json");
    let manifest = read_json(&manifest_path);
    for entry in manifest["artifacts"].as_array().unwrap() {
        assert_ne!(entry["kind"].as_str().unwrap(), "ScratchPad");
        let path = entry["workspace_path"].as_str().unwrap();
        assert!(!path.starts_with(".specman/scratchpad/"));
    }

    // Persisted index excludes scratch.
    let index_path = workspace.dot_specman().join("cache/index/index.v1.json");
    let persisted = read_json(&index_path);
    let artifacts = persisted["artifacts"].as_array().unwrap();
    assert!(
        artifacts
            .iter()
            .all(|a| a["key"]["kind"].as_str().unwrap() != "ScratchPad")
    );
}

#[test]
fn cache_lock_fails_fast() {
    let workspace = make_workspace();
    write_minimal_spec(&workspace, "demo");

    let indexer = FilesystemStructureIndexer::new(FilesystemWorkspaceLocator::new(
        workspace.dot_specman().to_path_buf(),
    ));

    // Create the cache dir and lock file before calling.
    let cache_dir = workspace.dot_specman().join("cache/index");
    fs::create_dir_all(&cache_dir).unwrap();
    fs::write(cache_dir.join(".lock"), "locked").unwrap();

    let err = indexer
        .build_cached_with_workspace(&workspace)
        .expect_err("should fail fast when cache locked");

    let msg = err.to_string();
    assert!(msg.contains("locked"), "{msg}");
}
