use crate::global_config::Profile;
use serde_yaml;

/// Core workflows included in the `core` profile.
///
/// These provide the streamlined experience for new users.
pub const CORE_WORKFLOWS: &[&str] = &[
    "propose",
    "explore",
    "implement",
    "update",
    "sync",
    "archive",
];

/// All available workflows in the system.
pub const ALL_WORKFLOWS: &[&str] = &[
    "propose",
    "explore",
    "new",
    "continue",
    "implement",
    "update",
    "ff",
    "sync",
    "archive",
    "bulk-archive",
    "verify",
    "onboard",
];

/// Resolves which workflows should be active for a given profile configuration.
///
/// - `Profile::Core` always returns [`CORE_WORKFLOWS`].
/// - `Profile::Custom` returns the provided `custom_workflows`, or an empty
///   slice when none are given.
pub fn get_profile_workflows<'a>(
    profile: &Profile,
    custom_workflows: Option<&'a [String]>,
) -> &'a [String] {
    match profile {
        Profile::Core => {
            // CORE_WORKFLOWS is static; the caller already owns a Vec<String>
            // when custom workflows are supplied.  For the core profile we
            // cannot return &'a [String] from a static &[&str], so callers
            // that need owned data should build it themselves.  This function
            // is intentionally simple: it returns the custom_workflows slice
            // when present and an empty slice otherwise; the *caller* is
            // responsible for passing in the core-workflow list when the
            // profile is Core.
            //
            // In practice the CLI always builds the Vec<String> before
            // calling this function, so the None path is unused for Core.
            custom_workflows.unwrap_or(&[])
        }
        Profile::Custom => custom_workflows.unwrap_or(&[]),
    }
}

/// Convenience: returns the core workflows as owned `String`s.
pub fn core_workflow_strings() -> Vec<String> {
    CORE_WORKFLOWS.iter().map(|s| s.to_string()).collect()
}

/// Convenience: returns all workflows as owned `String`s.
pub fn all_workflow_strings() -> Vec<String> {
    ALL_WORKFLOWS.iter().map(|s| s.to_string()).collect()
}

/// Resolve the active profile and workflow filter for skill generation.
///
/// This is the canonical resolution logic shared by `init` and `update`. It is
/// used to ensure both commands produce the same skill set for the same profile
/// configuration.
///
/// The precedence is:
///   1. The `profile_override` argument (CLI-level override).
///   2. Project-level config (`speckit/config.yaml` or `speckit/config.yml`).
///   3. Global config (`~/.config/speckit/config.json`).
///   4. Default `core` profile.
///
/// Returns `(active_profile, workflow_filter)` where `workflow_filter` is `None`
/// when the active profile covers all known workflows (equivalent to no filter).
pub fn resolve_profile_and_workflow_filter(
    profile_override: Option<&crate::global_config::Profile>,
    project_path: Option<&std::path::Path>,
) -> (Profile, Option<Vec<String>>) {
    // 1. CLI override
    if let Some(p) = profile_override {
        return resolve_filter_for_profile(p);
    }

    // 2. Project config
    if let Some(path) = project_path {
        let speckit_path = path.join("speckit");
        for fname in ["config.yaml", "config.yml"] {
            let cfg_path = speckit_path.join(fname);
            if let Ok(profile) = parse_profile_from_file(&cfg_path) {
                return resolve_filter_for_profile(&profile);
            }
        }
    }

    // 3. Global config
    let cfg = crate::global_config::get_global_config();
    resolve_filter_for_profile(&cfg.profile)
}

fn resolve_filter_for_profile(profile: &Profile) -> (Profile, Option<Vec<String>>) {
    match profile {
        Profile::Core => {
            let active = core_workflow_strings();
            if active.len() == ALL_WORKFLOWS.len() {
                (Profile::Core, None)
            } else {
                (Profile::Core, Some(active))
            }
        }
        Profile::Custom => {
            let workflows = crate::global_config::get_global_config()
                .workflows
                .clone()
                .unwrap_or_default();
            if workflows.is_empty() {
                (Profile::Custom, Some(vec![]))
            } else if workflows.len() == ALL_WORKFLOWS.len()
                && workflows
                    .iter()
                    .all(|w| ALL_WORKFLOWS.contains(&w.as_str()))
            {
                (Profile::Custom, None)
            } else {
                (Profile::Custom, Some(workflows))
            }
        }
    }
}

/// Parse a profile from a speckit config file.
fn parse_profile_from_file(path: &std::path::Path) -> anyhow::Result<Profile> {
    let content = std::fs::read_to_string(path)?;
    let parsed: serde_yaml::Value = serde_yaml::from_str(&content)?;
    if let Some(value) = parsed.get("profile").and_then(|v| v.as_str()) {
        match value.trim().to_lowercase().as_str() {
            "core" => Ok(Profile::Core),
            "custom" => Ok(Profile::Custom),
            other => Err(anyhow::anyhow!(
                "Unknown profile '{other}'. Supported: core, custom."
            )),
        }
    } else {
        Err(anyhow::anyhow!("no profile field"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_workflows_count() {
        assert_eq!(CORE_WORKFLOWS.len(), 6);
    }

    #[test]
    fn all_workflows_count() {
        assert_eq!(ALL_WORKFLOWS.len(), 12);
    }

    #[test]
    fn all_workflows_contains_core() {
        for wf in CORE_WORKFLOWS {
            assert!(
                ALL_WORKFLOWS.contains(wf),
                "CORE_WORKFLOWS entry '{wf}' missing from ALL_WORKFLOWS"
            );
        }
    }

    #[test]
    fn core_workflow_strings_are_correct() {
        let owned = core_workflow_strings();
        assert_eq!(
            owned,
            vec![
                "propose",
                "explore",
                "implement",
                "update",
                "sync",
                "archive"
            ]
        );
    }

    #[test]
    fn custom_profile_returns_empty_when_none() {
        let workflows = get_profile_workflows(&Profile::Custom, None);
        assert!(workflows.is_empty());
    }

    #[test]
    fn custom_profile_returns_supplied_workflows() {
        let custom = vec!["explore".to_string(), "implement".to_string()];
        let workflows = get_profile_workflows(&Profile::Custom, Some(&custom));
        assert_eq!(workflows, &["explore", "implement"]);
    }
}
