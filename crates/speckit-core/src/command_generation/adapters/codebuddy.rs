//! CodeBuddy Command Adapter
//!
//! Formats commands for CodeBuddy following its frontmatter specification.
//! File path: .codebuddy/commands/opsx/<id>.md
//! Frontmatter: name, description, argument-hint

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct CodebuddyAdapter;

impl ToolCommandAdapter for CodebuddyAdapter {
    fn tool_id(&self) -> &str {
        "codebuddy"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".codebuddy/commands/opsx/{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\nname: {}\ndescription: {}\nargument-hint: \"[command arguments]\"\n---\n\n{}\n",
            escape_yaml_value(&content.name),
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
