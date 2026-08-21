//! Archive Command: moves completed changes to the archive directory
//! and optionally applies delta specs to main specs.
//!
//! Mirrors the TypeScript archive flow:
//! 1. Resolve change name (interactive if needed)
//! 2. Validate tasks are complete (unless `--no-validate`)
//! 3. Read metadata for `skip_specs` / `retire_capabilities`
//! 4. Compute spec deltas (unless `--skip-specs`)
//! 5. Preview and confirm (unless `--yes`)
//! 6. Apply spec deltas
//! 7. Move change to archive (with concurrency lock)
//! 8. Output JSON result

use anyhow::Result;
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

use crate::archive;
use crate::change_metadata;
use crate::id::folder_style_name_problem;
use crate::specs_apply;
use crate::utils::task_progress::get_task_progress_for_change;

/// Options for the archive command.
#[derive(Debug, Clone, Default)]
pub struct ArchiveOptions {
    pub yes: bool,
    pub skip_specs: bool,
    pub no_validate: bool,
    pub json: bool,
    pub store: Option<String>,
    pub store_path: Option<String>,
}

/// Result of an archive operation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ArchiveResult {
    pub change: String,
    pub archived_as: String,
    pub path: String,
    pub specs_updated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totals: Option<SpecTotals>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

/// Spec update totals.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SpecTotals {
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
    pub renamed: usize,
}

/// Error codes for archive failures.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ArchiveError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

/// The archive command implementation.
pub struct ArchiveCommand;

impl ArchiveCommand {
    /// Execute the archive command.
    pub fn execute(
        change_name: Option<&str>,
        options: &ArchiveOptions,
        project_path: &Path,
    ) -> Result<Option<ArchiveResult>> {
        let project_path =
            dunce::canonicalize(project_path).unwrap_or_else(|_| project_path.to_path_buf());
        let speckit_dir = project_path.join("speckit");
        let changes_dir = speckit_dir.join("changes");
        let archive_dir = changes_dir.join("archive");

        // 1. Resolve change name
        let change_name = match change_name {
            Some(name) => name.to_string(),
            None => Self::prompt_change_name(&changes_dir, options)?,
        };

        // Validate change name format
        if let Some(problem) = folder_style_name_problem(&change_name, "Change name") {
            return Err(anyhow::anyhow!("{}", problem));
        }

        let change_dir = changes_dir.join(&change_name);

        // Verify change exists
        if !change_dir.exists() || !change_dir.is_dir() {
            let available = list_active_change_names(&changes_dir);
            if available.is_empty() {
                return Err(anyhow::anyhow!(
                    "Change '{}' not found. No active changes exist.",
                    change_name
                ));
            }
            return Err(anyhow::anyhow!(
                "Change '{}' not found. Available changes: {}",
                change_name,
                available.join(", ")
            ));
        }

        // 2. Validate tasks are complete (unless --no-validate)
        if !options.no_validate {
            Self::validate_tasks_complete(&change_dir, &change_name)?;
        }

        // 3. Read metadata for skip_specs
        let metadata = change_metadata::read_change_metadata(&change_dir)?;
        let effective_skip_specs =
            options.skip_specs || metadata.as_ref().map_or(false, |m| m.skip_specs);

        // 4. Compute spec deltas (unless skip_specs)
        let mut totals = SpecTotals {
            added: 0,
            modified: 0,
            removed: 0,
            renamed: 0,
        };
        let mut warnings = Vec::new();
        let specs_dir = speckit_dir.join("specs");

        if !effective_skip_specs {
            let change_specs_dir = change_dir.join("specs");
            if change_specs_dir.exists() {
                let updates = specs_apply::find_spec_updates(&change_dir, &specs_dir)?;

                if !updates.is_empty() {
                    // 5. Preview (unless --yes or --json)
                    if !options.yes && !options.json {
                        Self::preview_spec_updates(&updates, &change_name)?;
                        if !Self::confirm_archive()? {
                            println!("Archive cancelled.");
                            return Ok(None);
                        }
                    }

                    // 6. Apply spec deltas
                    for update in &updates {
                        match specs_apply::build_updated_spec(update, &change_name, options.json) {
                            Ok(result) => {
                                // Check if this is a retirement candidate (all requirements removed)
                                if result.no_requirement_blocks && update.exists {
                                    match specs_apply::retire_spec(update, &specs_dir, options.json)
                                    {
                                        Ok(_) => {
                                            totals.removed += 1;
                                            if !options.json {
                                                println!(
                                                    "Retired {}: all requirements removed.",
                                                    update.id
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            warnings.push(format!(
                                                "Failed to retire {}: {}",
                                                update.id, e
                                            ));
                                        }
                                    }
                                } else {
                                    match specs_apply::write_updated_spec(
                                        update,
                                        &result.rebuilt,
                                        &result.counts,
                                        options.json,
                                    ) {
                                        Ok(_) => {
                                            totals.added += result.counts.added;
                                            totals.modified += result.counts.modified;
                                            totals.removed += result.counts.removed;
                                            totals.renamed += result.counts.renamed;
                                        }
                                        Err(e) => {
                                            warnings.push(format!(
                                                "Failed to write {}: {}",
                                                update.id, e
                                            ));
                                        }
                                    }
                                }
                                warnings.extend(result.warnings);
                            }
                            Err(e) => {
                                return Err(anyhow::anyhow!(
                                    "Failed to build spec update for {}: {}",
                                    update.id,
                                    e
                                ));
                            }
                        }
                    }
                }
            }
        }

        // 7. Confirm move (if not already confirmed above)
        if !options.yes && !options.json && !Self::has_spec_updates(&change_dir, &specs_dir) {
            if !Self::confirm_archive()? {
                println!("Archive cancelled.");
                return Ok(None);
            }
        }

        // 8. Move change to archive with concurrency protection
        let date_prefix = Local::now().format("%Y-%m-%d").to_string();
        let archive_name = if change_name.starts_with(&date_prefix) {
            change_name.clone()
        } else {
            format!("{}-{}", date_prefix, change_name)
        };
        let archive_path = archive_dir.join(&archive_name);

        // Check archive destination is available
        if archive_path.exists() {
            return Err(anyhow::anyhow!(
                "Archive '{}' already exists.",
                archive_name
            ));
        }

        // Create archive directory
        fs::create_dir_all(&archive_dir)?;

        // Acquire concurrency lock
        let lock_path = archive_dir.join(".speckit-archive.lock");
        let _lock = Self::acquire_archive_lock(&lock_path, &archive_name)?;

        // Double-check destination after acquiring lock
        if archive_path.exists() {
            return Err(anyhow::anyhow!(
                "Archive '{}' already exists (created while waiting for lock).",
                archive_name
            ));
        }

        // Move change to archive
        if let Err(e) = fs::rename(&change_dir, &archive_path) {
            // Try copy + remove as fallback for cross-device or permission errors
            if e.raw_os_error()
                .map_or(false, |code| code == 18 || code == 1)
            {
                copy_dir_recursive(&change_dir, &archive_path)?;
                fs::remove_dir_all(&change_dir)?;
            } else {
                return Err(anyhow::anyhow!("Failed to move change to archive: {}", e));
            }
        }

        // Release lock (drop happens automatically, but explicit is cleaner)
        drop(_lock);

        if !options.json {
            println!("Change '{}' archived as '{}'.", change_name, archive_name);
            if totals.added > 0 || totals.modified > 0 || totals.removed > 0 || totals.renamed > 0 {
                println!(
                    "Specs updated: +{} ~{} -{} ->{}",
                    totals.added, totals.modified, totals.removed, totals.renamed
                );
            }
        }

        Ok(Some(ArchiveResult {
            change: change_name,
            archived_as: archive_name,
            path: archive_path.to_string_lossy().to_string(),
            specs_updated: totals.added + totals.modified + totals.removed + totals.renamed > 0,
            totals: if totals.added + totals.modified + totals.removed + totals.renamed > 0 {
                Some(totals)
            } else {
                None
            },
            warnings: if warnings.is_empty() {
                None
            } else {
                Some(warnings)
            },
        }))
    }

    /// Prompt the user to select a change name interactively.
    fn prompt_change_name(changes_dir: &Path, options: &ArchiveOptions) -> Result<String> {
        if options.json {
            return Err(anyhow::anyhow!(
                "A change name is required: archive --json is non-interactive."
            ));
        }
        let available = list_active_change_names(changes_dir);
        if available.is_empty() {
            println!("No active changes found.");
            return Err(anyhow::anyhow!("No active changes found."));
        }
        println!("Available changes:");
        for name in &available {
            println!("  {}", name);
        }
        println!("Enter change name:");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let selected = input.trim().to_string();
        if selected.is_empty() {
            return Err(anyhow::anyhow!("No change selected. Aborting."));
        }
        Ok(selected)
    }

    /// Validate that all tasks in the change are complete.
    fn validate_tasks_complete(change_dir: &Path, change_name: &str) -> Result<()> {
        let progress = get_task_progress_for_change(change_dir)?;

        if progress.total > 0 && progress.completed < progress.total {
            return Err(anyhow::anyhow!(
                "Change '{}' has incomplete tasks: {}/{} ({}%). Complete all tasks before archiving, or use --no-validate to skip.",
                change_name,
                progress.completed,
                progress.total,
                progress.percentage as usize
            ));
        }

        Ok(())
    }

    /// Preview spec updates before applying.
    fn preview_spec_updates(updates: &[specs_apply::SpecUpdate], change_name: &str) -> Result<()> {
        println!("The following spec changes will be applied:");
        println!();
        for update in updates {
            if update.exists {
                println!("  ~ {} (modify)", update.id);
            } else {
                println!("  + {} (new)", update.id);
            }
        }
        println!();
        Ok(())
    }

    /// Confirm archive with the user.
    fn confirm_archive() -> Result<bool> {
        println!("Proceed with archive? [y/N]");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let answer = input.trim().to_lowercase();
        Ok(answer == "y" || answer == "yes")
    }

    /// Check if there are spec updates to apply.
    fn has_spec_updates(change_dir: &Path, specs_dir: &Path) -> bool {
        let change_specs_dir = change_dir.join("specs");
        if !change_specs_dir.exists() {
            return false;
        }
        specs_apply::find_spec_updates(change_dir, specs_dir)
            .map(|updates| !updates.is_empty())
            .unwrap_or(false)
    }

    /// Acquire a concurrency lock for archive operations.
    fn acquire_archive_lock(lock_path: &Path, archive_name: &str) -> Result<ArchiveLock> {
        use std::io::Write;

        // Try to create the lock file exclusively
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(lock_path)
        {
            Ok(mut file) => {
                writeln!(file, "{}", std::process::id())?;
                Ok(ArchiveLock {
                    _path: lock_path.to_path_buf(),
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Check if the lock is stale (older than 30 seconds)
                if let Ok(metadata) = fs::metadata(lock_path) {
                    if let Ok(modified) = metadata.modified() {
                        if modified.elapsed().unwrap_or_default().as_secs() > 30 {
                            // Stale lock, remove and retry
                            fs::remove_file(lock_path)?;
                            return Self::acquire_archive_lock(lock_path, archive_name);
                        }
                    }
                }
                Err(anyhow::anyhow!(
                    "Archive '{}' is already being created. If no archive process is running, \
                     remove the stale lock at {} and rerun.",
                    archive_name,
                    lock_path.display()
                ))
            }
            Err(e) => Err(anyhow::anyhow!("Failed to acquire archive lock: {}", e)),
        }
    }
}

/// Guard that removes the archive lock file on drop.
struct ArchiveLock {
    _path: PathBuf,
}

impl Drop for ArchiveLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self._path);
    }
}

/// Check if a rebuilt spec is retirable (only error is no requirements).
pub fn is_retirable_spec(spec_name: &str, rebuilt: &str) -> bool {
    // A spec with no requirement blocks at all is a retirement candidate.
    // This is a simplified check; the full version uses the validator.
    let has_requirement = rebuilt
        .lines()
        .any(|line| line.trim().starts_with("### Requirement:"));
    !has_requirement
}

/// List active change names in the changes directory.
fn list_active_change_names(changes_dir: &Path) -> Vec<String> {
    if !changes_dir.exists() {
        return Vec::new();
    }

    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(changes_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_dir() && name != "archive" {
                names.push(name);
            }
        }
    }
    names.sort();
    names
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn archive_fails_without_change_dir() {
        let tmp = setup_temp_dir();
        let opts = ArchiveOptions::default();
        let result = ArchiveCommand::execute(Some("nonexistent"), &opts, tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn archive_validates_incomplete_tasks() {
        let tmp = setup_temp_dir();
        let changes = tmp.path().join("speckit").join("changes");
        let change_dir = changes.join("my-change");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(change_dir.join("tasks.md"), "- [x] Done\n- [ ] Not done\n").unwrap();

        let opts = ArchiveOptions {
            no_validate: false,
            ..Default::default()
        };
        let result = ArchiveCommand::execute(Some("my-change"), &opts, tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("incomplete tasks"));
    }

    #[test]
    fn archive_skips_validation_with_no_validate() {
        let tmp = setup_temp_dir();
        let changes = tmp.path().join("speckit").join("changes");
        let change_dir = changes.join("my-change");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(change_dir.join("tasks.md"), "- [x] Done\n- [ ] Not done\n").unwrap();

        let opts = ArchiveOptions {
            no_validate: true,
            yes: true,
            ..Default::default()
        };
        let result = ArchiveCommand::execute(Some("my-change"), &opts, tmp.path());
        // Should succeed with --no-validate
        assert!(result.is_ok());
    }

    #[test]
    fn archive_json_requires_change_name() {
        let tmp = setup_temp_dir();
        let opts = ArchiveOptions {
            json: true,
            ..Default::default()
        };
        let result = ArchiveCommand::execute(None, &opts, tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("non-interactive"));
    }

    #[test]
    fn archive_moves_change_to_archive() {
        let tmp = setup_temp_dir();
        let changes = tmp.path().join("speckit").join("changes");
        let change_dir = changes.join("my-change");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(change_dir.join("proposal.md"), "# Proposal\n").unwrap();

        let opts = ArchiveOptions {
            yes: true,
            ..Default::default()
        };
        let result = ArchiveCommand::execute(Some("my-change"), &opts, tmp.path()).unwrap();
        let result = result.unwrap();

        assert_eq!(result.change, "my-change");
        assert!(result.archived_as.contains("my-change"));
        assert!(!result.specs_updated);

        // Verify the change was moved
        assert!(!change_dir.exists());
        assert!(Path::new(&result.path).exists());
    }

    #[test]
    fn archive_blocks_duplicate_destination() {
        let tmp = setup_temp_dir();
        let changes = tmp.path().join("speckit").join("changes");
        let archive_dir = changes.join("archive");
        let change_dir = changes.join("my-change");

        fs::create_dir_all(&change_dir).unwrap();
        fs::write(change_dir.join("proposal.md"), "# Proposal\n").unwrap();

        let date_prefix = Local::now().format("%Y-%m-%d").to_string();
        let archive_name = format!("{}-my-change", date_prefix);
        fs::create_dir_all(archive_dir.join(&archive_name)).unwrap();

        let opts = ArchiveOptions {
            yes: true,
            ..Default::default()
        };
        let result = ArchiveCommand::execute(Some("my-change"), &opts, tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn is_retirable_spec_with_no_requirements() {
        assert!(is_retirable_spec(
            "test",
            "# Spec\n\nNo requirements here.\n"
        ));
    }

    #[test]
    fn is_retirable_spec_with_requirements() {
        assert!(!is_retirable_spec(
            "test",
            "# Spec\n\n### Requirement: Something\n"
        ));
    }

    #[test]
    fn list_active_change_names_works() {
        let tmp = setup_temp_dir();
        let changes = tmp.path().join("changes");
        fs::create_dir_all(changes.join("alpha")).unwrap();
        fs::create_dir_all(changes.join("beta")).unwrap();
        fs::create_dir_all(changes.join("archive")).unwrap();

        let names = list_active_change_names(&changes);
        assert_eq!(names, vec!["alpha", "beta"]);
    }
}
