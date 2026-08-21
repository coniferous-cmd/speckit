//! Update Command: refreshes Speckit skills and commands for configured tools.
//!
//! Supports profile-aware updates, delivery changes, migration, and smart
//! update detection.

use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;

use crate::config::AI_TOOLS;
use crate::legacy_cleanup;
use crate::migration;
use crate::planning_home;

/// Options for the update command.
#[derive(Debug, Clone, Default)]
pub struct UpdateCommandOptions {
    pub force: bool,
}

/// Maps a parsed skill `name` like `speckit-explore` back to its workflow id
/// (e.g. `explore`, `archive-change`, `bulk-archive-change`). Used by update's
/// stale-detection step to confirm the file under inspection matches the
/// workflow the registry expects before deciding to rewrite it.
fn expected_workflow_contains(skill_name: &str, expected_workflow: &str) -> bool {
    // Strip the `speckit-` prefix.
    let rest = skill_name.strip_prefix("speckit-").unwrap_or(skill_name);
    rest == expected_workflow
        || expected_workflow == "bulk-archive" && rest == "bulk-archive-change"
}

/// The update command implementation.
pub struct UpdateCommand {
    force: bool,
}

impl UpdateCommand {
    /// Create a new update command with the given options.
    pub fn new(options: UpdateCommandOptions) -> Self {
        Self {
            force: options.force,
        }
    }

    /// Execute the update command on the given project path.
    pub fn execute(&self, project_path: &Path) -> Result<()> {
        let resolved_path =
            dunce::canonicalize(project_path).unwrap_or_else(|_| project_path.to_path_buf());
        let speckit_path = resolved_path.join("speckit");

        // 1. Check speckit directory exists
        if !speckit_path.exists() {
            return Err(anyhow::anyhow!(
                "No Speckit directory found. Run 'speckit init' first."
            ));
        }

        // 2. Migrate legacy tool directories
        let migrations = migration::migrate_legacy_tool_dirs(
            &resolved_path,
            None,
            &migration::MigrationTiming::BeforeGeneration,
        );
        for migration in &migrations {
            if migration::has_movable_content(migration) {
                println!(
                    "Migrated {}: {} -> {}",
                    migration::describe_legacy_migration(migration),
                    migration.from,
                    migration.to
                );
            }
            if let Some(notice) = migration::kept_in_place_notice(migration) {
                println!("{}", notice);
            }
        }

        // 3. Detect available tools
        let detected_tool_ids = self.detect_available_tools(&resolved_path);
        let detected_tool_refs: Vec<&str> = detected_tool_ids.iter().map(|s| s.as_str()).collect();

        // 4. Perform profile migration if needed
        migration::migrate_if_needed(&resolved_path, &detected_tool_refs)?;

        // 5. Handle legacy cleanup
        self.handle_legacy_cleanup(&resolved_path)?;

        // 6. Find configured tools
        let configured_tools = self.get_configured_tools(&resolved_path);

        if configured_tools.is_empty() {
            println!("No configured tools found.");
            println!("Run 'speckit init' to set up tools.");
            return Ok(());
        }

        // 7. Check version status for all configured tools
        let tools_needing_update = self.check_version_status(&resolved_path, &configured_tools);

        // 8. Smart update detection
        if !self.force && tools_needing_update.is_empty() {
            println!("All tools up to date.");
            println!("Use --force to refresh files anyway.");
            self.detect_new_tools(&resolved_path, &configured_tools);
            return Ok(());
        }

        // 9. Display update plan
        let tools_to_update = if self.force {
            configured_tools.clone()
        } else {
            tools_needing_update
        };

        println!(
            "Updating {} tool(s): {}",
            tools_to_update.len(),
            tools_to_update.join(", ")
        );
        println!();

        // 10. Update tools
        let mut updated_tools = Vec::new();
        let mut failed_tools = Vec::new();

        for tool_id in &tools_to_update {
            let tool = match AI_TOOLS.iter().find(|t| t.value == *tool_id) {
                Some(t) => t,
                None => continue,
            };

            println!("Updating {}...", tool.name);

            match self.update_tool(&resolved_path, tool_id) {
                Ok(_) => {
                    println!("Updated {}", tool.name);
                    updated_tools.push(tool.name.clone());
                }
                Err(e) => {
                    println!("Failed to update {}: {}", tool.name, e);
                    failed_tools.push((tool.name.clone(), e.to_string()));
                }
            }
        }

        // 11. Summary
        println!();
        if !updated_tools.is_empty() {
            println!("Updated: {}", updated_tools.join(", "));
        }
        if !failed_tools.is_empty() {
            let failures: Vec<String> = failed_tools
                .iter()
                .map(|(name, error)| format!("{} ({})", name, error))
                .collect();
            println!("Failed: {}", failures.join(", "));
        }

        // 12. Detect new tool directories
        self.detect_new_tools(&resolved_path, &configured_tools);

        // 13. Show setup notes
        for tool_id in &configured_tools {
            if let Some(tool) = AI_TOOLS.iter().find(|t| t.value == *tool_id) {
                if let Some(ref note) = tool.setup_note {
                    println!("Setup required for {}: {}", tool.name, note);
                }
            }
        }

        println!();
        println!("Restart your IDE for changes to take effect.");

        if !failed_tools.is_empty() {
            let failed_names: Vec<&str> = failed_tools.iter().map(|(n, _)| n.as_str()).collect();
            return Err(anyhow::anyhow!(
                "Speckit update failed for: {}",
                failed_names.join(", ")
            ));
        }

        Ok(())
    }

    /// Detect available tools in the project directory.
    fn detect_available_tools(&self, project_path: &Path) -> Vec<String> {
        AI_TOOLS
            .iter()
            .filter(|tool| {
                if let Some(ref skills_dir) = tool.skills_dir {
                    let paths_to_check = match &tool.detection_paths {
                        Some(paths) => paths.clone(),
                        None => vec![skills_dir.clone()],
                    };
                    paths_to_check.iter().any(|p| project_path.join(p).exists())
                } else {
                    false
                }
            })
            .map(|tool| tool.value.clone())
            .collect()
    }

    /// Get currently configured tools (those with skill directories).
    fn get_configured_tools(&self, project_path: &Path) -> Vec<String> {
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

    /// Check version status for configured tools.
    fn check_version_status(
        &self,
        project_path: &Path,
        configured_tools: &[String],
    ) -> Vec<String> {
        configured_tools
            .iter()
            .filter(|tool_id| {
                // Simple check: look for skill directories and compare
                if let Some(tool) = AI_TOOLS.iter().find(|t| t.value == **tool_id) {
                    if let Some(ref skills_dir) = tool.skills_dir {
                        let skills_path = project_path.join(skills_dir).join("skills");
                        // If skills dir exists but is empty or missing SKILL.md files,
                        // mark as needing update
                        if skills_path.exists() {
                            let has_any_skill = migration::ALL_WORKFLOWS.iter().any(|w| {
                                if let Some(dir_name) = migration::workflow_to_skill_dir(w) {
                                    skills_path.join(dir_name).join("SKILL.md").exists()
                                } else {
                                    false
                                }
                            });
                            return !has_any_skill;
                        }
                    }
                }
                false
            })
            .cloned()
            .collect()
    }

    /// Update a single tool.
    ///
    /// Walks the canonical workflow registry, regenerates every missing or
    /// out-of-date `SKILL.md` via the unified generator (so update produces the
    /// exact same content as `init` would), and removes any leftover skill
    /// directory that is no longer in the registry. Unmanaged skill files
    /// (anything not produced by the canonical generator) are preserved.
    fn update_tool(&self, project_path: &Path, tool_id: &str) -> Result<()> {
        let tool = AI_TOOLS
            .iter()
            .find(|t| t.value == tool_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", tool_id))?;

        let skills_dir = tool
            .skills_dir
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tool {} does not support skills", tool_id))?;

        let skills_path = project_path.join(skills_dir).join("skills");
        std::fs::create_dir_all(&skills_path)?;

        let skill_entries = crate::templates::generation::get_skill_templates(None);
        let generated_by_version =
            crate::templates::generation::speckit_generated_by_version();
        let current_version = generated_by_version.clone();
        let canonical_dirs: std::collections::HashSet<String> = skill_entries
            .iter()
            .map(|e| e.dir_name.clone())
            .collect();

        for entry in &skill_entries {
            let skill_dir = skills_path.join(&entry.dir_name);
            std::fs::create_dir_all(&skill_dir)?;
            let skill_file = skill_dir.join("SKILL.md");
            let desired = crate::templates::generation::generate_skill_content(
                &entry.template,
                &generated_by_version,
                None,
            );

            let should_write = self.force
                || !skill_file.exists()
                || Self::skill_needs_update(&skill_file, &entry.workflow_id, &current_version)?;

            if should_write {
                std::fs::write(&skill_file, desired)?;
            }
        }

        // Remove leftover directories from previous registries (e.g. workflow
        // was removed). Only removes directories whose SKILL.md was generated
        // by speckit - unmanaged skill dirs are preserved.
        if let Ok(rd) = std::fs::read_dir(&skills_path) {
            for dir_entry in rd.flatten() {
                let path = dir_entry.path();
                if !path.is_dir() {
                    continue;
                }
                let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                if canonical_dirs.contains(dir_name) {
                    continue;
                }
                let skill_md = path.join("SKILL.md");
                if !skill_md.exists() {
                    continue;
                }
                if Self::is_managed_skill(&skill_md) {
                    let _ = std::fs::remove_dir_all(&path);
                }
            }
        }

        Ok(())
    }

    /// Returns whether the existing `SKILL.md` is older or stale relative to
    /// the running CLI's version. `Ok(true)` => needs update. `Ok(false)` =>
    /// up to date. `Err(_)` => file is missing or unreadable; callers should
    /// write it.
    fn skill_needs_update(
        skill_file: &std::path::Path,
        expected_workflow: &str,
        current_version: &str,
    ) -> Result<bool> {
        Self::skill_needs_update_static(
            skill_file,
            "",
            expected_workflow,
            current_version,
        )
    }

    /// Pure-function form of `skill_needs_update` that does not depend on
    /// `Self`, used by tests.
    fn skill_needs_update_static(
        skill_file: &std::path::Path,
        _expected_name: &str,
        expected_workflow: &str,
        current_version: &str,
    ) -> Result<bool> {
        let content = std::fs::read_to_string(skill_file)?;
        let Some(parsed) = crate::templates::generation::parse_skill_frontmatter(&content) else {
            return Ok(true);
        };
        // Out-of-registry skill (different name) -> managed by another tool,
        // leave alone.
        if !parsed.name.starts_with("speckit-") && parsed.name != "feedback" {
            return Ok(false);
        }
        if !expected_workflow_contains(&parsed.name, expected_workflow) {
            return Ok(false);
        }
        match parsed.generated_by {
            Some(v) if v == current_version => Ok(false),
            _ => Ok(true),
        }
    }

    /// True when the file's frontmatter looks like one our generator emitted
    /// (`generatedBy` set, `author: speckit`). Used to decide whether a
    /// leftover skill directory is safe to delete during reconcile.
    fn is_managed_skill(skill_file: &std::path::Path) -> bool {
        let Ok(content) = std::fs::read_to_string(skill_file) else {
            return false;
        };
        match crate::templates::generation::parse_skill_frontmatter(&content) {
            Some(parsed) => parsed.generated_by.is_some(),
            None => false,
        }
    }

    /// Handle legacy cleanup detection and execution.
    fn handle_legacy_cleanup(&self, project_path: &Path) -> Result<()> {
        let detection = legacy_cleanup::detect_legacy_artifacts(project_path)?;

        if !detection.has_legacy_artifacts {
            return Ok(());
        }

        let summary = legacy_cleanup::format_detection_summary(&detection);
        if !summary.is_empty() {
            println!("{}", summary);
            println!();
        }

        if self.force {
            let result = legacy_cleanup::cleanup_legacy_artifacts(project_path, &detection)?;
            let cleanup_summary = legacy_cleanup::format_cleanup_summary(&result);
            if !cleanup_summary.is_empty() {
                println!("{}", cleanup_summary);
            }
            println!();
        } else {
            println!("Run with --force to auto-cleanup legacy files.");
            println!();
        }

        Ok(())
    }

    /// Detect new tool directories not currently configured.
    fn detect_new_tools(&self, project_path: &Path, configured_tools: &[String]) {
        let available = self.detect_available_tools(project_path);
        let configured_set: HashSet<&str> = configured_tools.iter().map(|s| s.as_str()).collect();
        let new_tools: Vec<String> = available
            .iter()
            .filter(|t| !configured_set.contains(t.as_str()))
            .cloned()
            .collect();

        if !new_tools.is_empty() {
            let new_tool_names: Vec<String> = new_tools
                .iter()
                .filter_map(|id| {
                    AI_TOOLS
                        .iter()
                        .find(|t| t.value == *id)
                        .map(|t| t.name.clone())
                })
                .collect();
            println!(
                "Detected new tools: {}. Run 'speckit init' to add them.",
                new_tool_names.join(", ")
            );
        }
    }
}

/// Test-only export of `skill_needs_update`.
///
/// Re-exposes the private helper so the parity integration tests can verify
/// stale detection without spinning up a full `UpdateCommand::execute()`.
pub fn skill_needs_update_for_test(
    skill_file: &std::path::Path,
    expected_name: &str,
    expected_workflow: &str,
    current_version: &str,
) -> Result<bool> {
    UpdateCommand::skill_needs_update_static(
        skill_file,
        expected_name,
        expected_workflow,
        current_version,
    )
}
