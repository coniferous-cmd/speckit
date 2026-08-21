//! Command Code Command Adapter
//!
//! Command Code reads custom slash commands from `.commandcode/commands/`. The
//! command name is the markdown filename without its `.md` extension, so
//! `opsx-<id>.md` registers `/opsx-<id>` — the same flat naming Cursor and
//! OpenCode use.

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};

pub struct CommandCodeAdapter;

/// Injects `$ARGUMENTS` after `**Input**:` headings if not already present.
fn inject_command_code_args(body: &str) -> String {
    if body.contains("$ARGUMENTS") || body.contains("$@") {
        return body.to_string();
    }

    // Look for **Input**: heading and inject arguments after it
    if let Some(pos) = body.find("**Input**") {
        // Find the end of the line containing **Input**
        let line_end = body[pos..]
            .find('\n')
            .map(|i| pos + i)
            .unwrap_or(body.len());

        let mut result = String::with_capacity(body.len() + 50);
        result.push_str(&body[..line_end]);
        result.push_str("\n**Provided arguments**: $ARGUMENTS");
        result.push_str(&body[line_end..]);
        result
    } else {
        body.to_string()
    }
}

impl ToolCommandAdapter for CommandCodeAdapter {
    fn tool_id(&self) -> &str {
        "command-code"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".commandcode/commands/opsx-{}.md", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!("{}\n", inject_command_code_args(&content.body))
    }
}
