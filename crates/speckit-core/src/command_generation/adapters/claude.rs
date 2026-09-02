//! Claude Code Command Adapter
//!
//! Formats commands for Claude Code following its frontmatter specification.
//! File path: .claude/commands/specx/<id>.md
//! Frontmatter: name, description, allowed-tools, category, tags

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::{escape_yaml_value, format_tags_array};

/// Base tools allowed by Speckit-generated Claude Code commands.
const SPECKIT_CLI_ALLOWED_TOOLS: &str = "Bash(speckit:*)";

/// Explore is the one command that may delegate read-only investigation to
/// Claude Code's native subagent runner.
const SPECKIT_EXPLORE_ALLOWED_TOOLS: &str = "Bash(speckit:*), Task";

pub struct ClaudeAdapter;

impl ToolCommandAdapter for ClaudeAdapter {
    fn tool_id(&self) -> &str {
        "claude"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".claude/commands/specx/{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        let allowed_tools = if content.id == "explore" {
            SPECKIT_EXPLORE_ALLOWED_TOOLS
        } else {
            SPECKIT_CLI_ALLOWED_TOOLS
        };

        format!(
            "---\nname: {}\ndescription: {}\nallowed-tools: {}\ncategory: {}\ntags: {}\n---\n\n{}\n",
            escape_yaml_value(&content.name),
            escape_yaml_value(&content.description),
            allowed_tools,
            escape_yaml_value(&content.category),
            format_tags_array(&content.tags),
            content.body
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(id: &str) -> CommandContent {
        CommandContent {
            id: id.into(),
            name: "Test".into(),
            description: "Test command".into(),
            category: "Test".into(),
            tags: vec![],
            body: "Instructions".into(),
        }
    }

    #[test]
    fn explore_command_allows_native_task_delegation() {
        let rendered = ClaudeAdapter.format_file(&command("explore"));

        assert!(rendered.contains("allowed-tools: Bash(speckit:*), Task\n"));
    }

    #[test]
    fn commands_are_written_to_the_specx_namespace() {
        assert_eq!(
            ClaudeAdapter.get_file_path("implement"),
            ".claude/commands/specx/implement.md"
        );
    }

    #[test]
    fn other_commands_keep_their_existing_allowlist() {
        let rendered = ClaudeAdapter.format_file(&command("implement"));

        assert!(rendered.contains("allowed-tools: Bash(speckit:*)\n"));
        assert!(!rendered.contains("allowed-tools: Bash(speckit:*), Task\n"));
    }
}
