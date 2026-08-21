//! GitHub Copilot Command Adapter
//!
//! Formats commands for GitHub Copilot following its .prompt.md specification.
//! File path: .github/prompts/opsx-<id>.prompt.md
//! Frontmatter: description

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct GithubCopilotAdapter;

impl ToolCommandAdapter for GithubCopilotAdapter {
    fn tool_id(&self) -> &str {
        "github-copilot"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".github/prompts/opsx-{}.prompt.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\ndescription: {}\n---\n\n{}\n",
            escape_yaml_value(&content.description),
            content.body
        )
    }
}
