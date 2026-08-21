//! Kilo Code Command Adapter
//!
//! Formats commands for Kilo Code following its workflow specification.
//! Kilo Code workflows don't use frontmatter.
//! File path: .kilocode/workflows/opsx-<id>.md
//! Format: Plain markdown without frontmatter

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};

pub struct KilocodeAdapter;

impl ToolCommandAdapter for KilocodeAdapter {
    fn tool_id(&self) -> &str {
        "kilocode"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".kilocode/workflows/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!("{}\n", content.body)
    }
}
