//! Available Tools Detection
//!
//! Detects which AI tools are available in a project by scanning for their
//! configuration directories.

use std::fs;
use std::path::Path;

use crate::config::{AI_TOOLS, AiToolOption, OPENSPEC_SKILL_NAMES};
use crate::shared_skill_target::reconcile_shared_skill_targets;

/// Returns `true` if the tool supports skills (has a skills directory
/// configured, either project-local or global).
fn tool_supports_skills(tool: &AiToolOption) -> bool {
    tool.skills_dir.is_some() || tool.global_skills_dir.is_some()
}

/// Resolves the full path to a tool's skills directory.
fn resolve_tool_skills_dir(project_path: &Path, tool: &AiToolOption) -> std::path::PathBuf {
    if let Some(ref global_dir) = tool.global_skills_dir {
        // Global skills: resolve from user's home directory
        if let Some(home) = dirs::home_dir() {
            return home.join(global_dir).join("skills");
        }
    }
    if let Some(ref skills_dir) = tool.skills_dir {
        return project_path.join(skills_dir).join("skills");
    }
    project_path.join(".unknown").join("skills")
}

/// Scans the project path for AI tool configuration directories and returns
/// the tools that are present.
///
/// For tools with `detection_paths`, checks those specific paths (files or
/// directories). Otherwise checks the project's `skillsDir`, or managed skill
/// files in the user's home directory for a global skill target.
pub fn get_available_tools(project_path: &Path) -> Vec<AiToolOption> {
    let available: Vec<AiToolOption> = AI_TOOLS
        .iter()
        .filter(|tool| {
            if !tool_supports_skills(tool) {
                return false;
            }

            // Global skills dir: check if any skill files exist
            if tool.global_skills_dir.is_some() {
                let skills_dir = resolve_tool_skills_dir(project_path, tool);
                return OPENSPEC_SKILL_NAMES
                    .iter()
                    .any(|skill_name| skills_dir.join(skill_name).join("SKILL.md").exists());
            }

            let skills_dir = match &tool.skills_dir {
                Some(d) => d,
                None => return false,
            };

            // Detection paths: check if any exist
            if let Some(ref detection_paths) = tool.detection_paths {
                return detection_paths.iter().any(|p| {
                    let full_path = project_path.join(p);
                    fs::metadata(&full_path).is_ok()
                });
            }

            // Default: check if the skills directory exists
            let dir_path = project_path.join(skills_dir);
            fs::metadata(&dir_path).map(|m| m.is_dir()).unwrap_or(false)
        })
        .cloned()
        .collect();

    // Reconcile shared skill targets
    let project_tools: Vec<AiToolOption> = available
        .iter()
        .filter(|tool| tool.skills_dir.is_some())
        .cloned()
        .collect();
    let active_project_tools: std::collections::HashSet<String> =
        reconcile_shared_skill_targets(project_path, &project_tools)
            .into_iter()
            .map(|t| t.value)
            .collect();

    available
        .into_iter()
        .filter(|tool| {
            tool.global_skills_dir.is_some() || active_project_tools.contains(&tool.value)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_supports_skills_with_skills_dir() {
        let tool = AI_TOOLS.iter().find(|t| t.value == "claude").unwrap();
        assert!(tool_supports_skills(tool));
    }

    #[test]
    fn tool_supports_skills_with_global_dir() {
        let tool = AI_TOOLS.iter().find(|t| t.value == "minimax-code").unwrap();
        assert!(tool_supports_skills(tool));
    }

    #[test]
    fn get_available_tools_returns_empty_for_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let tools = get_available_tools(tmp.path());
        // No tool config dirs exist in an empty temp directory
        assert!(tools.is_empty());
    }

    #[test]
    fn get_available_tools_detects_claude_dir() {
        let tmp = tempfile::tempdir().unwrap();
        // Create .claude directory to simulate Claude Code presence
        fs::create_dir_all(tmp.path().join(".claude")).unwrap();
        let tools = get_available_tools(tmp.path());
        assert!(tools.iter().any(|t| t.value == "claude"));
    }
}
