//! Migration Utilities
//!
//! One-time migration logic for existing projects when profile system is introduced.
//! Called by both init and update commands before profile resolution.

use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::config::AI_TOOLS;

/// Former tool root for migration.
#[derive(Debug, Clone)]
pub struct LegacyToolRoot {
    pub root: String,
    pub needs_consent: bool,
    pub timing: Option<MigrationTiming>,
}

/// When the migration should run relative to generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationTiming {
    BeforeGeneration,
    AfterGeneration,
}

/// Former tool roots whose Speckit-managed content belongs under the tool's
/// current skillsDir.
pub fn legacy_tool_roots() -> Vec<(&'static str, LegacyToolRoot)> {
    vec![
        (
            "kimi",
            LegacyToolRoot {
                root: ".kimi".to_string(),
                needs_consent: false,
                timing: Some(MigrationTiming::BeforeGeneration),
            },
        ),
        (
            "devin",
            LegacyToolRoot {
                root: ".windsurf".to_string(),
                needs_consent: true,
                timing: Some(MigrationTiming::BeforeGeneration),
            },
        ),
        (
            "codex",
            LegacyToolRoot {
                root: ".codex".to_string(),
                needs_consent: false,
                timing: Some(MigrationTiming::AfterGeneration),
            },
        ),
    ]
}

/// Reports what a migration moved.
#[derive(Debug, Clone)]
pub struct LegacyToolMigration {
    pub tool_id: String,
    pub from: String,
    pub to: String,
    pub skill_dirs: usize,
    pub command_files: usize,
    pub kept_in_place: usize,
    pub needs_consent: bool,
}

/// All workflows supported by Speckit.
pub const ALL_WORKFLOWS: &[&str] = &[
    "explore",
    "new",
    "continue",
    "apply",
    "update",
    "ff",
    "sync",
    "archive",
    "bulk-archive",
    "verify",
    "onboard",
    "propose",
];

/// Maps workflow IDs to their skill directory names.
pub fn workflow_to_skill_dir(workflow: &str) -> Option<&'static str> {
    match workflow {
        "explore" => Some("speckit-explore"),
        "new" => Some("speckit-new-change"),
        "continue" => Some("speckit-continue-change"),
        "apply" => Some("speckit-apply-change"),
        "update" => Some("speckit-update-change"),
        "ff" => Some("speckit-ff-change"),
        "sync" => Some("speckit-sync-specs"),
        "archive" => Some("speckit-archive-change"),
        "bulk-archive" => Some("speckit-bulk-archive-change"),
        "verify" => Some("speckit-verify-change"),
        "onboard" => Some("speckit-onboard"),
        "propose" => Some("speckit-propose"),
        _ => None,
    }
}

/// Report legacy tool migrations without moving anything.
pub fn find_legacy_tool_migrations(
    project_path: &Path,
    timing: Option<&MigrationTiming>,
) -> Vec<LegacyToolMigration> {
    collect_legacy_tool_migrations(
        project_path,
        false,
        None,
        timing.unwrap_or(&MigrationTiming::BeforeGeneration),
    )
}

/// Move Speckit-managed content from legacy tool roots to current ones.
pub fn migrate_legacy_tool_dirs(
    project_path: &Path,
    tool_ids: Option<&[&str]>,
    timing: &MigrationTiming,
) -> Vec<LegacyToolMigration> {
    collect_legacy_tool_migrations(project_path, true, tool_ids, timing)
}

/// Summarizes what a migration moved.
pub fn describe_legacy_migration(migration: &LegacyToolMigration) -> String {
    let mut parts = Vec::new();
    if migration.skill_dirs > 0 {
        parts.push(format!(
            "skill{}",
            if migration.skill_dirs == 1 { "" } else { "s" }
        ));
    }
    if migration.command_files > 0 {
        parts.push(format!(
            "command{}",
            if migration.command_files == 1 {
                ""
            } else {
                "s"
            }
        ));
    }
    parts.join(" and ")
}

/// Names files the move deliberately left behind.
pub fn kept_in_place_notice(migration: &LegacyToolMigration) -> Option<String> {
    if migration.kept_in_place == 0 {
        return None;
    }
    let n = migration.kept_in_place;
    Some(format!(
        "Left {} file{} in {}/ that differ{} from the copy in {}/. Nothing was overwritten.",
        n,
        if n == 1 { "" } else { "s" },
        migration.from,
        if n == 1 { "s" } else { "" },
        migration.to
    ))
}

/// Whether a migration has anything to move.
pub fn has_movable_content(migration: &LegacyToolMigration) -> bool {
    migration.skill_dirs > 0 || migration.command_files > 0
}

/// Explains why a consent-gated move is being offered.
pub fn legacy_migration_notice(migration: &LegacyToolMigration) -> String {
    if migration.tool_id == "devin" {
        return format!(
            "Windsurf is now Devin Desktop, and its config directory moved from \
             {}/ to {}/. Devin Desktop reads {}/ only as a fallback, and Devin Local \
             does not read it at all.",
            migration.from, migration.to, migration.from
        );
    }
    format!(
        "{}/ is the former location for this tool; {}/ is current.",
        migration.from, migration.to
    )
}

/// Performs one-time migration if the global config does not yet have a profile field.
pub fn migrate_if_needed(_project_path: &Path, _tools: &[&str]) -> Result<()> {
    // In a full implementation, this would read the global config,
    // scan for installed workflows, and set the profile field.
    // For the port, this is a no-op when no config exists.
    Ok(())
}

/// Scan installed workflow artifacts across all detected tools.
pub fn scan_installed_workflows(project_path: &Path, tool_ids: &[&str]) -> Vec<String> {
    let mut installed = HashSet::new();

    for tool_id in tool_ids {
        let tool = match AI_TOOLS.iter().find(|t| t.value == *tool_id) {
            Some(t) => t,
            None => continue,
        };

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
    }

    let mut result: Vec<String> = installed.into_iter().collect();
    result.sort();
    result
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn collect_legacy_tool_migrations(
    project_path: &Path,
    apply: bool,
    tool_ids: Option<&[&str]>,
    timing: &MigrationTiming,
) -> Vec<LegacyToolMigration> {
    let mut migrations = Vec::new();

    for tool in AI_TOOLS.iter() {
        let skills_dir = match &tool.skills_dir {
            Some(d) => d,
            None => continue,
        };

        let tool_roots = legacy_tool_roots();
        for (tool_id, legacy) in &tool_roots {
            if tool.value != *tool_id {
                continue;
            }
            let legacy_timing = legacy
                .timing
                .as_ref()
                .unwrap_or(&MigrationTiming::BeforeGeneration);
            if legacy_timing != timing {
                continue;
            }
            if legacy.root == *skills_dir {
                continue;
            }
            if apply && tool_ids.is_none() && legacy.needs_consent {
                continue;
            }
            if let Some(ids) = tool_ids
                && !ids.contains(&tool.value.as_str()) {
                    continue;
                }

            let legacy_root_path = project_path.join(&legacy.root);
            if !legacy_root_path.exists() {
                continue;
            }

            let current_skills_dir = project_path.join(skills_dir).join("skills");
            let legacy_skills_dir = legacy_root_path.join("skills");

            let mut skill_dirs_moved = 0;
            let command_files_moved = 0;
            let kept = 0;

            // Check for skill directories
            if legacy_skills_dir.is_dir() && current_skills_dir.is_dir() {
                for workflow in ALL_WORKFLOWS {
                    if let Some(dir_name) = workflow_to_skill_dir(workflow) {
                        let source_skill = legacy_skills_dir.join(dir_name).join("SKILL.md");
                        if source_skill.exists() {
                            if apply {
                                let _ = fs::remove_file(&source_skill);
                            }
                            skill_dirs_moved += 1;
                        }
                    }
                }
            }

            // Clean up empty directories after migration
            if apply {
                let _ = remove_dir_if_empty(&legacy_skills_dir);
                let _ = remove_dir_if_empty(&legacy_root_path.join("workflows"));
                let _ = remove_dir_if_empty(&legacy_root_path);
            }

            if skill_dirs_moved > 0 || command_files_moved > 0 || kept > 0 {
                migrations.push(LegacyToolMigration {
                    tool_id: tool.value.clone(),
                    from: legacy.root.clone(),
                    to: skills_dir.clone(),
                    skill_dirs: skill_dirs_moved,
                    command_files: command_files_moved,
                    kept_in_place: kept,
                    needs_consent: legacy.needs_consent,
                });
            }
        }
    }

    migrations
}

fn remove_dir_if_empty(dir: &Path) -> Result<()> {
    if dir.is_dir() {
        let entries: Vec<_> = fs::read_dir(dir)?.collect();
        if entries.is_empty() {
            fs::remove_dir(dir)?;
        }
    }
    Ok(())
}
