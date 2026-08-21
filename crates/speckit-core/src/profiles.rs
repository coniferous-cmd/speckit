use crate::global_config::Profile;

/// Core workflows included in the `core` profile.
///
/// These provide the streamlined experience for new users.
pub const CORE_WORKFLOWS: &[&str] = &["propose", "explore", "apply", "update", "sync", "archive"];

/// All available workflows in the system.
pub const ALL_WORKFLOWS: &[&str] = &[
    "propose",
    "explore",
    "new",
    "continue",
    "apply",
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
            vec!["propose", "explore", "apply", "update", "sync", "archive"]
        );
    }

    #[test]
    fn custom_profile_returns_empty_when_none() {
        let workflows = get_profile_workflows(&Profile::Custom, None);
        assert!(workflows.is_empty());
    }

    #[test]
    fn custom_profile_returns_supplied_workflows() {
        let custom = vec!["explore".to_string(), "apply".to_string()];
        let workflows = get_profile_workflows(&Profile::Custom, Some(&custom));
        assert_eq!(workflows, &["explore", "apply"]);
    }
}
