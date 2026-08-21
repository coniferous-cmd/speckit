//! Amazon Q Developer Command Adapter
//!
//! Formats commands for Amazon Q Developer following its frontmatter specification.
//! File path: .amazonq/prompts/opsx-<id>.md
//! Frontmatter: description
//!
//! Amazon Q surfaces these files as its prompt library rather than as slash
//! commands: the user types `@opsx-propose`, not `/opsx-propose`.

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct AmazonQAdapter;

impl ToolCommandAdapter for AmazonQAdapter {
    fn tool_id(&self) -> &str {
        "amazon-q"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".amazonq/prompts/opsx-{}.md", command_id)
    }

    fn invocation_prefix(&self) -> Option<&str> {
        Some("@")
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\ndescription: {}\n---\n\n{}\n",
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
