//! iFlow Command Adapter
//!
//! Formats commands for iFlow following its frontmatter specification.
//! File path: .iflow/commands/opsx-<id>.md
//! Frontmatter: name, id, category, description

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct IflowAdapter;

impl ToolCommandAdapter for IflowAdapter {
    fn tool_id(&self) -> &str {
        "iflow"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".iflow/commands/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\nname: {}\nid: {}\ncategory: {}\ndescription: {}\n---\n\n{}\n",
            escape_yaml_value(&format!("/opsx-{}", content.id)),
            escape_yaml_value(&format!("opsx-{}", content.id)),
            escape_yaml_value(&content.category),
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
