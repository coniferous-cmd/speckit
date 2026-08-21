//! Zoo Code Command Adapter
//!
//! Formats commands for Zoo Code following its workflow specification.
//! Zoo Code uses markdown headers instead of YAML frontmatter.
//! File path: .roo/commands/opsx-<id>.md
//! Format: Markdown header with description

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};

pub struct RoocodeAdapter;

impl ToolCommandAdapter for RoocodeAdapter {
    fn tool_id(&self) -> &str {
        "roocode"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".roo/commands/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "# {}\n\n{}\n\n{}\n",
            content.name, content.description, content.body
        )
    }
}
