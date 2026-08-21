//! Command Surface Resolution
//!
//! Determines how each AI tool surfaces Speckit commands: via an adapter-
//! backed slash-command system, via skill invocation, or not at all.

use std::sync::LazyLock;

/// The delivery mode for Speckit content to AI tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// Deliver both skills and commands.
    Both,
    /// Deliver only skills.
    Skills,
    /// Deliver only commands.
    Commands,
}

impl Delivery {
    /// Parses a delivery string from configuration.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "both" => Some(Self::Both),
            "skills" => Some(Self::Skills),
            "commands" => Some(Self::Commands),
            _ => None,
        }
    }
}

/// How the tool spells its Speckit commands.
#[derive(Debug, Clone)]
pub struct CommandInvocation {
    /// The prefix for command names (e.g., "opsx").
    pub prefix: String,
    /// The separator between prefix and command name (e.g., ":").
    pub separator: String,
}

/// The capability of a tool's command surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSurfaceCapability {
    /// Tool has an adapter-backed slash-command system.
    AdapterBacked,
    /// Tool invokes commands through its skill system (e.g., Codex).
    SkillsInvocable,
    /// Tool has no command surface.
    None,
}

/// Tool IDs that have adapter-backed command surfaces.
///
/// In the full implementation, this would be driven by a registry. Here we
/// enumerate the known adapter-backed tools.
static ADAPTER_BACKED_TOOLS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        "claude",
        "cursor",
        "cline",
        "github-copilot",
        "roocode",
        "kilocode",
        "continue",
        "auggie",
        "gemini",
        "qwen",
        "kiro",
    ]
});

/// Resolves the command invocation syntax for a tool.
pub fn resolve_command_invocation(tool_id: &str) -> Option<CommandInvocation> {
    if ADAPTER_BACKED_TOOLS.contains(&tool_id) {
        Some(CommandInvocation {
            prefix: "opsx".to_string(),
            separator: ":".to_string(),
        })
    } else {
        None
    }
}

/// Resolves the command surface capability for a tool.
pub fn resolve_command_surface_capability(tool_id: &str) -> CommandSurfaceCapability {
    if ADAPTER_BACKED_TOOLS.contains(&tool_id) {
        CommandSurfaceCapability::AdapterBacked
    } else if tool_id == "codex" {
        CommandSurfaceCapability::SkillsInvocable
    } else {
        CommandSurfaceCapability::None
    }
}

/// Whether skills should be generated for a tool given the delivery mode.
pub fn should_generate_skills_for_tool(tool_id: &str, delivery: Delivery) -> bool {
    delivery != Delivery::Commands
        || resolve_command_surface_capability(tool_id) == CommandSurfaceCapability::SkillsInvocable
}

/// Whether skills should be removed for a tool given the delivery mode.
pub fn should_remove_skills_for_tool(tool_id: &str, delivery: Delivery) -> bool {
    delivery == Delivery::Commands
        && resolve_command_surface_capability(tool_id) != CommandSurfaceCapability::SkillsInvocable
}

/// Whether commands should be generated for a tool given the delivery mode.
pub fn should_generate_commands_for_tool(tool_id: &str, delivery: Delivery) -> bool {
    delivery != Delivery::Skills
        && resolve_command_surface_capability(tool_id) == CommandSurfaceCapability::AdapterBacked
}

/// Whether command files should be reconciled (removed) for a tool given
/// the delivery mode.
pub fn should_reconcile_command_files_for_tool(tool_id: &str, delivery: Delivery) -> bool {
    delivery == Delivery::Skills
        && resolve_command_surface_capability(tool_id) == CommandSurfaceCapability::AdapterBacked
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
