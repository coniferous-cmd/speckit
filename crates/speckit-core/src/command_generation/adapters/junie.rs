//! Junie Command Adapter
//!
//! Formats commands for Junie following its frontmatter specification.
//! File path: .junie/commands/opsx-<id>.md
//! Frontmatter: description

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct JunieAdapter;

impl ToolCommandAdapter for JunieAdapter {
    fn tool_id(&self) -> &str {
        "junie"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".junie/commands/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\ndescription: {}\n---\n\n{}\n",
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
