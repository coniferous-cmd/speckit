//! Bob Shell Command Adapter
//!
//! Formats commands for Bob Shell following its markdown specification.
//! File path: .bob/commands/opsx-<id>.md
//! Frontmatter: description
//!
//! Bob uses the filename (minus .md) as the slash command name, so
//! opsx-propose.md -> /opsx-propose.

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct BobAdapter;

impl ToolCommandAdapter for BobAdapter {
    fn tool_id(&self) -> &str {
        "bob"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".bob/commands/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\ndescription: {}\nargument-hint: command arguments\n---\n\n{}\n",
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
