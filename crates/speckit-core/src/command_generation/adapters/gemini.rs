//! Gemini CLI Command Adapter
//!
//! Formats commands for Gemini CLI following its TOML specification.
//! File path: .gemini/commands/opsx/<id>.toml
//! Format: TOML with description and prompt fields

use crate::command_generation::types::{CommandContent, ToolCommandAdapter};

pub struct GeminiAdapter;

/// TOML basic strings are escape-active: a backslash or double quote in the
/// value breaks the file if written raw. Newlines cannot appear in a
/// single-line basic string at all.
fn escape_toml_basic_string(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 16);
    for ch in value.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            // C0 controls (except tab, LF, CR) and DEL
            '\x00'..='\x08' | '\x0b' | '\x0c' | '\x0e'..='\x1f' | '\x7f' => {
                result.push_str(&format!("\\u{:04x}", ch as u32));
            }
            _ => result.push(ch),
        }
    }
    result
}

/// Multiline basic strings keep raw newlines and tabs, but backslashes are
/// still escape-active, any run of three quotes would end the string, and the
/// same control characters are invalid as in single-line basic strings.
fn escape_toml_multiline_basic_string(value: &str) -> String {
    // Normalize CRLF to LF
    let normalized = value.replace("\r\n", "\n");
    let mut result = String::with_capacity(normalized.len() + 16);

    for ch in normalized.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => {
                // Check if this starts a run of three or more quotes
                // We need to escape the first one to prevent """ from appearing
                result.push_str("\\\"");
            }
            '\r' => result.push_str("\\r"),
            // C0 controls (except LF and tab) and DEL
            '\x00'..='\x08' | '\x0b' | '\x0c' | '\x0e'..='\x1f' | '\x7f' => {
                result.push_str(&format!("\\u{:04x}", ch as u32));
            }
            _ => result.push(ch),
        }
    }
    result
}

impl ToolCommandAdapter for GeminiAdapter {
    fn tool_id(&self) -> &str {
        "gemini"
    }

    fn get_file_path(&self, command_id: &str) -> String {
        format!(".gemini/commands/opsx/{}.toml", command_id)
    }

    fn format_file(&self, content: &CommandContent) -> String {
        format!(
            "description = \"{}\"\n\nprompt = \"\"\"\n{}\n\"\"\"\n",
            escape_toml_basic_string(&content.description),
            escape_toml_multiline_basic_string(&content.body)
        )
    }
}
