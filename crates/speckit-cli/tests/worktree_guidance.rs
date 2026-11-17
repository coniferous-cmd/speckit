//! Worktree guidance is additive and instruction queries are read-only.
use std::path::Path;
use std::process::{Command, Output};

fn cli(home: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_speckit"))
        .current_dir(cwd)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("APPDATA", home.join("config"))
        .env("LOCALAPPDATA", home.join("data"))
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn git(root: &Path, args: &[&str]) -> String {
    success(
        Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .unwrap(),
    )
}

fn files(root: &Path) -> Vec<std::path::PathBuf> {
    fn visit(root: &Path, result: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().unwrap() == ".git" {
                continue;
            }
            result.push(path.clone());
            if path.is_dir() {
                visit(&path, result);
            }
        }
    }
    let mut result = Vec::new();
    visit(root, &mut result);
    result.sort();
    result
}

#[test]
fn worktree_guidance_matrix_and_selected_store_are_read_only() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    let root = home.join("project");
    let caller = home.join("caller");
    std::fs::create_dir_all(caller.join("speckit/changes")).unwrap();
    // Opposing caller config must never override an explicitly selected store.
    std::fs::write(
        caller.join("speckit/config.yaml"),
        "schema: spec-driven\nuse-worktree: true\n",
    )
    .unwrap();
    let schema = root.join("speckit/schemas/spec-driven");
    let change = root.join("speckit/changes/example");
    std::fs::create_dir_all(schema.join("templates")).unwrap();
    std::fs::create_dir_all(&change).unwrap();
    std::fs::create_dir_all(root.join("speckit/specs")).unwrap();
    std::fs::write(schema.join("schema.yaml"), "name: spec-driven\nversion: 1\nartifacts:\n  - id: tasks\n    generates: tasks.md\n    description: Tasks\n    template: tasks.md\n    requires: []\nimplement:\n  requires: [tasks]\n  tracks: tasks.md\n  instruction: Preserve this custom instruction.\n").unwrap();
    std::fs::write(schema.join("templates/tasks.md"), "# Tasks\n").unwrap();
    std::fs::write(change.join(".speckit.yaml"), "schema: spec-driven\n").unwrap();
    std::fs::write(root.join("speckit/config.yaml"), "schema: spec-driven\n").unwrap();
    git(&root, &["init"]);
    git(
        &root,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "baseline",
        ],
    );
    success(cli(
        home,
        &caller,
        &[
            "store",
            "register",
            root.to_str().unwrap(),
            "--id",
            "team",
            "--yes",
            "--json",
        ],
    ));

    for filename in ["config.yaml", "config.yml"] {
        for preference in [None, Some(false), Some(true)] {
            for state in ["ready", "blocked", "all_done"] {
                for name in ["config.yaml", "config.yml"] {
                    let path = root.join("speckit").join(name);
                    if path.exists() {
                        std::fs::remove_file(path).unwrap();
                    }
                }
                let setting = preference
                    .map(|v| format!("use-worktree: {v}\n"))
                    .unwrap_or_default();
                std::fs::write(root.join("speckit").join(filename), format!("schema: spec-driven\ncontext: Keep project context.\noperations:\n  implement:\n    guidance: [Keep operation guidance.]\n{setting}")).unwrap();
                let tasks = change.join("tasks.md");
                if state == "blocked" {
                    if tasks.exists() {
                        std::fs::remove_file(&tasks).unwrap();
                    }
                } else {
                    std::fs::write(
                        &tasks,
                        if state == "ready" {
                            "- [ ] pending\n"
                        } else {
                            "- [x] complete\n"
                        },
                    )
                    .unwrap();
                }
                let branches = git(&root, &["show-ref", "--heads"]);
                let worktrees = git(&root, &["worktree", "list", "--porcelain"]);
                let status = git(&root, &["status", "--porcelain", "--untracked-files=all"]);
                let before_files = files(&root);
                for store in [false, true] {
                    let mut args = vec!["instructions", "implement", "--change", "example"];
                    if store {
                        args.extend(["--store", "team"]);
                    }
                    let cwd = if store { &caller } else { &root };
                    let text = success(cli(home, cwd, &args));
                    args.push("--json");
                    let json: serde_json::Value =
                        serde_json::from_str(&success(cli(home, cwd, &args))).unwrap();
                    let enabled = preference == Some(true) && state == "ready";
                    assert_eq!(json["state"], state);
                    assert_eq!(json.get("useWorktree").is_some(), enabled);
                    assert_eq!(json.get("workspaceInstruction").is_some(), enabled);
                    assert_eq!(text.contains("### Workspace Preparation"), enabled);
                    assert_eq!(text.contains("dedicated Git worktree"), enabled);
                    assert_eq!(json["context"], "Keep project context.");
                    assert_eq!(
                        json["operation_guidance"],
                        serde_json::json!(["Keep operation guidance."])
                    );
                    if state == "ready" {
                        assert_eq!(json["instruction"], "Preserve this custom instruction.");
                        assert_eq!(json["progress"]["remaining"], 1);
                        assert_eq!(json["tasks"][0]["done"], false);
                    }
                    if enabled {
                        assert_eq!(json["useWorktree"], true);
                        let guidance = json["workspaceInstruction"].as_str().unwrap();
                        assert!(guidance.contains("change 'example'"));
                        assert!(text.contains(guidance));
                    }
                }
                assert_eq!(branches, git(&root, &["show-ref", "--heads"]));
                assert_eq!(worktrees, git(&root, &["worktree", "list", "--porcelain"]));
                assert_eq!(
                    status,
                    git(&root, &["status", "--porcelain", "--untracked-files=all"])
                );
                assert_eq!(before_files, files(&root));
            }
        }
    }
}
