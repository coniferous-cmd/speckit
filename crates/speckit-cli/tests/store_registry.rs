//! Regression coverage for store commands and lifecycle root selection sharing one registry.
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_speckit"))
        .args(args)
        .current_dir(home)
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("APPDATA", home.join("config"))
        .env("LOCALAPPDATA", home.join("data"))
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", home.join("gitconfig"))
        .env("GIT_AUTHOR_NAME", "Store Test")
        .env("GIT_AUTHOR_EMAIL", "store@example.invalid")
        .env("GIT_COMMITTER_NAME", "Store Test")
        .env("GIT_COMMITTER_EMAIL", "store@example.invalid")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn success(home: &Path, args: &[&str]) -> Value {
    let output = run(home, args);
    assert!(
        output.status.success(),
        "{args:?}: {}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn existing_root(home: &Path) -> String {
    let root = home.join("team");
    std::fs::create_dir_all(root.join("speckit/specs")).unwrap();
    std::fs::create_dir_all(root.join("speckit/changes/archive")).unwrap();
    std::fs::write(root.join("speckit/config.yaml"), "schema: spec-driven\n").unwrap();
    root.to_str().unwrap().to_owned()
}

#[test]
fn register_list_context_and_unregister_share_registry() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();
    let root = existing_root(home);
    let registered = success(
        home,
        &[
            "store", "register", &root, "--id", "team", "--yes", "--json",
        ],
    );
    assert_eq!(registered["store"]["id"], "team");
    assert!(home.join("data/speckit/stores/registry.yaml").is_file());
    assert!(Path::new(&root).join(".speckit-store/store.yaml").is_file());
    assert!(!Path::new(&root).join("store.yaml").exists());
    assert!(!home.join("config/speckit/store-registry.json").exists());
    let listed = success(home, &["store", "list", "--json"]);
    assert_eq!(listed["stores"][0]["root"], registered["store"]["root"]);
    let context = success(home, &["context", "--json", "--store", "team"]);
    assert_eq!(context["root"]["store_id"], "team");
    assert_eq!(context["root"]["path"], registered["store"]["root"]);
    let rerun = success(home, &["store", "register", &root, "--json"]);
    assert_eq!(rerun["registry"]["already_registered"], true);
    let doctor = success(home, &["store", "doctor", "team", "--json"]);
    assert_eq!(doctor["stores"][0]["metadata"]["valid"], true);
    assert_eq!(doctor["stores"][0]["speckit_root"]["healthy"], true);
    success(home, &["store", "unregister", "team", "--json"]);
    assert!(Path::new(&root).is_dir());
    assert_eq!(
        success(home, &["store", "list", "--json"])["stores"],
        serde_json::json!([])
    );
    assert!(
        !run(home, &["context", "--json", "--store", "team"])
            .status
            .success()
    );
}

#[test]
fn incomplete_root_is_rejected_without_registration_or_identity_creation() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("incomplete");
    std::fs::create_dir_all(&root).unwrap();
    let output = run(
        temp.path(),
        &[
            "store",
            "register",
            root.to_str().unwrap(),
            "--yes",
            "--json",
        ],
    );
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["status"][0]["code"], "unhealthy_store_root");
    assert!(!root.join(".speckit-store").exists());
    assert_eq!(
        success(temp.path(), &["store", "list", "--json"])["stores"],
        serde_json::json!([])
    );
}

#[test]
fn legacy_registration_can_be_explicitly_reregistered_without_overwriting_old_files() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();
    let root = existing_root(home);
    let registry = home.join("config/speckit/store-registry.json");
    std::fs::create_dir_all(registry.parent().unwrap()).unwrap();
    let legacy =
        serde_json::to_string(&serde_json::json!([{ "id": "custom-id", "root": root }])).unwrap();
    std::fs::write(&registry, &legacy).unwrap();
    let metadata = Path::new(&root).join("store.yaml");
    let old_metadata = "version: '1.0'\nid: custom-id\n";
    std::fs::write(&metadata, old_metadata).unwrap();
    success(
        home,
        &[
            "store",
            "register",
            &root,
            "--id",
            "custom-id",
            "--yes",
            "--json",
        ],
    );
    assert_eq!(
        success(home, &["context", "--store", "custom-id", "--json"])["root"]["store_id"],
        "custom-id"
    );
    assert_eq!(std::fs::read_to_string(registry).unwrap(), legacy);
    assert_eq!(std::fs::read_to_string(metadata).unwrap(), old_metadata);
}

#[test]
fn setup_is_visible_to_context() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("team");
    success(
        temp.path(),
        &[
            "store",
            "setup",
            "team",
            "--path",
            root.to_str().unwrap(),
            "--json",
        ],
    );
    let context = success(temp.path(), &["context", "--store", "team", "--json"]);
    assert_eq!(context["root"]["store_id"], "team");
}

#[test]
fn destructive_remove_requires_explicit_confirmation() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();
    let root = existing_root(home);
    success(home, &["store", "register", &root, "--yes", "--json"]);
    let denied = run(home, &["store", "remove", "team", "--json"]);
    assert!(!denied.status.success());
    let error: Value = serde_json::from_slice(&denied.stdout).unwrap();
    assert_eq!(
        error["status"][0]["code"],
        "store_remove_confirmation_required"
    );
    assert!(Path::new(&root).is_dir());
    assert_eq!(
        success(home, &["store", "list", "--json"])["stores"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    success(home, &["store", "remove", "team", "--yes", "--json"]);
    assert!(!Path::new(&root).exists());
}

#[test]
fn identity_creation_requires_confirmation_and_doctor_detects_corruption() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();
    let root = existing_root(home);
    let denied = run(home, &["store", "register", &root, "--json"]);
    assert!(!denied.status.success());
    assert!(!Path::new(&root).join(".speckit-store/store.yaml").exists());
    success(home, &["store", "register", &root, "--yes", "--json"]);
    std::fs::write(
        Path::new(&root).join(".speckit-store/store.yaml"),
        "invalid: metadata\n",
    )
    .unwrap();
    let doctor = success(home, &["store", "doctor", "team", "--json"]);
    assert_eq!(doctor["stores"][0]["metadata"]["valid"], false);
    assert!(!doctor["stores"][0]["status"].as_array().unwrap().is_empty());
}
