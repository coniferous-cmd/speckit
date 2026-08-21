//! Claude Code Command Adapter
//!
//! Formats commands for Claude Code following its frontmatter specification.
//! File path: .claude/commands/opsx/<id>.md
//! Frontmatter: name, description, allowed-tools, category, tags

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::{escape_yaml_value, format_tags_array};

/// Allowed tools constant matching the TypeScript SPECKIT_CLI_ALLOWED_TOOLS.
const SPECKIT_CLI_ALLOWED_TOOLS: &str = "Bash(speckit:*)";

pub struct ClaudeAdapter;

impl ToolCommandAdapter for ClaudeAdapter {
    fn tool_id(&self) -> &str {
        "claude"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".claude/commands/opsx/{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\nname: {}\ndescription: {}\nallowed-tools: {}\ncategory: {}\ntags: {}\n---\n\n{}\n",
            escape_yaml_value(&content.name),
            escape_yaml_value(&content.description),
            SPECKIT_CLI_ALLOWED_TOOLS,
            escape_yaml_value(&content.category),
            format_tags_array(&content.tags),
            content.body
        )
    }
}
