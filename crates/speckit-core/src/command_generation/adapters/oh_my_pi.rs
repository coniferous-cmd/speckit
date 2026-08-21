//! Oh My Pi (OMP) Command Adapter
//!
//! Formats commands for Oh My Pi following its slash command specification.
//! File path: .omp/commands/opsx-<id>.md
//! Frontmatter: description
//!
//! OMP uses the filename (minus .md) as the slash command name, so
//! opsx-propose.md -> /opsx-propose. $@ is injected after **Input**:
//! headings so user-supplied arguments are visible to the agent.

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};
use crate::command_generation::yaml::escape_yaml_value;

pub struct OhMyPiAdapter;

/// Injects `$@` after `**Input**:` headings if not already present.
fn inject_omp_args(body: &str) -> String {
    if body.contains("$@") || body.contains("$ARGUMENTS") {
        return body.to_string();
    }

    if let Some(pos) = body.find("**Input**") {
        let line_end = body[pos..]
            .find('\n')
            .map(|i| pos + i)
            .unwrap_or(body.len());

        let mut result = String::with_capacity(body.len() + 50);
        result.push_str(&body[..line_end]);
        result.push_str("\n**Provided arguments**: $@");
        result.push_str(&body[line_end..]);
        result
    } else {
        body.to_string()
    }
}

impl ToolCommandAdapter for OhMyPiAdapter {
    fn tool_id(&self) -> &str {
        "oh-my-pi"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".omp/commands/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "---\ndescription: {}\n---\n\n{}\n",
            escape_yaml_value(&content.description),
            inject_omp_args(&content.body)
        )
    }
}
