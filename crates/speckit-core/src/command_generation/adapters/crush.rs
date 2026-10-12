//! Crush Command Adapter
//!
//! Formats commands for Crush following its frontmatter specification.
//! File path: .crush/commands/specx/<id>.md
//! Frontmatter: name, description, category, tags

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::{escape_yaml_value, format_tags_array};

pub struct CrushAdapter;

impl ToolCommandAdapter for CrushAdapter {
    fn tool_id(&self) -> &str {
        "crush"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".crush/commands/specx/{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\nname: {}\ndescription: {}\ncategory: {}\ntags: {}\n---\n\n{}\n",
            escape_yaml_value(&content.name),
            escape_yaml_value(&content.description),
            escape_yaml_value(&content.category),
            format_tags_array(&content.tags),
            content.body
        )
    }
}
