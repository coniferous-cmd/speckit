use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// A detected tool presence in the project filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedTool {
    /// The tool identifier (e.g. "claude", "cursor", "copilot").
    pub tool_id: String,
    /// The path where the tool configuration was found.
    pub config_path: String,
    /// Human-readable description of the detection.
    pub description: String,
}

/// Known tool configuration markers. Each entry is a (directory/file name, tool_id, description).
const TOOL_MARKERS: &[(&str, &str, &str)] = &[
    (".claude", "claude", "Claude Code"),
    (".cursor", "cursor", "Cursor"),
    (".github/copilot", "copilot", "GitHub Copilot"),
    (".vscode", "vscode", "VS Code"),
    (".cursorrules", "cursor", "Cursor Rules"),
    (".clinerules", "cline", "Cline Rules"),
    (".windsurfrules", "windsurf", "Windsurf Rules"),
    (".aider", "aider", "Aider"),
    (".continue", "continue", "Continue"),
];

/// Scans the project root for known AI tool configuration directories
/// and files. Returns a deduplicated list of detected tools.
pub fn detect_tools(project_root: &Path) -> Vec<DetectedTool> {
    let mut seen = HashSet::new();
    let mut detected = Vec::new();

    for &(marker, tool_id, description) in TOOL_MARKERS {
        if seen.contains(tool_id) {
            continue;
        }

        let marker_path = project_root.join(marker);
        if marker_path.exists() {
            seen.insert(tool_id);
            detected.push(DetectedTool {
                tool_id: tool_id.to_string(),
                config_path: marker.to_string(),
                description: description.to_string(),
            });
        }
    }

    detected
}

/// Returns a sorted, deduplicated list of detected tool ids.
pub fn detect_tool_ids(project_root: &Path) -> Vec<String> {
    let mut ids: Vec<String> = detect_tools(project_root)
        .into_iter()
        .map(|d| d.tool_id)
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// Returns `true` when at least one known tool marker exists under `project_root`.
pub fn has_any_tool_marker(project_root: &Path) -> bool {
    TOOL_MARKERS
        .iter()
        .any(|&(marker, _, _)| project_root.join(marker).exists())
}

/// Looks for a CLAUDE.md (or similar) marker file at the project root.
pub fn has_claude_md(project_root: &Path) -> bool {
    project_root.join("CLAUDE.md").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn detect_tools_finds_claude_dir() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".claude")).unwrap();

        let tools = detect_tools(tmp.path());
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].tool_id, "claude");
    }

    #[test]
    fn detect_tools_finds_multiple() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".claude")).unwrap();
        fs::create_dir(tmp.path().join(".cursor")).unwrap();

        let mut ids = detect_tool_ids(tmp.path());
        ids.sort();
        assert_eq!(ids, vec!["claude", "cursor"]);
    }

    #[test]
    fn detect_tools_empty_for_clean_dir() {
        let tmp = TempDir::new().unwrap();
        assert!(detect_tools(tmp.path()).is_empty());
        assert!(!has_any_tool_marker(tmp.path()));
    }

    #[test]
    fn has_claude_md_detects_file() {
        let tmp = TempDir::new().unwrap();
        assert!(!has_claude_md(tmp.path()));
        fs::write(tmp.path().join("CLAUDE.md"), "# Claude").unwrap();
        assert!(has_claude_md(tmp.path()));
    }
}
