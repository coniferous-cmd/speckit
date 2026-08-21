//! Antigravity Command Adapter
//!
//! Formats commands for Antigravity following its frontmatter specification.
//! File path: .agent/workflows/opsx-<id>.md
//! Frontmatter: description

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct AntigravityAdapter;

impl ToolCommandAdapter for AntigravityAdapter {
    fn tool_id(&self) -> &str {
        "antigravity"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".agent/workflows/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\ndescription: {}\n---\n\n{}\n",
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
