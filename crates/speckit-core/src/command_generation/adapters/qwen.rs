//! Qwen Code Command Adapter
//!
//! Formats commands for Qwen Code following its Markdown custom command
//! specification. Qwen Code has deprecated TOML commands in favor of
//! Markdown files with YAML frontmatter.
//! File path: .qwen/commands/opsx-<id>.md
//! Format: Markdown with description frontmatter

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct QwenAdapter;

impl ToolCommandAdapter for QwenAdapter {
    fn tool_id(&self) -> &str {
        "qwen"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".qwen/commands/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\ndescription: {}\n---\n\n{}\n",
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
