//! Onboarding command hints.
//!
//! The commands shown to a user after setup are limited to the workflows
//! their profile actually installs.

/// A single onboarding hint.
#[derive(Debug, Clone)]
pub struct OnboardingCommand {
    pub workflow: String,
    pub command: String,
    pub description: String,
}

/// Maximum description length the welcome screen can render.
pub const DESCRIPTION_BUDGET: usize = 17;

/// Ordered onboarding hints. Each entry is shown only when its workflow is
/// installed, so the list follows the change lifecycle: start, then build,
/// then implement.
fn onboarding_commands() -> Vec<OnboardingCommand> {
    vec![
        OnboardingCommand {
            workflow: "propose".to_string(),
            command: "/opsx:propose".to_string(),
            description: "Start a change".to_string(),
        },
        OnboardingCommand {
            workflow: "new".to_string(),
            command: "/opsx:new".to_string(),
            description: "Scaffold a change".to_string(),
        },
        OnboardingCommand {
            workflow: "continue".to_string(),
            command: "/opsx:continue".to_string(),
            description: "Next artifact".to_string(),
        },
        OnboardingCommand {
            workflow: "apply".to_string(),
            command: "/opsx:apply".to_string(),
            description: "Implement tasks".to_string(),
        },
    ]
}

/// Returns the onboarding hints for the installed workflows, in lifecycle order.
/// Returns an empty vector when none of the onboarding workflows are installed.
pub fn get_onboarding_commands(workflows: &[&str]) -> Vec<OnboardingCommand> {
    let installed: std::collections::HashSet<&str> = workflows.iter().copied().collect();
    onboarding_commands()
        .into_iter()
        .filter(|entry| installed.contains(entry.workflow.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_onboarding_commands_empty() {
        let result = get_onboarding_commands(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_onboarding_commands_subset() {
        let result = get_onboarding_commands(&["propose", "new"]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].workflow, "propose");
        assert_eq!(result[1].workflow, "new");
    }

    #[test]
    fn test_get_onboarding_commands_order() {
        let result = get_onboarding_commands(&["apply", "propose"]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].workflow, "propose");
        assert_eq!(result[1].workflow, "apply");
    }
}
