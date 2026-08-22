//! Referenced-store index assembly.
//!
//! A root's `speckit/config.yaml` may declare `references:` -- store ids
//! whose specs the root's work draws on. This module builds an INDEX of
//! those stores' specs, rendered as XML or markdown for agent consumption.
//! Content is never inlined; root resolution is never affected; problems
//! degrade to `warning` diagnostics instead of failing generation.

use serde::{Deserialize, Serialize};

/// Maximum rendered index size in UTF-8 bytes (shared with project context cap).
const MAX_RENDERED_INDEX_SIZE: usize = 50 * 1024;

/// A single spec entry in a referenced store index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceSpecEntry {
    pub id: String,
    pub summary: String,
}

/// A diagnostic entry for store status reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

/// An entry in the referenced-store index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceIndexEntry {
    pub store_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specs: Option<Vec<ReferenceSpecEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch: Option<String>,
    #[serde(default)]
    pub status: Vec<StoreDiagnostic>,
}

/// Build a warning diagnostic.
pub fn warning(code: &str, message: &str, fix: &str) -> StoreDiagnostic {
    StoreDiagnostic {
        severity: "warning".to_string(),
        code: code.to_string(),
        message: message.to_string(),
        target: Some("references".to_string()),
        fix: Some(fix.to_string()),
    }
}

/// Check whether a remote string is safe to paste into a shell command.
pub fn is_shell_safe_remote(remote: &str) -> bool {
    if remote.is_empty() || remote.starts_with('-') {
        return false;
    }
    remote
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "@:/._~+-".contains(c))
}

/// Build a registration fix hint for an unresolved store.
pub fn register_fix(id: &str, remote: Option<&str>) -> String {
    if let Some(r) = remote
        && is_shell_safe_remote(r) {
            let checkout = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("speckit")
                .join(id);
            let quoted = format!("'{}'", checkout.display());
            return format!(
                "git clone -- {} {} && speckit store register {} --id {}",
                r, quoted, quoted, id
            );
        }
    format!(
        "Get a checkout from a teammate and run: speckit store register <path> --id {}",
        id
    )
}

/// Drop a CommonMark closing sequence (`## Purpose ##`).
fn strip_closing_sequence(title: &str) -> String {
    let trimmed_end = title.trim_end();
    let hash_end = trimmed_end.trim_end_matches('#');
    if hash_end == trimmed_end {
        return trimmed_end.to_string();
    }
    // Only strip if there's a space before the closing hashes
    if hash_end.ends_with(' ') {
        hash_end.trim().to_string()
    } else {
        trimmed_end.to_string()
    }
}

/// Parse an ATX heading title, returning `None` if the line is not a heading.
fn parse_heading_title(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let mut level = 0;
    for c in trimmed.chars() {
        if c == '#' {
            level += 1;
        } else {
            break;
        }
    }
    if level == 0 || level >= 6 {
        return None;
    }
    let rest = &trimmed[level..];
    if !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let title = rest.trim_start();
    if title.is_empty() {
        return None;
    }
    Some(strip_closing_sequence(title))
}

/// Extract the first line of the `## Purpose` section from markdown.
///
/// Tolerant extraction: scans for the heading directly, fence-aware.
pub fn extract_first_purpose_line(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut in_purpose = false;
    let mut fence_marker: Option<&str> = None;

    for line in &lines {
        // Check for fence markers
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker = if trimmed.starts_with("```") {
                "```"
            } else {
                "~~~"
            };
            match fence_marker {
                None => fence_marker = Some(marker),
                Some(fm) => {
                    if fm == marker {
                        fence_marker = None;
                    }
                }
            }
            continue;
        }
        if fence_marker.is_some() {
            continue;
        }

        if let Some(title) = parse_heading_title(line) {
            if in_purpose {
                return String::new();
            }
            in_purpose = title.to_lowercase() == "purpose";
            continue;
        }
        if in_purpose && !line.trim().is_empty() {
            return line.trim().to_string();
        }
    }

    String::new()
}

/// Sanitize a string for inline rendering: strip control characters, limit length.
pub fn sanitize_inline(value: &str, max_length: usize) -> String {
    let flattened: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>();
    let trimmed = flattened.trim().to_string();
    if trimmed.len() > max_length {
        format!("{}...", &trimmed[..max_length])
    } else {
        trimmed
    }
}

/// Build a fetch recipe for a store id.
pub fn fetch_recipe(store_id: &str) -> String {
    format!("speckit show <spec-id> --type spec --store {}", store_id)
}

/// Format a spec entry as a markdown list item.
fn spec_line(spec: &ReferenceSpecEntry) -> String {
    let id = sanitize_inline(&spec.id, 100);
    if spec.summary.is_empty() {
        format!("  - {}", id)
    } else {
        format!("  - {}: {}", id, spec.summary)
    }
}

/// Render a single reference index entry's lines.
fn render_entry_lines(entry: &ReferenceIndexEntry) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(ref root) = entry.root {
        lines.push(format!("Store {} ({}):", entry.store_id, root));
        if let Some(ref specs) = entry.specs {
            for spec in specs {
                lines.push(spec_line(spec));
            }
        }
        if let Some(ref fetch) = entry.fetch {
            lines.push(format!("  Fetch: {}", fetch));
        }
        for diagnostic in &entry.status {
            lines.push(format!("  Note: {}", diagnostic.message));
            if let Some(ref fix) = diagnostic.fix {
                lines.push(format!("  Fix: {}", fix));
            }
        }
    } else {
        for diagnostic in &entry.status {
            lines.push(format!("Store {}: {}", entry.store_id, diagnostic.message));
            if let Some(ref fix) = diagnostic.fix {
                lines.push(format!("  Fix: {}", fix));
            }
        }
    }

    lines
}

/// Pure renderer for the artifact-instructions XML block.
pub fn render_referenced_stores_block(entries: &[ReferenceIndexEntry]) -> String {
    let mut lines = vec![
        "<referenced_stores>".to_string(),
        "<!-- Read-only upstream context. Fetch what you need; cite what you use. -->".to_string(),
    ];

    for entry in entries {
        lines.extend(render_entry_lines(entry));
    }

    lines.push("</referenced_stores>".to_string());
    lines.join("\n")
}

/// Pure renderer for the apply-instructions markdown section.
pub fn render_referenced_stores_section(entries: &[ReferenceIndexEntry]) -> String {
    let mut lines = vec![
        "### Referenced Stores".to_string(),
        String::new(),
        "Read-only upstream context. Fetch what you need; cite what you use.".to_string(),
        String::new(),
    ];

    for entry in entries {
        lines.extend(render_entry_lines(entry));
    }

    lines.join("\n")
}

/// Measure the byte size of the rendered XML block.
fn rendered_byte_size(entries: &[ReferenceIndexEntry]) -> usize {
    render_referenced_stores_block(entries).len()
}

/// Configuration for assembling a reference index.
#[derive(Debug, Clone)]
pub struct AssembleReferenceIndexInput {
    pub references: Vec<DeclarationEntry>,
    pub resolved_root_path: String,
    pub include_specs: bool,
}

/// A reference declaration from config.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclarationEntry {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
}

/// Build the referenced-store index from declaration entries.
///
/// This is a simplified version that builds entries without requiring
/// filesystem access to referenced stores. The full version in the
/// TypeScript implementation performs registry reads and spec collection,
/// but this port provides the rendering and data structures.
pub fn build_reference_index_entries(
    declarations: &[DeclarationEntry],
    resolved_root_path: &str,
    include_specs: bool,
) -> Vec<ReferenceIndexEntry> {
    let mut entries = Vec::new();

    for decl in declarations {
        // Skip self-references by id
        if decl.id == resolved_root_path {
            continue;
        }

        if !is_kebab_id_check(&decl.id) {
            entries.push(ReferenceIndexEntry {
                store_id: decl.id.clone(),
                root: None,
                specs: None,
                fetch: None,
                status: vec![warning(
                    "reference_invalid_id",
                    &format!("Reference '{}' is not a valid store id.", decl.id),
                    "Use kebab-case store ids in the references list.",
                )],
            });
            continue;
        }

        // In the full implementation, this would look up the registry
        // and collect spec entries. For the port, we provide unresolved entries.
        entries.push(ReferenceIndexEntry {
            store_id: decl.id.clone(),
            root: None,
            specs: None,
            fetch: if include_specs {
                Some(fetch_recipe(&decl.id))
            } else {
                None
            },
            status: vec![warning(
                "reference_unresolved",
                &format!(
                    "Referenced store '{}' is not registered on this machine.",
                    decl.id
                ),
                &register_fix(&decl.id, decl.remote.as_deref()),
            )],
        });
    }

    // Apply budget truncation
    if include_specs && rendered_byte_size(&entries) > MAX_RENDERED_INDEX_SIZE {
        truncate_entries_to_budget(&mut entries);
    }

    entries
}

/// Truncate spec lists to fit within the rendered byte budget.
fn truncate_entries_to_budget(entries: &mut Vec<ReferenceIndexEntry>) {
    // First pass: find which entries need truncation and how much
    let mut truncations = Vec::new();
    for (idx, entry) in entries.iter().enumerate() {
        if let Some(ref specs) = entry.specs
            && specs.len() > 10 {
                truncations.push((idx, specs.len()));
            }
    }

    // Second pass: apply truncations
    let entries_count = entries.len().max(1);
    for (idx, _original_len) in truncations {
        let entry = &mut entries[idx];
        let original_specs = match entry.specs.take() {
            Some(s) => s,
            None => continue,
        };
        let original_len = original_specs.len();
        let mut low = 0;
        let mut high = original_len;

        // Simple truncation: keep halving until it fits
        // Note: We estimate size locally instead of calling rendered_byte_size
        // to avoid borrow conflicts
        while low < high {
            let mid = (low + high).div_ceil(2);
            entry.specs = Some(original_specs[..mid].to_vec());
            // Simple heuristic: check if this entry alone exceeds budget
            let entry_size = serde_json::to_string(&entry).map(|s| s.len()).unwrap_or(0);
            if entry_size > MAX_RENDERED_INDEX_SIZE / entries_count {
                high = mid - 1;
            } else {
                low = mid;
            }
        }

        entry.specs = Some(original_specs[..low].to_vec());
        if low < original_len {
            entry.status.push(warning(
                "reference_index_truncated",
                &format!(
                    "Referenced store '{}' index truncated at the 50KB budget ({} of {} specs listed).",
                    entry.store_id, low, original_len
                ),
                &format!(
                    "List the rest directly: speckit list --specs --store {}",
                    entry.store_id
                ),
            ));
        }
    }
}

/// Check kebab-case id validity.
fn is_kebab_id_check(value: &str) -> bool {
    crate::id::is_kebab_id(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_shell_safe_remote() {
        assert!(is_shell_safe_remote("git@github.com:user/repo.git"));
        assert!(is_shell_safe_remote("https://github.com/user/repo.git"));
        assert!(!is_shell_safe_remote("--upload-pack=evil"));
        assert!(!is_shell_safe_remote("path with spaces"));
        assert!(!is_shell_safe_remote(""));
    }

    #[test]
    fn test_extract_first_purpose_line() {
        let md = "# My Spec\n\n## Purpose\nThis is the purpose.\n\n## Requirements\n";
        assert_eq!(extract_first_purpose_line(md), "This is the purpose.");
    }

    #[test]
    fn test_extract_first_purpose_line_fenced() {
        let md =
            "# My Spec\n\n```\n## Purpose\nNot the purpose.\n```\n\n## Purpose\nReal purpose.\n";
        assert_eq!(extract_first_purpose_line(md), "Real purpose.");
    }

    #[test]
    fn test_extract_first_purpose_line_empty() {
        let md = "# My Spec\n\n## Purpose\n\n## Requirements\n";
        assert_eq!(extract_first_purpose_line(md), "");
    }

    #[test]
    fn test_sanitize_inline() {
        assert_eq!(sanitize_inline("hello", 100), "hello");
        assert_eq!(sanitize_inline("he\nllo", 100), "he llo");
        assert_eq!(
            sanitize_inline("a".repeat(200).as_str(), 10),
            "aaaaaaaaaa..."
        );
    }

    #[test]
    fn test_render_referenced_stores_block() {
        let entries = vec![ReferenceIndexEntry {
            store_id: "my-store".to_string(),
            root: Some("/path/to/store".to_string()),
            specs: Some(vec![ReferenceSpecEntry {
                id: "auth".to_string(),
                summary: "Authentication spec".to_string(),
            }]),
            fetch: Some("speckit show <spec-id> --type spec --store my-store".to_string()),
            status: vec![],
        }];
        let rendered = render_referenced_stores_block(&entries);
        assert!(rendered.contains("<referenced_stores>"));
        assert!(rendered.contains("Store my-store"));
        assert!(rendered.contains("auth: Authentication spec"));
        assert!(rendered.contains("</referenced_stores>"));
    }

    #[test]
    fn test_render_referenced_stores_section() {
        let entries = vec![ReferenceIndexEntry {
            store_id: "test".to_string(),
            root: None,
            specs: None,
            fetch: None,
            status: vec![warning("test_code", "test message", "test fix")],
        }];
        let rendered = render_referenced_stores_section(&entries);
        assert!(rendered.contains("### Referenced Stores"));
        assert!(rendered.contains("Store test: test message"));
    }

    #[test]
    fn test_register_fix() {
        let fix = register_fix("my-store", Some("git@github.com:org/repo.git"));
        assert!(fix.contains("git clone"));
        assert!(fix.contains("speckit store register"));

        let fix_no_remote = register_fix("my-store", None);
        assert!(fix_no_remote.contains("teammate"));
    }
}
