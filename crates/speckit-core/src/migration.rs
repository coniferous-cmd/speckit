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
    "implement",
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
        "implement" => Some("speckit-implement-change"),
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
pub fn migrate_if_needed(project_path: &Path, tools: &[&str]) -> Result<()> {
    let config_path = crate::global_config::get_global_config_path();
    let raw = match std::fs::read_to_string(&config_path) {
        Ok(content) => serde_json::from_str::<serde_json::Value>(&content).map_err(|error| {
            anyhow::anyhow!("Invalid JSON in {}: {error}", config_path.display())
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };

    // An explicit profile always wins. This keeps migration one-shot and
    // avoids changing a user's later configuration choices.
    if raw.get("profile").is_some() {
        return Ok(());
    }

    let installed = scan_installed_workflows(project_path, tools);
    if installed.is_empty() {
        return Ok(());
    }

    let mut config = crate::global_config::get_global_config();
    config.profile = crate::global_config::Profile::Custom;
    config.workflows = Some(installed.clone());
    crate::global_config::save_global_config(&config)?;
    println!(
        "Migrated: custom profile with {} installed workflows",
        installed.len()
    );
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
                && !ids.contains(&tool.value.as_str())
            {
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

// ── Workflow rename migration ─────────────────────────────────────────────────

/// Records one workflow rename applied to the global config.
///
/// Returned from [`migrate_workflow_renames`] so callers can describe what
/// changed (e.g. for logging).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRename {
    pub old: &'static str,
    pub new: &'static str,
}

/// Legacy workflow IDs and their current equivalents. Extend this table when a
/// workflow is renamed so future invocations of [`migrate_workflow_renames`]
/// (and tests using [`migrate_workflow_renames_at`]) handle the rename too.
const WORKFLOW_RENAMES: &[(&str, &str)] = &[("apply", "implement")];

/// Migrate legacy workflow IDs in the global config's `workflows` array.
///
/// Reads `~/.config/speckit/config.json` (or its platform equivalent), renames
/// any workflow whose ID appears in [`WORKFLOW_RENAMES`], dedupes while
/// preserving first-occurrence order, and writes the result back. Idempotent:
/// running on an already-migrated config is a no-op and does not rewrite the
/// file.
///
/// Returns the renames that were actually applied. Empty when:
/// - the file does not exist (no global config written yet),
/// - the file is unreadable or malformed JSON (a warning is logged),
/// - the file has no `workflows` array, or
/// - no rename matched.
pub fn migrate_workflow_renames() -> Vec<WorkflowRename> {
    migrate_workflow_renames_at(&crate::global_config::get_global_config_path())
}

/// Test seam: same as [`migrate_workflow_renames`] but reads and writes the
/// supplied path directly. Used by unit tests to avoid mutating the real
/// global config.
fn migrate_workflow_renames_at(config_path: &Path) -> Vec<WorkflowRename> {
    let raw = match fs::read_to_string(config_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(_) => return Vec::new(),
    };

    let mut parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(_) => {
            eprintln!(
                "Warning: invalid JSON in {}; skipping workflow rename migration.",
                config_path.display()
            );
            return Vec::new();
        }
    };

    let workflows = match parsed
        .get_mut("workflows")
        .and_then(|value| value.as_array_mut())
    {
        Some(array) => array,
        None => return Vec::new(),
    };

    // Phase 1: rename legacy IDs in place.
    let mut applied: Vec<WorkflowRename> = Vec::new();
    for entry in workflows.iter_mut() {
        let Some(name) = entry.as_str() else { continue };
        for &(old, new) in WORKFLOW_RENAMES {
            if name == old {
                *entry = serde_json::Value::String(new.to_string());
                applied.push(WorkflowRename { old, new });
                break;
            }
        }
    }

    if applied.is_empty() {
        return Vec::new();
    }

    // Phase 2: dedupe while preserving the order of first occurrence.
    let mut seen: HashSet<String> = HashSet::new();
    workflows.retain(|value| match value.as_str() {
        Some(name) => seen.insert(name.to_string()),
        None => true,
    });

    let serialized = match serde_json::to_string_pretty(&parsed) {
        Ok(text) => format!("{text}\n"),
        Err(_) => return Vec::new(),
    };

    match fs::write(config_path, serialized) {
        Ok(()) => applied,
        Err(error) => {
            eprintln!(
                "Warning: failed to write migrated global config to {}: {error}",
                config_path.display()
            );
            Vec::new()
        }
    }
}

#[cfg(test)]
mod workflow_rename_tests {
    use super::*;

    fn write_config_with_workflows(path: &Path, workflows: &[&str]) {
        let body = serde_json::json!({
            "featureFlags": {},
            "profile": "custom",
            "delivery": "both",
            "workflows": workflows,
        });
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&body).unwrap()),
        )
        .unwrap();
    }

    fn read_workflow_names(path: &Path) -> Vec<String> {
        let raw = fs::read_to_string(path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        parsed
            .get("workflows")
            .and_then(|value| value.as_array())
            .map(|array| {
                array
                    .iter()
                    .filter_map(|value| value.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn renames_apply_to_implement() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.json");
        write_config_with_workflows(&path, &["apply", "archive", "explore"]);

        let applied = migrate_workflow_renames_at(&path);

        assert_eq!(
            applied,
            vec![WorkflowRename {
                old: "apply",
                new: "implement"
            }]
        );
        assert_eq!(
            read_workflow_names(&path),
            vec!["implement", "archive", "explore"]
        );
    }

    #[test]
    fn no_op_when_already_migrated() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.json");
        let original = format!(
            "{}\n",
            serde_json::to_string_pretty(&serde_json::json!({
                "featureFlags": {},
                "profile": "custom",
                "workflows": ["implement", "archive"]
            }))
            .unwrap()
        );
        fs::write(&path, &original).unwrap();

        let applied = migrate_workflow_renames_at(&path);

        assert!(applied.is_empty());
        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(
            after, original,
            "file should not be rewritten when no rename applies"
        );
    }

    #[test]
    fn dedupes_after_rename() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.json");
        write_config_with_workflows(&path, &["apply", "implement", "archive"]);

        let applied = migrate_workflow_renames_at(&path);

        assert_eq!(
            applied,
            vec![WorkflowRename {
                old: "apply",
                new: "implement"
            }]
        );
        assert_eq!(read_workflow_names(&path), vec!["implement", "archive"]);
    }

    #[test]
    fn missing_file_is_silent_no_op() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("does-not-exist.json");

        let applied = migrate_workflow_renames_at(&path);

        assert!(applied.is_empty());
        assert!(!path.exists());
    }

    #[test]
    fn malformed_json_logs_and_does_not_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.json");
        fs::write(&path, "not valid json {").unwrap();

        let applied = migrate_workflow_renames_at(&path);

        assert!(applied.is_empty());
        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(
            after, "not valid json {",
            "malformed config must be left untouched"
        );
    }

    #[test]
    fn missing_workflows_array_is_no_op() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.json");
        let body = serde_json::json!({
            "featureFlags": {},
            "profile": "core"
        });
        let original = format!("{}\n", serde_json::to_string_pretty(&body).unwrap());
        fs::write(&path, &original).unwrap();

        let applied = migrate_workflow_renames_at(&path);

        assert!(applied.is_empty());
        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(after, original, "no-op configs must not be rewritten");
    }
}
