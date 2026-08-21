//! Auggie (Augment CLI) Command Adapter
//!
//! Formats commands for Auggie following its frontmatter specification.
//! File path: .augment/commands/opsx-<id>.md
//! Frontmatter: description, argument-hint

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct AuggieAdapter;

impl ToolCommandAdapter for AuggieAdapter {
    fn tool_id(&self) -> &str {
        "auggie"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".augment/commands/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\ndescription: {}\nargument-hint: command arguments\n---\n\n{}\n",
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
