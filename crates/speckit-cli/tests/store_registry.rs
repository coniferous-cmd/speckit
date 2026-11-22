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
    assert_eq!(context["root"]["storeId"], "team");
    assert_eq!(context["root"]["path"], registered["store"]["root"]);
    let rerun = success(home, &["store", "register", &root, "--json"]);
    assert_eq!(rerun["registry"]["alreadyRegistered"], true);
    let doctor = success(home, &["store", "doctor", "team", "--json"]);
    assert_eq!(doctor["stores"][0]["metadata"]["valid"], true);
    assert_eq!(doctor["stores"][0]["speckitRoot"]["healthy"], true);
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
        success(home, &["context", "--store", "custom-id", "--json"])["root"]["storeId"],
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
    assert_eq!(context["root"]["storeId"], "team");
    assert!(!temp.path().join("speckit/config.yaml").exists());
}

#[test]
fn setup_links_a_store_as_the_fallback_for_local_planning() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().join("code");
    let store = temp.path().join("planning");
    let config = project.join("speckit/config.yaml");
    std::fs::create_dir_all(project.join("speckit/specs")).unwrap();
    std::fs::create_dir_all(project.join("speckit/changes/archive")).unwrap();
    std::fs::write(project.join("speckit/specs/.gitkeep"), "").unwrap();
    let original = "# Project configuration\nschema: spec-driven\ncontext: |-\n  Project context.";
    std::fs::write(&config, original).unwrap();

    let args = [
        "store",
        "setup",
        "team",
        "--path",
        store.to_str().unwrap(),
        "--json",
    ];
    let setup = success(&project, &args);
    assert_eq!(setup["status"][0]["code"], "store_project_linked");
    let linked = format!("{original}\nstore: team\n");
    assert_eq!(std::fs::read_to_string(&config).unwrap(), linked);
    assert!(
        !std::fs::read_to_string(store.join("speckit/config.yaml"))
            .unwrap()
            .contains("store:")
    );

    let context = success(&project, &["context", "--json"]);
    assert_eq!(context["root"]["storeId"], "team");
    assert_eq!(
        Path::new(context["root"]["path"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        store.canonicalize().unwrap()
    );
    let created = success(&project, &["new", "change", "external-change", "--json"]);
    assert_eq!(created["change"]["id"], "external-change");
    assert!(
        store
            .join("speckit/changes/external-change/.speckit.yaml")
            .is_file()
    );
    assert!(!project.join("speckit/changes/external-change").exists());

    let assert_local_root = || {
        let context = success(&project, &["context", "--json"]);
        assert!(context["root"].get("storeId").is_none());
        assert_eq!(
            Path::new(context["root"]["path"].as_str().unwrap())
                .canonicalize()
                .unwrap(),
            project.canonicalize().unwrap(),
        );
    };
    let spec = project.join("speckit/specs/local-capability/spec.md");
    std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
    std::fs::write(&spec, "# Local spec\n").unwrap();
    assert_local_root();
    std::fs::remove_file(&spec).unwrap();

    let archive = project.join("speckit/changes/archive/archived-change");
    std::fs::create_dir(&archive).unwrap();
    assert_local_root();
    std::fs::remove_dir(&archive).unwrap();

    std::fs::create_dir(project.join("speckit/changes/local-change")).unwrap();
    assert_local_root();
    success(
        &project,
        &["new", "change", "another-local-change", "--json"],
    );
    assert!(
        project
            .join("speckit/changes/another-local-change/.speckit.yaml")
            .is_file()
    );
    assert!(!store.join("speckit/changes/another-local-change").exists());

    let retry = success(&project, &args);
    assert_eq!(retry["registry"]["alreadyRegistered"], true);
    assert_eq!(std::fs::read_to_string(&config).unwrap(), linked);

    let other = temp.path().join("other");
    let second = success(
        &project,
        &[
            "store",
            "setup",
            "other",
            "--path",
            other.to_str().unwrap(),
            "--json",
        ],
    );
    assert_eq!(second["registry"]["registered"], true);
    assert_eq!(second["status"][0]["code"], "store_project_link_skipped");
    assert_eq!(std::fs::read_to_string(&config).unwrap(), linked);
    let explicit = success(&project, &["context", "--store", "other", "--json"]);
    assert_eq!(explicit["root"]["storeId"], "other");
}

#[test]
fn setup_supports_flow_config_and_yaml_like_store_ids() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().join("code");
    let store = temp.path().join("planning");
    let config = project.join("speckit/config.yml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "{schema: spec-driven, context: hello}\n").unwrap();

    success(
        &project,
        &[
            "store",
            "setup",
            "123",
            "--path",
            store.to_str().unwrap(),
            "--json",
        ],
    );
    let linked = speckit_core::project_config::read_project_config(&project).unwrap();
    assert_eq!(linked.schema, "spec-driven");
    assert_eq!(linked.context.as_deref(), Some("hello"));
    assert_eq!(linked.store.as_deref(), Some("123"));
    assert_eq!(
        success(&project, &["context", "--json"])["root"]["storeId"],
        "123"
    );
    assert!(!project.join("speckit/config.yaml").exists());
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
