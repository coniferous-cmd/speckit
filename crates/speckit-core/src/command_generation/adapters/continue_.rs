//! Continue Command Adapter
//!
//! Formats commands for Continue following its .prompt specification.
//! File path: .continue/prompts/opsx-<id>.prompt
//! Frontmatter: name, description, invokable

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct ContinueAdapter;

impl ToolCommandAdapter for ContinueAdapter {
    fn tool_id(&self) -> &str {
        "continue"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".continue/prompts/opsx-{}.prompt", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\nname: {}\ndescription: {}\ninvokable: true\n---\n\n{}\n",
            escape_yaml_value(&format!("opsx-{}", content.id)),
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
