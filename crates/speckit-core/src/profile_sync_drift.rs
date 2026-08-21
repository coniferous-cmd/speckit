//! Profile sync drift detection.
//!
//! Detects when a tool's installed artifacts don't match the desired
//! profile and delivery configuration.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::config::AI_TOOLS;
use crate::migration::{ALL_WORKFLOWS, workflow_to_skill_dir};

/// Maps workflow IDs to their skill directory names.
pub fn workflow_to_skill_dir_name(workflow: &str) -> Option<&'static str> {
    workflow_to_skill_dir(workflow)
}

/// Returns tools that are configured via either skills or commands.
pub fn get_configured_tools_for_profile_sync(project_path: &Path) -> Vec<String> {
    AI_TOOLS
        .iter()
        .filter(|tool| {
            if let Some(ref skills_dir) = tool.skills_dir {
                project_path.join(skills_dir).join("skills").exists()
            } else {
                false
            }
        })
        .map(|tool| tool.value.clone())
        .collect()
}

/// Detects if a single tool has profile/delivery drift against the desired state.
pub fn has_tool_profile_or_delivery_drift(
    project_path: &Path,
    tool_id: &str,
    desired_workflows: &[&str],
) -> bool {
    let tool = match AI_TOOLS.iter().find(|t| t.value == tool_id) {
        Some(t) => t,
        None => return false,
    };

    let skills_dir = match &tool.skills_dir {
        Some(d) => d,
        None => return false,
    };

    let skills_path = project_path.join(skills_dir).join("skills");
    let desired_set: HashSet<&str> = desired_workflows.iter().copied().collect();

    // Check for missing required artifacts
    for workflow in desired_workflows {
        if let Some(dir_name) = workflow_to_skill_dir(workflow) {
            let skill_file = skills_path.join(dir_name).join("SKILL.md");
            if !skill_file.exists() {
                return true;
            }
        }
    }

    // Check for artifacts that should not exist
    for workflow in ALL_WORKFLOWS {
        if desired_set.contains(workflow) {
            continue;
        }
        if let Some(dir_name) = workflow_to_skill_dir(workflow) {
            let skill_dir = skills_path.join(dir_name);
            if skill_dir.exists() {
                return true;
            }
        }
    }

    false
}

/// Returns configured tools that currently need a profile/delivery sync.
pub fn get_tools_needing_profile_sync(
    project_path: &Path,
    desired_workflows: &[&str],
    configured_tools: Option<&[String]>,
) -> Vec<String> {
    let tools = match configured_tools {
        Some(ids) => ids.to_vec(),
        None => get_configured_tools_for_profile_sync(project_path),
    };

    tools
        .iter()
        .filter(|tool_id| {
            has_tool_profile_or_delivery_drift(project_path, tool_id, desired_workflows)
        })
        .cloned()
        .collect()
}

/// Detects whether the current project has any profile/delivery drift.
pub fn has_project_config_drift(project_path: &Path, desired_workflows: &[&str]) -> bool {
    let configured_tools = get_configured_tools_for_profile_sync(project_path);
    let tools_needing_sync =
        get_tools_needing_profile_sync(project_path, desired_workflows, Some(&configured_tools));
    if !tools_needing_sync.is_empty() {
        return true;
    }

    let desired_set: HashSet<&str> = desired_workflows.iter().copied().collect();

    for tool_id in &configured_tools {
        let tool = match AI_TOOLS.iter().find(|t| t.value == *tool_id) {
            Some(t) => t,
            None => continue,
        };
        if let Some(ref skills_dir) = tool.skills_dir {
            let skills_path = project_path.join(skills_dir).join("skills");
            for workflow in ALL_WORKFLOWS {
                if desired_set.contains(workflow) {
                    continue;
                }
                if let Some(dir_name) = workflow_to_skill_dir(workflow) {
                    let skill_file = skills_path.join(dir_name).join("SKILL.md");
                    if skill_file.exists() {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Get installed workflows for a tool.
pub fn get_installed_workflows_for_tool(project_path: &Path, tool_id: &str) -> Vec<String> {
    let tool = match AI_TOOLS.iter().find(|t| t.value == tool_id) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut installed = HashSet::new();

    if let Some(ref skills_dir) = tool.skills_dir {
        let skills_path = project_path.join(skills_dir).join("skills");
        for workflow in ALL_WORKFLOWS {
            if let Some(dir_name) = workflow_to_skill_dir(workflow) {
                let skill_file = skills_path.join(dir_name).join("SKILL.md");
                if skill_file.exists() {
                    installed.insert(workflow.to_string());
                }
            }
        }
    }

    let mut result: Vec<String> = installed.into_iter().collect();
    result.sort();
    result
}
