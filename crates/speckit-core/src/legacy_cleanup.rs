//! Legacy cleanup module for detecting and removing Speckit artifacts
//! from previous init versions during the migration to the skill-based workflow.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{AI_TOOLS, OPENSPEC_MARKERS};

/// Legacy config file names from the old ToolRegistry.
pub const LEGACY_CONFIG_FILES: &[&str] = &[
    "CLAUDE.md",
    "CLINE.md",
    "CODEBUDDY.md",
    "COSTRICT.md",
    "QODER.md",
    "IFLOW.md",
    "AGENTS.md",
    "QWEN.md",
];

/// Pattern type for legacy slash commands.
#[derive(Debug, Clone)]
pub enum LegacySlashCommandPatternType {
    Directory,
    Files,
}

/// Pattern for legacy slash commands.
#[derive(Debug, Clone)]
pub struct LegacySlashCommandPattern {
    pub pattern_type: LegacySlashCommandPatternType,
    pub path: Option<String>,
    pub pattern: Option<Vec<String>>,
}

/// Describes a managed global prompt home.
#[derive(Debug, Clone)]
pub struct LegacyGlobalPromptPattern {
    pub managed_file_names: Vec<String>,
    pub resolve_prompt_dir: fn() -> PathBuf,
    pub replacement_label: Option<String>,
}

/// Workflow-aware metadata for a detected global legacy prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyGlobalPromptMatch {
    pub path: String,
    pub tool_id: String,
    pub managed_file_name: String,
    pub workflow_ids: Vec<String>,
    pub replacement_label: Option<String>,
}

/// Result of legacy artifact detection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LegacyDetectionResult {
    pub config_files: Vec<String>,
    pub config_files_to_update: Vec<String>,
    pub slash_command_dirs: Vec<String>,
    pub slash_command_files: Vec<String>,
    pub global_slash_command_files: Vec<String>,
    pub global_slash_command_details: Vec<LegacyGlobalPromptMatch>,
    pub has_speckit_agents: bool,
    pub has_project_md: bool,
    pub has_root_agents_with_markers: bool,
    pub has_legacy_artifacts: bool,
}

/// Result of cleanup operation.
#[derive(Debug, Clone, Default)]
pub struct CleanupResult {
    pub deleted_files: Vec<String>,
    pub deleted_file_replacement_labels: std::collections::HashMap<String, String>,
    pub modified_files: Vec<String>,
    pub deleted_dirs: Vec<String>,
    pub project_md_needs_migration: bool,
    pub errors: Vec<String>,
}

/// Resolve the Codex global prompts directory.
pub fn get_codex_prompt_dir() -> PathBuf {
    if let Ok(env_home) = std::env::var("CODEX_HOME") {
        if !env_home.trim().is_empty() {
            return PathBuf::from(env_home.trim()).join("prompts");
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
        .join("prompts")
}

/// Checks if content contains Speckit markers.
pub fn has_speckit_markers(content: &str) -> bool {
    content.contains(OPENSPEC_MARKERS.start) && content.contains(OPENSPEC_MARKERS.end)
}

/// Checks if file content is 100% Speckit content.
pub fn is_only_speckit_content(content: &str) -> bool {
    let start_idx = content.find(OPENSPEC_MARKERS.start);
    let end_idx = content.find(OPENSPEC_MARKERS.end);

    match (start_idx, end_idx) {
        (Some(s), Some(e)) if e > s => {
            let before = &content[..s];
            let after = &content[e + OPENSPEC_MARKERS.end.len()..];
            before.trim().is_empty() && after.trim().is_empty()
        }
        _ => false,
    }
}

/// Removes the Speckit marker block from file content.
pub fn remove_marker_block(content: &str) -> String {
    let start_idx = match content.find(OPENSPEC_MARKERS.start) {
        Some(idx) => idx,
        None => return content.to_string(),
    };
    let end_marker_end = match content.find(OPENSPEC_MARKERS.end) {
        Some(idx) => idx + OPENSPEC_MARKERS.end.len(),
        None => return content.to_string(),
    };

    let before = &content[..start_idx];
    let after = &content[end_marker_end..];

    // Clean up double blank lines
    let result = format!("{}{}", before.trim_end(), after.trim_start());
    let mut prev_blank = false;
    let mut cleaned = String::new();
    for line in result.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank && prev_blank {
            continue;
        }
        if !cleaned.is_empty() {
            cleaned.push('\n');
        }
        cleaned.push_str(line);
        prev_blank = is_blank;
    }
    cleaned
}

/// Detects all legacy Speckit artifacts in a project.
pub fn detect_legacy_artifacts(project_path: &Path) -> Result<LegacyDetectionResult> {
    let mut result = LegacyDetectionResult::default();

    // Detect legacy config files
    for file_name in LEGACY_CONFIG_FILES {
        let file_path = project_path.join(file_name);
        if file_path.exists() {
            if let Ok(content) = fs::read_to_string(&file_path) {
                if has_speckit_markers(&content) {
                    result.config_files.push(file_name.to_string());
                    result.config_files_to_update.push(file_name.to_string());
                }
            }
        }
    }

    // Detect legacy slash command directories
    let legacy_command_dirs = [
        (".claude/commands/speckit", "claude"),
        (".codebuddy/commands/speckit", "codebuddy"),
        (".qoder/commands/speckit", "qoder"),
        (".lingma/commands/speckit", "lingma"),
        (".crush/commands/speckit", "crush"),
        (".gemini/commands/speckit", "gemini"),
    ];

    for (dir_path, _tool_id) in &legacy_command_dirs {
        if project_path.join(dir_path).is_dir() {
            result.slash_command_dirs.push(dir_path.to_string());
        }
    }

    // Detect legacy slash command files
    let legacy_command_files: Vec<(&str, &str)> = vec![
        (".cursor/commands", "speckit-*.md"),
        (".windsurf/workflows", "speckit-*.md"),
        (".kilocode/workflows", "speckit-*.md"),
        (".kiro/prompts", "speckit-*.prompt.md"),
        (".github/prompts", "speckit-*.prompt.md"),
        (".amazonq/prompts", "speckit-*.md"),
        (".clinerules/workflows", "speckit-*.md"),
        (".roo/commands", "speckit-*.md"),
    ];

    for (dir, pattern) in &legacy_command_files {
        let dir_path = project_path.join(dir);
        if !dir_path.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if pattern.contains('*') {
                    let file_prefix = pattern.replace('*', "");
                    if name.starts_with(&file_prefix) && name.ends_with(".md") {
                        result.slash_command_files.push(format!("{}/{}", dir, name));
                    }
                }
            }
        }
    }

    // Detect legacy global prompts
    let codex_prompts_dir = get_codex_prompt_dir();
    let managed_codex_files = [
        "opsx-propose.md",
        "opsx-explore.md",
        "opsx-new.md",
        "opsx-continue.md",
        "opsx-apply.md",
        "opsx-update.md",
        "opsx-ff.md",
        "opsx-sync.md",
        "opsx-archive.md",
        "opsx-bulk-archive.md",
        "opsx-verify.md",
        "opsx-onboard.md",
    ];

    if codex_prompts_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&codex_prompts_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if managed_codex_files.contains(&name.as_str()) {
                    let full_path = entry.path().to_string_lossy().to_string();
                    result.global_slash_command_files.push(full_path.clone());
                    result
                        .global_slash_command_details
                        .push(LegacyGlobalPromptMatch {
                            path: full_path,
                            tool_id: "codex".to_string(),
                            managed_file_name: name,
                            workflow_ids: Vec::new(),
                            replacement_label: Some("Codex skills".to_string()),
                        });
                }
            }
        }
    }

    // Detect legacy structure files
    result.has_speckit_agents = project_path.join("speckit/AGENTS.md").exists();
    result.has_project_md = project_path.join("speckit/project.md").exists();

    let root_agents = project_path.join("AGENTS.md");
    if root_agents.exists() {
        if let Ok(content) = fs::read_to_string(&root_agents) {
            result.has_root_agents_with_markers = has_speckit_markers(&content);
        }
    }

    result.has_legacy_artifacts = !result.config_files.is_empty()
        || !result.slash_command_dirs.is_empty()
        || !result.slash_command_files.is_empty()
        || !result.global_slash_command_files.is_empty()
        || result.has_speckit_agents
        || result.has_root_agents_with_markers
        || result.has_project_md;

    Ok(result)
}

/// Cleans up legacy Speckit artifacts from a project.
pub fn cleanup_legacy_artifacts(
    project_path: &Path,
    detection: &LegacyDetectionResult,
) -> Result<CleanupResult> {
    let mut result = CleanupResult {
        project_md_needs_migration: detection.has_project_md,
        ..Default::default()
    };

    // Remove marker blocks from config files (NEVER delete config files)
    for file_name in &detection.config_files_to_update {
        let file_path = project_path.join(file_name);
        if let Ok(content) = fs::read_to_string(&file_path) {
            let new_content = remove_marker_block(&content);
            match fs::write(&file_path, &new_content) {
                Ok(_) => result.modified_files.push(file_name.clone()),
                Err(e) => result
                    .errors
                    .push(format!("Failed to modify {}: {}", file_name, e)),
            }
        }
    }

    // Delete legacy slash command directories
    for dir_path in &detection.slash_command_dirs {
        let full_path = project_path.join(dir_path);
        match fs::remove_dir_all(&full_path) {
            Ok(_) => result.deleted_dirs.push(dir_path.clone()),
            Err(e) => result
                .errors
                .push(format!("Failed to delete directory {}: {}", dir_path, e)),
        }
    }

    // Delete legacy slash command files
    for file_path in &detection.slash_command_files {
        let full_path = project_path.join(file_path);
        match fs::remove_file(&full_path) {
            Ok(_) => result.deleted_files.push(file_path.clone()),
            Err(e) => result
                .errors
                .push(format!("Failed to delete {}: {}", file_path, e)),
        }
    }

    // Delete managed global slash command files
    for prompt in &detection.global_slash_command_details {
        let full_path = PathBuf::from(&prompt.path);
        match fs::remove_file(&full_path) {
            Ok(_) => {
                result.deleted_files.push(prompt.path.clone());
                if let Some(ref label) = prompt.replacement_label {
                    result
                        .deleted_file_replacement_labels
                        .insert(prompt.path.clone(), label.clone());
                }
            }
            Err(e) => result
                .errors
                .push(format!("Failed to delete {}: {}", prompt.path, e)),
        }
    }

    // Delete speckit/AGENTS.md
    if detection.has_speckit_agents {
        let agents_path = project_path.join("speckit/AGENTS.md");
        if agents_path.exists() {
            match fs::remove_file(&agents_path) {
                Ok(_) => result.deleted_files.push("speckit/AGENTS.md".to_string()),
                Err(e) => result
                    .errors
                    .push(format!("Failed to delete speckit/AGENTS.md: {}", e)),
            }
        }
    }

    Ok(result)
}

/// Generates a cleanup summary message.
pub fn format_cleanup_summary(result: &CleanupResult) -> String {
    let mut lines = Vec::new();

    if !result.deleted_files.is_empty()
        || !result.deleted_dirs.is_empty()
        || !result.modified_files.is_empty()
    {
        lines.push("Cleaned up legacy files:".to_string());

        for file in &result.deleted_files {
            let replacement = result
                .deleted_file_replacement_labels
                .get(file)
                .map(|label| format!(" (replaced by {})", label))
                .unwrap_or_default();
            lines.push(format!("  Removed {}{}", file, replacement));
        }

        for dir in &result.deleted_dirs {
            lines.push(format!(
                "  Removed {}/ (replaced by Speckit skills and commands)",
                dir
            ));
        }

        for file in &result.modified_files {
            lines.push(format!("  Removed Speckit markers from {}", file));
        }
    }

    if result.project_md_needs_migration {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(format_project_md_migration_hint());
    }

    if !result.errors.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push("Errors during cleanup:".to_string());
        for error in &result.errors {
            lines.push(format!("  {}", error));
        }
    }

    lines.join("\n")
}

/// Generates a detection summary message.
pub fn format_detection_summary(detection: &LegacyDetectionResult) -> String {
    let mut lines = Vec::new();

    if !detection.config_files.is_empty()
        || !detection.slash_command_dirs.is_empty()
        || !detection.slash_command_files.is_empty()
        || detection.has_speckit_agents
        || detection.has_root_agents_with_markers
        || detection.has_project_md
    {
        lines.push("Upgrading to the new Speckit".to_string());
        lines.push(String::new());

        if !detection.slash_command_dirs.is_empty()
            || !detection.slash_command_files.is_empty()
            || detection.has_speckit_agents
        {
            lines.push("Files to remove".to_string());
            for dir in &detection.slash_command_dirs {
                lines.push(format!("  {}/", dir));
            }
            for file in &detection.slash_command_files {
                lines.push(format!("  {}", file));
            }
            if detection.has_speckit_agents {
                lines.push("  speckit/AGENTS.md".to_string());
            }
        }

        if !detection.config_files.is_empty() {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push("Files to update".to_string());
            for file in &detection.config_files {
                lines.push(format!("  {}", file));
            }
        }
    }

    lines.join("\n")
}

/// Generates a summary for deferred global prompt files.
pub fn format_deferred_global_prompt_summary(detection: &LegacyDetectionResult) -> String {
    let deferred = get_legacy_global_prompt_matches(detection);
    if deferred.is_empty() {
        return String::new();
    }

    let mut lines = vec![
        "Deferred global prompts cleanup".to_string(),
        "These global prompts will only be removed after matching replacement skills are installed.".to_string(),
    ];
    for prompt in &deferred {
        let tool_label = if prompt.tool_id.is_empty() {
            String::new()
        } else {
            format!("{}: ", prompt.tool_id)
        };
        lines.push(format!("  {}{}", tool_label, prompt.path));
    }

    lines.join("\n")
}

/// Extract tool IDs from detected legacy artifacts.
pub fn get_tools_from_legacy_artifacts(detection: &LegacyDetectionResult) -> Vec<String> {
    let mut tools = HashSet::new();

    for prompt in get_legacy_global_prompt_matches(detection) {
        tools.insert(prompt.tool_id);
    }

    // Map known legacy dirs back to tool ids
    let dir_to_tool: std::collections::HashMap<&str, &str> = [
        (".claude/commands/speckit", "claude"),
        (".codebuddy/commands/speckit", "codebuddy"),
        (".qoder/commands/speckit", "qoder"),
        (".lingma/commands/speckit", "lingma"),
        (".crush/commands/speckit", "crush"),
        (".gemini/commands/speckit", "gemini"),
    ]
    .iter()
    .cloned()
    .collect();

    for dir in &detection.slash_command_dirs {
        if let Some(tool_id) = dir_to_tool.get(dir.as_str()) {
            tools.insert(tool_id.to_string());
        }
    }

    tools.into_iter().collect()
}

/// Normalize global Codex prompt matches.
pub fn get_legacy_global_prompt_matches(
    detection: &LegacyDetectionResult,
) -> Vec<LegacyGlobalPromptMatch> {
    if !detection.global_slash_command_details.is_empty() {
        return detection.global_slash_command_details.clone();
    }

    detection
        .global_slash_command_files
        .iter()
        .filter_map(|file_path| get_managed_global_legacy_prompt_metadata(file_path))
        .collect()
}

/// Collects workflow IDs inferred from detected legacy global prompts for a specific tool.
pub fn get_legacy_workflow_ids_for_tool(
    detection: &LegacyDetectionResult,
    tool_id: &str,
) -> Vec<String> {
    let mut workflows = HashSet::new();
    for prompt in get_legacy_global_prompt_matches(detection) {
        if prompt.tool_id == tool_id {
            for workflow_id in &prompt.workflow_ids {
                workflows.insert(workflow_id.clone());
            }
        }
    }
    workflows.into_iter().collect()
}

/// Returns a detection snapshot with global prompt cleanup removed.
pub fn omit_global_legacy_prompt_files(detection: &LegacyDetectionResult) -> LegacyDetectionResult {
    let mut next = detection.clone();
    next.global_slash_command_files.clear();
    next.global_slash_command_details.clear();
    recalculate_has_legacy(&mut next);
    next
}

/// Returns a detection snapshot with specific tool artifacts omitted.
pub fn omit_tool_legacy_artifacts(
    detection: &LegacyDetectionResult,
    tool_ids: &[String],
) -> LegacyDetectionResult {
    if tool_ids.is_empty() {
        return detection.clone();
    }
    let skip: HashSet<&str> = tool_ids.iter().map(|s| s.as_str()).collect();
    let dir_to_tool: std::collections::HashMap<&str, &str> = [
        (".claude/commands/speckit", "claude"),
        (".codebuddy/commands/speckit", "codebuddy"),
        (".qoder/commands/speckit", "qoder"),
        (".lingma/commands/speckit", "lingma"),
        (".crush/commands/speckit", "crush"),
        (".gemini/commands/speckit", "gemini"),
    ]
    .iter()
    .cloned()
    .collect();

    let mut next = detection.clone();
    next.slash_command_dirs.retain(|dir| {
        !dir_to_tool
            .get(dir.as_str())
            .map_or(false, |t| skip.contains(t))
    });
    recalculate_has_legacy(&mut next);
    next
}

/// Builds a detection snapshot containing only selected global prompt matches.
pub fn pick_global_legacy_prompt_files(
    detection: &LegacyDetectionResult,
    file_paths: &[&str],
) -> LegacyDetectionResult {
    let selected: HashSet<PathBuf> = file_paths
        .iter()
        .map(|p| {
            PathBuf::from(p)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(p))
        })
        .collect();
    let details: Vec<LegacyGlobalPromptMatch> = get_legacy_global_prompt_matches(detection)
        .into_iter()
        .filter(|d| {
            selected.contains(
                &PathBuf::from(&d.path)
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(&d.path)),
            )
        })
        .collect();

    LegacyDetectionResult {
        config_files: Vec::new(),
        config_files_to_update: Vec::new(),
        slash_command_dirs: Vec::new(),
        slash_command_files: Vec::new(),
        global_slash_command_files: details.iter().map(|d| d.path.clone()).collect(),
        global_slash_command_details: details.clone(),
        has_speckit_agents: false,
        has_project_md: false,
        has_root_agents_with_markers: false,
        has_legacy_artifacts: !details.is_empty(),
    }
}

/// Generates a migration hint message for project.md.
pub fn format_project_md_migration_hint() -> String {
    let lines = vec![
        "Needs your attention".to_string(),
        "  speckit/project.md".to_string(),
        "    We won't delete this file. It may contain useful project context.".to_string(),
        String::new(),
        "    The new speckit/config.yaml has a 'context:' section for planning".to_string(),
        "    context. This is included in every Speckit request and works more".to_string(),
        "    reliably than the old project.md approach.".to_string(),
        String::new(),
        "    Review project.md, move any useful content to config.yaml's context".to_string(),
        "    section, then delete the file when ready.".to_string(),
    ];
    lines.join("\n")
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn get_managed_global_legacy_prompt_metadata(file_path: &str) -> Option<LegacyGlobalPromptMatch> {
    let path = PathBuf::from(file_path);
    if !path.is_absolute() {
        return None;
    }

    let codex_prompts_dir = get_codex_prompt_dir();
    if path.parent() != Some(&codex_prompts_dir) {
        return None;
    }

    let managed_codex_files = [
        "opsx-propose.md",
        "opsx-explore.md",
        "opsx-new.md",
        "opsx-continue.md",
        "opsx-apply.md",
        "opsx-update.md",
        "opsx-ff.md",
        "opsx-sync.md",
        "opsx-archive.md",
        "opsx-bulk-archive.md",
        "opsx-verify.md",
        "opsx-onboard.md",
    ];

    let file_name = path.file_name()?.to_str()?;
    if managed_codex_files.contains(&file_name) {
        Some(LegacyGlobalPromptMatch {
            path: file_path.to_string(),
            tool_id: "codex".to_string(),
            managed_file_name: file_name.to_string(),
            workflow_ids: Vec::new(),
            replacement_label: Some("Codex skills".to_string()),
        })
    } else {
        None
    }
}

fn recalculate_has_legacy(result: &mut LegacyDetectionResult) {
    result.has_legacy_artifacts = !result.config_files.is_empty()
        || !result.slash_command_dirs.is_empty()
        || !result.slash_command_files.is_empty()
        || !result.global_slash_command_files.is_empty()
        || result.has_speckit_agents
        || result.has_root_agents_with_markers
        || result.has_project_md;
}
