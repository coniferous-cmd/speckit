//! Shared Skill Target Resolution
//!
//! Manages ownership markers for shared skill directories. When multiple AI
//! tools share the same physical skills root (e.g., Codex and agents both
//! use `.agents/skills`), this module tracks which tool owns the root and
//! prevents clobbering.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::config::{AI_TOOLS, AiToolOption, SPECKIT_SKILL_NAMES};

/// The ownership-marker filename.
const TARGET_MARKER: &str = ".speckit-target";

/// Returns the ownership-marker path for one shared skills root.
fn marker_path(project_path: &Path, skills_dir: &str) -> std::path::PathBuf {
    project_path
        .join(skills_dir)
        .join("skills")
        .join(TARGET_MARKER)
}

/// Reads a valid-looking marker value.
pub fn read_shared_skill_target(project_path: &Path, skills_dir: &str) -> Option<String> {
    let target = marker_path(project_path, skills_dir);
    let content = fs::read_to_string(&target).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Whether a tool still has an allowlisted managed skill under an old root.
fn has_legacy_skills(project_path: &Path, tool: &AiToolOption) -> bool {
    tool.legacy_skills_dirs
        .as_ref()
        .map(|dirs| {
            dirs.iter().any(|root| {
                let skills_dir = project_path.join(root).join("skills");
                SPECKIT_SKILL_NAMES
                    .iter()
                    .any(|skill_name| skills_dir.join(skill_name).join("SKILL.md").exists())
            })
        })
        .unwrap_or(false)
}

/// Infers pre-marker ownership from generated invocation syntax.
fn infer_shared_skill_target(project_path: &Path, skills_dir: &str) -> Option<String> {
    let mut found_generic_reference = false;

    for skill_name in SPECKIT_SKILL_NAMES {
        let skill_file = project_path
            .join(skills_dir)
            .join("skills")
            .join(skill_name)
            .join("SKILL.md");
        if let Ok(content) = fs::read_to_string(&skill_file) {
            if content.contains("$speckit-") {
                return Some("codex".to_string());
            }
            if content.contains("/speckit-") {
                found_generic_reference = true;
            }
        }
    }

    if found_generic_reference {
        Some("agents".to_string())
    } else {
        None
    }
}

/// Whether the canonical shared root already contains an Speckit skill.
fn has_current_skills(project_path: &Path, skills_dir: &str) -> bool {
    SPECKIT_SKILL_NAMES.iter().any(|skill_name| {
        project_path
            .join(skills_dir)
            .join("skills")
            .join(skill_name)
            .join("SKILL.md")
            .exists()
    })
}

/// A shared skill root can only hold one rendered variant of each skill.
/// Keep the writer recorded so later updates do not infer every tool that
/// happens to use the same directory.
pub fn reconcile_shared_skill_targets(
    project_path: &Path,
    tools: &[AiToolOption],
) -> Vec<AiToolOption> {
    // Group tools by their skills_dir
    let mut by_root: HashMap<String, Vec<&AiToolOption>> = HashMap::new();
    for tool in tools {
        if let Some(ref skills_dir) = tool.skills_dir {
            by_root.entry(skills_dir.clone()).or_default().push(tool);
        }
    }

    let mut reconciled = Vec::new();

    for group in by_root.values() {
        if group.len() == 1 {
            reconciled.push((*group[0]).clone());
            continue;
        }

        let root = group[0].skills_dir.as_ref().unwrap();

        // Check for explicit marker
        if let Some(marked) = read_shared_skill_target(project_path, root)
            && let Some(marked_tool) = group.iter().find(|t| t.value == marked)
        {
            reconciled.push((*marked_tool).clone());
            continue;
        }

        // Infer from content
        let inferred = infer_shared_skill_target(project_path, root);
        let legacy_codex = group
            .iter()
            .find(|t| t.value == "codex" && has_legacy_skills(project_path, t));

        if inferred.as_deref() == Some("agents")
            && let Some(codex) = legacy_codex
        {
            reconciled.push((*codex).clone());
            continue;
        }

        if let Some(ref inferred_val) = inferred
            && let Some(inferred_tool) = group.iter().find(|t| &t.value == inferred_val)
        {
            reconciled.push((*inferred_tool).clone());
            continue;
        }

        // Existing skills -> prefer agents
        if has_current_skills(project_path, root) {
            let fallback = group
                .iter()
                .find(|t| t.value == "agents")
                .or_else(|| group.first())
                .unwrap();
            reconciled.push((*fallback).clone());
            continue;
        }

        // Legacy skills
        if let Some(legacy) = group.iter().find(|t| has_legacy_skills(project_path, t)) {
            reconciled.push((*legacy).clone());
            continue;
        }

        // Default: prefer agents
        let fallback = group
            .iter()
            .find(|t| t.value == "agents")
            .or_else(|| group.first())
            .unwrap();
        reconciled.push((*fallback).clone());
    }

    reconciled
}

/// Returns whether a tool is the active writer for its physical skills root.
/// Non-shared roots are always active.
pub fn is_shared_skill_target_active(project_path: &Path, tool_id: &str) -> bool {
    let tool = match AI_TOOLS.iter().find(|t| t.value == tool_id) {
        Some(t) => t,
        None => return false,
    };
    let skills_dir = match &tool.skills_dir {
        Some(d) => d,
        None => return false,
    };
    let sharing_root: Vec<&AiToolOption> = AI_TOOLS
        .iter()
        .filter(|t| t.skills_dir.as_deref() == Some(skills_dir.as_str()))
        .collect();
    if sharing_root.len() < 2 {
        return true;
    }
    let reconciled = reconcile_shared_skill_targets(
        project_path,
        &sharing_root.into_iter().cloned().collect::<Vec<_>>(),
    );
    reconciled.iter().any(|t| t.value == tool_id)
}

/// The tool that already owns `tool_id`'s shared skills root, when a
/// DIFFERENT one does. Returns the owner's tool id only when the root
/// already carries an ownership signal.
pub fn shared_skill_root_owner(project_path: &Path, tool_id: &str) -> Option<String> {
    let tool = AI_TOOLS.iter().find(|t| t.value == tool_id)?;
    let skills_dir = tool.skills_dir.as_ref()?;
    let sharing_root: Vec<&AiToolOption> = AI_TOOLS
        .iter()
        .filter(|t| t.skills_dir.as_deref() == Some(skills_dir.as_str()))
        .collect();
    if sharing_root.len() < 2 {
        return None;
    }

    let has_owner_signal = read_shared_skill_target(project_path, skills_dir).is_some()
        || has_current_skills(project_path, skills_dir);
    if !has_owner_signal {
        return None;
    }

    let reconciled = reconcile_shared_skill_targets(
        project_path,
        &sharing_root.into_iter().cloned().collect::<Vec<_>>(),
    );
    let owner = reconciled.first()?.value.clone();
    if owner != tool_id { Some(owner) } else { None }
}

/// Whether generating `tool_id` into its shared skills root would clobber
/// a tree a DIFFERENT tool already owns.
pub fn shared_skill_root_owned_by_other(project_path: &Path, tool_id: &str) -> bool {
    shared_skill_root_owner(project_path, tool_id).is_some()
}

/// Writes the ownership marker for a shared skill root.
pub fn write_shared_skill_target(project_path: &Path, tool_id: &str) {
    let tool = match AI_TOOLS.iter().find(|t| t.value == tool_id) {
        Some(t) => t,
        None => return,
    };
    let skills_dir = match &tool.skills_dir {
        Some(d) => d,
        None => return,
    };
    let sharing_root: Vec<&AiToolOption> = AI_TOOLS
        .iter()
        .filter(|t| t.skills_dir.as_deref() == Some(skills_dir.as_str()))
        .collect();
    if sharing_root.len() < 2 {
        return;
    }

    let target = marker_path(project_path, skills_dir);
    if let Some(parent) = target.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&target, format!("{}\n", tool_id));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_path_construction() {
        let project = Path::new("/tmp/proj");
        let result = marker_path(project, ".claude");
        assert_eq!(
            result,
            std::path::PathBuf::from("/tmp/proj/.claude/skills/.speckit-target")
        );
    }

    #[test]
    fn read_shared_skill_target_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read_shared_skill_target(tmp.path(), ".claude").is_none());
    }

    #[test]
    fn write_and_read_shared_skill_target() {
        let tmp = tempfile::tempdir().unwrap();
        let skills_dir = tmp.path().join(".agents").join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        write_shared_skill_target(tmp.path(), "agents");
        let result = read_shared_skill_target(tmp.path(), ".agents");
        assert_eq!(result.as_deref(), Some("agents"));
    }

    #[test]
    fn reconcile_single_tool_returns_it() {
        let tmp = tempfile::tempdir().unwrap();
        let tools = vec![AiToolOption {
            name: "Claude".into(),
            value: "claude".into(),
            available: true,
            success_label: None,
            skills_dir: Some(".claude".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        }];
        let result = reconcile_shared_skill_targets(tmp.path(), &tools);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "claude");
    }
}
