use speckit_core::project_config::{ProjectConfig, read_project_config};

#[test]
fn worktree_preference_roundtrips_and_uses_only_selected_root() {
    let temp = tempfile::tempdir().unwrap();
    for filename in ["config.yaml", "config.yml"] {
        for value in [None, Some(false), Some(true)] {
            let selected = temp.path().join(format!("{filename}-{value:?}"));
            let other = temp.path().join("other");
            for root in [&selected, &other] {
                std::fs::create_dir_all(root.join("speckit")).unwrap();
            }
            std::fs::write(
                other.join("speckit/config.yaml"),
                "schema: spec-driven\nuse-worktree: true\n",
            )
            .unwrap();
            let setting = value
                .map(|v| format!("use-worktree: {v}\n"))
                .unwrap_or_default();
            let yaml = format!("schema: spec-driven\n{setting}");
            std::fs::write(selected.join("speckit").join(filename), &yaml).unwrap();
            let config = read_project_config(&selected).unwrap();
            assert_eq!(config.use_worktree, value);
            let direct: ProjectConfig = serde_yaml::from_str(&yaml).unwrap();
            assert_eq!(direct.use_worktree, value);
            let serialized = serde_yaml::to_string(&config).unwrap();
            assert!(!serialized.contains("useWorktree"));
            assert_eq!(serialized.contains("use-worktree:"), value.is_some());
            let roundtrip: ProjectConfig = serde_yaml::from_str(&serialized).unwrap();
            assert_eq!(roundtrip.use_worktree, value);
            let prompt = speckit_core::config_prompts::serialize_config(&config);
            assert_eq!(prompt.contains("use-worktree:"), value.is_some());
            let roundtrip: ProjectConfig = serde_yaml::from_str(&prompt).unwrap();
            assert_eq!(roundtrip.use_worktree, value);
        }
    }
}
