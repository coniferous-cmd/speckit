//! CoStrict Command Adapter
//!
//! Formats commands for CoStrict following its frontmatter specification.
//! File path: .cospec/speckit/commands/opsx-<id>.md
//! Frontmatter: description, argument-hint

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct CostrictAdapter;

impl ToolCommandAdapter for CostrictAdapter {
    fn tool_id(&self) -> &str {
        "costrict"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".cospec/speckit/commands/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\ndescription: {}\nargument-hint: command arguments\n---\n\n{}\n",
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
