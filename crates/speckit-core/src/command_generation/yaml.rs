//! Shared YAML frontmatter helpers for command adapters.
//!
//! Several tool adapters emit YAML frontmatter and need to escape
//! user-facing strings (name, description, category, tags) so the
//! generated file stays valid YAML.

/// Escapes a string value for safe YAML output.
///
/// Always emits a double-quoted scalar. Quoting unconditionally keeps the
/// value a string no matter what it holds: an unquoted `true`, `null` or
/// `123` would round-trip as a boolean, null or number, and an unquoted
/// value opening with a block indicator (`|`, `>`) or containing `: `
/// is not valid YAML at all.
///
/// Inside the quotes it escapes everything that cannot appear verbatim in a
/// double-quoted scalar: backslash, double quote, line feed, carriage
/// return, and the non-printable characters YAML's `c-printable` production
/// excludes (C0 controls, DEL and C1 controls).
pub fn escape_yaml_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 16);
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            // C0 controls (except tab \x09, LF \x0a, CR \x0d), DEL \x7f, C1 controls \u{80}-\u{9f}
            '\x00'..='\x08' | '\x0b' | '\x0c' | '\x0e'..='\x1f' | '\x7f' => {
                escaped.push_str(&format!("\\x{:02x}", ch as u32));
            }
            '\u{80}'..='\u{9f}' => {
                escaped.push_str(&format!("\\x{:02x}", ch as u32));
            }
            _ => escaped.push(ch),
        }
    }
    format!("\"{}\"", escaped)
}

/// Formats a tags array as a YAML array with proper escaping.
pub fn format_tags_array(tags: &[String]) -> String {
    let escaped_tags: Vec<String> = tags.iter().map(|tag| escape_yaml_value(tag)).collect();
    format!("[{}]", escaped_tags.join(", "))
}
