//! Trae Command Adapter
//!
//! Formats commands for Trae IDE following its command specification.
//! File path: .trae/commands/opsx-<id>.md
//! Frontmatter: name, description

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct TraeAdapter;

impl ToolCommandAdapter for TraeAdapter {
    fn tool_id(&self) -> &str {
        "trae"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".trae/commands/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\nname: {}\ndescription: {}\n---\n\n{}\n",
            escape_yaml_value(&content.name),
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
