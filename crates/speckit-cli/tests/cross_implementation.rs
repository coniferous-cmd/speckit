//! Cross-layer integration tests for the CLI contracts shared with core.
//!
//! These tests deliberately execute the built binary.  Unit tests can prove
//! that either implementation works in isolation, but they do not catch
//! mismatches between root selection, the CLI adapters, persistence, and the
//! machine-readable output contract.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use speckit_core::root_selection::{
    ResolveSpeckitRootOptions, SpeckitRootSource, resolve_speckit_root,
};
use tempfile::TempDir;

fn run_cli(cwd: &Path, config_home: &Path, data_home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_speckit"))
        .current_dir(cwd)
        .env("HOME", config_home.parent().expect("config home parent"))
        .env("XDG_CONFIG_HOME", config_home)
        .env("XDG_DATA_HOME", data_home)
        .env("SPECKIT_TELEMETRY", "0")
        .args(args)
        .output()
        .expect("run speckit CLI")
}

fn json_stdout(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "CLI failed: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be exactly one JSON document: {error}\nstdout={}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn find_named_file(root: &Path, name: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(root).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.file_name().and_then(|value| value.to_str()) == Some(name) {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_named_file(&path, name) {
                return Some(found);
            }
        }
    }
    None
}

fn fixture() -> (TempDir, PathBuf, PathBuf, PathBuf, PathBuf) {
    let temp = TempDir::new().expect("temporary fixture");
    let project = temp.path().join("project");
    let nested = project.join("packages").join("app");
    // macOS `dirs` resolves config_dir from HOME, while Linux follows XDG.
    // Put the fixture at the conventional HOME/.config location and set both
    // variables so the child process is isolated on every supported platform.
    let config_home = temp.path().join(".config");
    let data_home = temp.path().join("data");
    std::fs::create_dir_all(project.join("speckit").join("specs")).unwrap();
    std::fs::create_dir_all(project.join("speckit").join("changes")).unwrap();
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir_all(&config_home).unwrap();
    std::fs::create_dir_all(&data_home).unwrap();
    (temp, project, nested, config_home, data_home)
}

#[test]
fn cli_context_and_core_resolve_the_same_nearest_root() {
    let (_temp, _project, nested, config_home, data_home) = fixture();

    let core_root = resolve_speckit_root(&ResolveSpeckitRootOptions {
        store: None,
        store_path: None,
        start_path: Some(nested.clone()),
        allow_implicit_root: Some(false),
        global_data_dir: Some(data_home.clone()),
    })
    .expect("core should find the fixture root");
    assert_eq!(core_root.source, SpeckitRootSource::Nearest);

    let output = run_cli(&nested, &config_home, &data_home, &["context", "--json"]);
    let json = json_stdout(&output);
    let cli_path = PathBuf::from(json["root"]["path"].as_str().unwrap())
        .canonicalize()
        .unwrap();
    assert_eq!(cli_path, core_root.path.canonicalize().unwrap());
    assert!(json["status"].as_array().unwrap().is_empty());
    assert!(String::from_utf8_lossy(&output.stdout).lines().count() > 1);
}

#[test]
fn workset_json_round_trips_between_create_list_and_disk() {
    let (_temp, project, nested, config_home, data_home) = fixture();
    let member = project.join("member");
    std::fs::create_dir_all(&member).unwrap();

    let member_arg = format!("app={}", nested.display());
    let member_arg_ref = member_arg.as_str();
    let created = run_cli(
        &project,
        &config_home,
        &data_home,
        &[
            "workset",
            "create",
            "dev-view",
            "--member",
            member_arg_ref,
            "--member",
            member.to_str().unwrap(),
            "--tool",
            "code",
            "--json",
        ],
    );
    let created_json = json_stdout(&created);
    assert_eq!(created_json["status"], serde_json::json!([]));
    assert_eq!(created_json["workset"]["name"], "dev-view");
    assert_eq!(created_json["workset"]["members"][0]["name"], "app");

    let listed = run_cli(
        &project,
        &config_home,
        &data_home,
        &["workset", "list", "--json"],
    );
    let listed_json = json_stdout(&listed);
    assert_eq!(listed_json["status"], serde_json::json!([]));
    assert_eq!(listed_json["worksets"].as_array().unwrap().len(), 1);
    assert_eq!(listed_json["worksets"][0], created_json["workset"]);

    let state_path = find_named_file(config_home.parent().unwrap(), "worksets.json")
        .expect("workset state should be persisted below the isolated HOME");
    let state: Value = serde_json::from_str(&std::fs::read_to_string(state_path).unwrap()).unwrap();
    assert_eq!(state.as_array().unwrap().len(), 1);
    assert_eq!(state[0], created_json["workset"]);
}

#[test]
fn json_mode_is_pure_across_root_and_workset_commands() {
    let (_temp, project, nested, config_home, data_home) = fixture();

    let context = run_cli(&nested, &config_home, &data_home, &["context", "--json"]);
    let _: Value = json_stdout(&context);
    assert!(!String::from_utf8_lossy(&context.stdout).contains("Note: Speckit collects"));

    let worksets = run_cli(
        &project,
        &config_home,
        &data_home,
        &["workset", "list", "--json"],
    );
    let json = json_stdout(&worksets);
    assert_eq!(json["worksets"], serde_json::json!([]));
    assert_eq!(json["status"], serde_json::json!([]));
}
