//! OpenCode Command Adapter
//!
//! Formats commands for OpenCode following its frontmatter specification.
//! File path: .opencode/commands/opsx-<id>.md
//! Frontmatter: description

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct OpencodeAdapter;

impl ToolCommandAdapter for OpencodeAdapter {
    fn tool_id(&self) -> &str {
        "opencode"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".opencode/commands/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\ndescription: {}\n---\n\n{}\n",
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
