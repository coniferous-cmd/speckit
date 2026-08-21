//! Kiro Command Adapter
//!
//! Formats commands for Kiro following its .prompt.md specification.
//! File path: .kiro/prompts/opsx-<id>.prompt.md
//! Frontmatter: description

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct KiroAdapter;

impl ToolCommandAdapter for KiroAdapter {
    fn tool_id(&self) -> &str {
        "kiro"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".kiro/prompts/opsx-{}.prompt.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\ndescription: {}\n---\n\n{}\n",
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
