//! Command Surface Resolution
//!
//! Determines how each AI tool surfaces Speckit commands: via an adapter-
//! backed slash-command system, via skill invocation, or not at all.

// Compatibility facade. Keep the historical module path, but delegate to the
// registry-backed implementation so it can never drift from the actual
// adapter set used by command generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    Both,
    Skills,
    Commands,
}

impl Delivery {
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "both" => Some(Self::Both),
            "skills" => Some(Self::Skills),
            "commands" => Some(Self::Commands),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandInvocation {
    pub prefix: String,
    pub separator: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSurfaceCapability {
    AdapterBacked,
    SkillsInvocable,
    None,
}

fn to_generation_delivery(delivery: Delivery) -> crate::command_generation::Delivery {
    match delivery {
        Delivery::Both => crate::command_generation::Delivery::Both,
        Delivery::Skills => crate::command_generation::Delivery::Skills,
        Delivery::Commands => crate::command_generation::Delivery::Commands,
    }
}

pub fn resolve_command_invocation(tool_id: &str) -> Option<CommandInvocation> {
    crate::command_generation::resolve_command_invocation(tool_id).map(|invocation| {
        CommandInvocation {
            // The legacy facade exposes the logical namespace (`opsx`), not
            // the tool-specific literal prefix (`/`, `@`, ...).
            prefix: "opsx".to_string(),
            separator: match invocation.style {
                crate::command_generation::CommandInvocationStyle::Namespaced => ":".to_string(),
                crate::command_generation::CommandInvocationStyle::Flat => "-".to_string(),
            },
        }
    })
}

pub fn resolve_command_surface_capability(tool_id: &str) -> CommandSurfaceCapability {
    match crate::command_generation::resolve_command_surface_capability(tool_id) {
        crate::command_generation::CommandSurfaceCapability::AdapterBacked => {
            CommandSurfaceCapability::AdapterBacked
        }
        crate::command_generation::CommandSurfaceCapability::SkillsInvocable => {
            CommandSurfaceCapability::SkillsInvocable
        }
        crate::command_generation::CommandSurfaceCapability::None => CommandSurfaceCapability::None,
    }
}

pub fn should_generate_skills_for_tool(tool_id: &str, delivery: Delivery) -> bool {
    crate::command_generation::should_generate_skills_for_tool(
        tool_id,
        to_generation_delivery(delivery),
    )
}
pub fn should_remove_skills_for_tool(tool_id: &str, delivery: Delivery) -> bool {
    crate::command_generation::should_remove_skills_for_tool(
        tool_id,
        to_generation_delivery(delivery),
    )
}
pub fn should_generate_commands_for_tool(tool_id: &str, delivery: Delivery) -> bool {
    crate::command_generation::should_generate_commands_for_tool(
        tool_id,
        to_generation_delivery(delivery),
    )
}
pub fn should_reconcile_command_files_for_tool(tool_id: &str, delivery: Delivery) -> bool {
    crate::command_generation::should_reconcile_command_files_for_tool(
        tool_id,
        to_generation_delivery(delivery),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_is_adapter_backed() {
        assert_eq!(
            resolve_command_surface_capability("claude"),
            CommandSurfaceCapability::AdapterBacked
        );
    }

    #[test]
    fn every_registered_adapter_is_adapter_backed() {
        for adapter in crate::command_generation::CommandAdapterRegistry::global().get_all() {
            assert_eq!(
                resolve_command_surface_capability(adapter.tool_id()),
                CommandSurfaceCapability::AdapterBacked
            );
        }
    }

    #[test]
    fn codex_is_skills_invocable() {
        assert_eq!(
            resolve_command_surface_capability("codex"),
            CommandSurfaceCapability::SkillsInvocable
        );
    }

    #[test]
    fn unknown_tool_is_none() {
        assert_eq!(
            resolve_command_surface_capability("unknown-tool"),
            CommandSurfaceCapability::None
        );
    }

    #[test]
    fn resolve_command_invocation_for_adapter_backed() {
        let inv = resolve_command_invocation("claude").unwrap();
        assert_eq!(inv.prefix, "opsx");
        assert_eq!(inv.separator, ":");
    }

    #[test]
    fn resolve_command_invocation_none_for_unknown() {
        assert!(resolve_command_invocation("unknown").is_none());
    }

    #[test]
    fn should_generate_skills_default_delivery() {
        // With Both delivery, all tools get skills
        assert!(should_generate_skills_for_tool("claude", Delivery::Both));
        assert!(should_generate_skills_for_tool("codex", Delivery::Both));
        assert!(should_generate_skills_for_tool("unknown", Delivery::Both));
    }

    #[test]
    fn should_generate_skills_commands_delivery() {
        // With Commands delivery, only SkillsInvocable tools get skills
        assert!(!should_generate_skills_for_tool(
            "claude",
            Delivery::Commands
        ));
        assert!(should_generate_skills_for_tool("codex", Delivery::Commands));
        assert!(!should_generate_skills_for_tool(
            "unknown",
            Delivery::Commands
        ));
    }

    #[test]
    fn should_generate_commands_default_delivery() {
        // With Both delivery, adapter-backed tools get commands
        assert!(should_generate_commands_for_tool("claude", Delivery::Both));
        assert!(!should_generate_commands_for_tool("codex", Delivery::Both));
        assert!(!should_generate_commands_for_tool(
            "unknown",
            Delivery::Both
        ));
    }

    #[test]
    fn should_generate_commands_skills_delivery() {
        // With Skills delivery, no tools get commands
        assert!(!should_generate_commands_for_tool(
            "claude",
            Delivery::Skills
        ));
    }

    #[test]
    fn should_remove_skills_commands_delivery_adapter() {
        assert!(should_remove_skills_for_tool("claude", Delivery::Commands));
        assert!(!should_remove_skills_for_tool("codex", Delivery::Commands));
    }

    #[test]
    fn should_reconcile_command_files_skills_delivery() {
        assert!(should_reconcile_command_files_for_tool(
            "claude",
            Delivery::Skills
        ));
        assert!(!should_reconcile_command_files_for_tool(
            "codex",
            Delivery::Skills
        ));
    }

    #[test]
    fn delivery_from_str_roundtrip() {
        assert_eq!(Delivery::from_str("both"), Some(Delivery::Both));
        assert_eq!(Delivery::from_str("skills"), Some(Delivery::Skills));
        assert_eq!(Delivery::from_str("commands"), Some(Delivery::Commands));
        assert_eq!(Delivery::from_str("invalid"), None);
    }
}
