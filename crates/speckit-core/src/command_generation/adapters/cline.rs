//! Cline Command Adapter
//!
//! Formats commands for Cline following its workflow specification.
//! Cline uses markdown headers instead of YAML frontmatter.
//! File path: .clinerules/workflows/opsx-<id>.md
//! Format: Markdown header with description

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};

pub struct ClineAdapter;

impl ToolCommandAdapter for ClineAdapter {
    fn tool_id(&self) -> &str {
        "cline"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".clinerules/workflows/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "# {}\n\n{}\n\n{}\n",
            content.name, content.description, content.body
        )
    }
}
