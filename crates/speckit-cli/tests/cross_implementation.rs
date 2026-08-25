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

fn json_error(output: &Output) -> Value {
    assert!(
        !output.status.success(),
        "CLI unexpectedly succeeded: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "failed JSON commands must emit one JSON document: {error}\\nstdout={}",
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

#[test]
fn change_lifecycle_json_contract_is_available_from_nested_directory() {
    let (_temp, project, nested, config_home, data_home) = fixture();

    let created = run_cli(
        &project,
        &config_home,
        &data_home,
        &["new", "change", "cli-lifecycle", "--json"],
    );
    let created_json = json_stdout(&created);
    assert_eq!(created_json["change"]["id"], "cli-lifecycle");
    assert_eq!(created_json["change"]["schema"], "spec-driven");
    assert!(
        project
            .join("speckit/changes/cli-lifecycle/.speckit.yaml")
            .is_file()
    );

    let status = run_cli(
        &nested,
        &config_home,
        &data_home,
        &["status", "--change", "cli-lifecycle", "--json"],
    );
    let status_json = json_stdout(&status);
    assert_eq!(status_json["change_name"], "cli-lifecycle");
    assert_eq!(status_json["artifacts"][0]["id"], "proposal");
    assert_eq!(status_json["artifacts"][0]["status"], "ready");

    let instructions = run_cli(
        &nested,
        &config_home,
        &data_home,
        &[
            "instructions",
            "proposal",
            "--change",
            "cli-lifecycle",
            "--json",
        ],
    );
    let instructions_json = json_stdout(&instructions);
    assert_eq!(instructions_json["artifact_id"], "proposal");
    assert_eq!(instructions_json["change_name"], "cli-lifecycle");
    assert!(
        instructions_json["resolved_output_path"]
            .as_str()
            .unwrap()
            .ends_with("cli-lifecycle/proposal.md")
    );
}

#[test]
fn change_commands_report_json_errors_without_using_user_configuration() {
    let (_temp, project, _nested, config_home, data_home) = fixture();

    let invalid_name = run_cli(
        &project,
        &config_home,
        &data_home,
        &["new", "change", "Invalid_Name", "--json"],
    );
    let invalid_json = json_error(&invalid_name);
    assert_eq!(invalid_json["status"][0]["code"], "invalid_name");

    let missing_change = run_cli(
        &project,
        &config_home,
        &data_home,
        &["status", "--change", "does-not-exist", "--json"],
    );
    assert!(!missing_change.status.success());
    assert!(String::from_utf8_lossy(&missing_change.stderr).contains("does-not-exist"));
}

#[test]
fn status_marks_specs_skipped_when_change_metadata_requests_it() {
    let (_temp, project, _nested, config_home, data_home) = fixture();
    let change_dir = project.join("speckit/changes/tooling-only");
    std::fs::create_dir_all(&change_dir).unwrap();
    std::fs::write(
        change_dir.join(".speckit.yaml"),
        "schema: spec-driven\nskip_specs: true\n",
    )
    .unwrap();
    for file in ["proposal.md", "design.md", "tasks.md"] {
        std::fs::write(change_dir.join(file), "complete\n").unwrap();
    }

    let output = run_cli(
        &project,
        &config_home,
        &data_home,
        &["status", "--change", "tooling-only", "--json"],
    );
    let json = json_stdout(&output);
    assert_eq!(json["artifacts"][1]["id"], "specs");
    assert_eq!(json["artifacts"][1]["status"], "skipped");
    assert_eq!(json["is_planning_complete"], true);
}

#[test]
fn archive_json_moves_completed_change_and_preserves_failed_changes() {
    let (_temp, project, _nested, config_home, data_home) = fixture();
    let changes = project.join("speckit/changes");

    let completed = changes.join("completed-change");
    std::fs::create_dir_all(&completed).unwrap();
    let archived = run_cli(
        &project,
        &config_home,
        &data_home,
        &[
            "archive",
            "completed-change",
            "--yes",
            "--skip-specs",
            "--json",
        ],
    );
    let archived_json = json_stdout(&archived);
    assert_eq!(archived_json["change"], "completed-change");
    assert!(Path::new(archived_json["path"].as_str().unwrap()).is_dir());
    assert!(!completed.exists());

    let incomplete = changes.join("incomplete-change");
    std::fs::create_dir_all(&incomplete).unwrap();
    std::fs::write(incomplete.join("tasks.md"), "- [ ] pending\n").unwrap();
    let rejected = run_cli(
        &project,
        &config_home,
        &data_home,
        &["archive", "incomplete-change", "--yes", "--json"],
    );
    let rejected_json = json_error(&rejected);
    assert_eq!(rejected_json["status"][0]["code"], "archive_failed");
    assert!(incomplete.is_dir());

    let bad_change = changes.join("bad-spec-change");
    let delta = bad_change.join("specs/cap-a");
    std::fs::create_dir_all(&delta).unwrap();
    std::fs::write(bad_change.join("tasks.md"), "- [x] done\n").unwrap();
    std::fs::write(
        delta.join("spec.md"),
        "## MODIFIED Requirements\n\n### Requirement: Missing\nContent.\n",
    )
    .unwrap();
    let main_spec = project.join("speckit/specs/cap-a/spec.md");
    std::fs::create_dir_all(main_spec.parent().unwrap()).unwrap();
    std::fs::write(
        &main_spec,
        "# Cap A\n\n## Requirements\n\n### Requirement: Existing\nContent.\n",
    )
    .unwrap();
    let spec_failure = run_cli(
        &project,
        &config_home,
        &data_home,
        &["archive", "bad-spec-change", "--yes", "--json"],
    );
    let failure_json = json_error(&spec_failure);
    assert_eq!(failure_json["status"][0]["code"], "archive_failed");
    assert!(bad_change.is_dir());
    assert_eq!(
        std::fs::read_to_string(main_spec).unwrap(),
        "# Cap A\n\n## Requirements\n\n### Requirement: Existing\nContent.\n"
    );
}

#[test]
fn init_and_update_keep_managed_skills_in_sync_without_touching_custom_skills() {
    let (_temp, project, _nested, config_home, data_home) = fixture();

    let initialized = run_cli(
        &project,
        &config_home,
        &data_home,
        &["init", ".", "--tools", "claude", "--force"],
    );
    assert!(
        initialized.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&initialized.stderr)
    );

    let managed = project.join(".claude/skills/speckit-explore/SKILL.md");
    let generated_by_init = std::fs::read_to_string(&managed).unwrap();
    let custom = project.join(".claude/skills/my-custom-skill/SKILL.md");
    let custom_contents =
        "---\nname: my-custom-skill\ndescription: user owned\n---\n\nDo not replace.\n";
    std::fs::create_dir_all(custom.parent().unwrap()).unwrap();
    std::fs::write(&custom, custom_contents).unwrap();
    std::fs::write(
        &managed,
        generated_by_init.replacen("generatedBy: \"", "generatedBy: \"stale-", 1),
    )
    .unwrap();

    let updated = run_cli(
        &project,
        &config_home,
        &data_home,
        &["update", ".", "--force"],
    );
    assert!(
        updated.status.success(),
        "update failed: {}",
        String::from_utf8_lossy(&updated.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&managed).unwrap(),
        generated_by_init
    );
    assert_eq!(std::fs::read_to_string(&custom).unwrap(), custom_contents);
}
