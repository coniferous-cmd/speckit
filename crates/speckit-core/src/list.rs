//! List command: enumerates active changes or discovered specs.
//!
//! Port of the TypeScript `ListCommand`. Supports two modes:
//! - **changes**: reads `speckit/changes/` (excluding `archive/`), shows each
//!   change's name, task status, and relative last-modified time.
//! - **specs**: reads `speckit/specs/`, shows each spec's id and requirement
//!   count.
//!
//! Both modes accept sort order (`recent` / `name`) and a JSON flag.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use regex::Regex;
use serde::Serialize;

use crate::utils::date::format_relative_time;
use crate::utils::spec_discovery::discover_spec_files;
use crate::utils::task_progress::{TaskProgress, format_task_status, get_task_progress_for_change};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Sort order for list output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListSort {
    /// Most-recently modified first (default).
    Recent,
    /// Alphabetical by name.
    Name,
}

impl Default for ListSort {
    fn default() -> Self {
        Self::Recent
    }
}

/// Mode selector: list changes or specs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListMode {
    Changes,
    Specs,
}

impl Default for ListMode {
    fn default() -> Self {
        Self::Changes
    }
}

/// Options controlling list behaviour.
#[derive(Debug, Clone, Default)]
pub struct ListOptions {
    /// Sort order (default: recent).
    pub sort: ListSort,
    /// Emit JSON instead of human-readable text.
    pub json: bool,
}

/// The `list` command.
pub struct ListCommand;

impl ListCommand {
    /// Execute the list command.
    ///
    /// `target_path` is the project root (typically `.`).
    /// `mode` selects whether to list changes or specs.
    /// `options` controls sort order and output format.
    pub fn execute(&self, target_path: &Path, mode: ListMode, options: &ListOptions) -> Result<()> {
        match mode {
            ListMode::Changes => self.list_changes(target_path, options),
            ListMode::Specs => self.list_specs(target_path, options),
        }
    }

    // -- changes mode -------------------------------------------------------

    fn list_changes(&self, target_path: &Path, options: &ListOptions) -> Result<()> {
        let changes_dir = target_path.join("speckit").join("changes");

        let entries = read_change_directory_entries(&changes_dir);
        let change_names: Vec<String> = entries
            .into_iter()
            .filter(|name| name != "archive")
            .collect();

        if change_names.is_empty() {
            if options.json {
                let output = JsonChangesOutput { changes: vec![] };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("No active changes found.");
            }
            return Ok(());
        }

        // Collect information about each change.
        let mut changes: Vec<ChangeInfo> = Vec::with_capacity(change_names.len());
        for name in &change_names {
            let change_path = changes_dir.join(name);
            let progress = get_task_progress_for_change(&change_path).unwrap_or(TaskProgress {
                completed: 0,
                total: 0,
                percentage: 0.0,
            });
            let last_modified = get_last_modified(&change_path);
            changes.push(ChangeInfo {
                name: name.clone(),
                completed_tasks: progress.completed,
                total_tasks: progress.total,
                last_modified,
            });
        }

        // Sort.
        match options.sort {
            ListSort::Recent => {
                changes.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
            }
            ListSort::Name => {
                changes.sort_by(|a, b| a.name.cmp(&b.name));
            }
        }

        // Output.
        if options.json {
            let json_items: Vec<JsonChangeItem> = changes
                .iter()
                .map(|c| {
                    let status = if c.total_tasks == 0 {
                        "no-tasks"
                    } else if c.completed_tasks == c.total_tasks {
                        "complete"
                    } else {
                        "in-progress"
                    };
                    JsonChangeItem {
                        name: c.name.clone(),
                        completed_tasks: c.completed_tasks,
                        total_tasks: c.total_tasks,
                        last_modified: c.last_modified.to_rfc3339(),
                        status: status.to_string(),
                    }
                })
                .collect();
            let output = JsonChangesOutput {
                changes: json_items,
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!("Changes:");
            let padding = "  ";
            let name_width = changes.iter().map(|c| c.name.len()).max().unwrap_or(0);
            for change in &changes {
                let padded_name = format!("{:<width$}", change.name, width = name_width);
                let percentage = if change.total_tasks > 0 {
                    (change.completed_tasks as f64 / change.total_tasks as f64) * 100.0
                } else {
                    0.0
                };
                let status = format_task_status(&crate::utils::task_progress::TaskProgress {
                    total: change.total_tasks,
                    completed: change.completed_tasks,
                    percentage,
                });
                let time_ago = format_relative_time(&change.last_modified);
                println!(
                    "{}{}     {:<12}  {}",
                    padding, padded_name, status, time_ago
                );
            }
        }

        Ok(())
    }

    // -- specs mode ---------------------------------------------------------

    fn list_specs(&self, target_path: &Path, options: &ListOptions) -> Result<()> {
        let specs_dir = target_path.join("speckit").join("specs");

        if !specs_dir.exists() {
            if options.json {
                let output = JsonSpecsOutput { specs: vec![] };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("No specs found.");
            }
            return Ok(());
        }

        let discovered = discover_spec_files(&specs_dir)
            .with_context(|| format!("discovering specs in {}", specs_dir.display()))?;

        if discovered.is_empty() {
            if options.json {
                let output = JsonSpecsOutput { specs: vec![] };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("No specs found.");
            }
            return Ok(());
        }

        let mut specs: Vec<SpecInfo> = Vec::with_capacity(discovered.len());
        for spec_path in &discovered {
            let requirement_count = match fs::read_to_string(spec_path) {
                Ok(content) => count_requirements(&content),
                Err(_) => 0,
            };
            let id = spec_path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            specs.push(SpecInfo {
                id,
                requirement_count,
            });
        }

        // Specs are always sorted by id (already sorted by discover_spec_files,
        // but we re-sort to be explicit and match the TS behaviour).
        specs.sort_by(|a, b| a.id.cmp(&b.id));

        if options.json {
            let json_items: Vec<JsonSpecItem> = specs
                .iter()
                .map(|s| JsonSpecItem {
                    id: s.id.clone(),
                    requirement_count: s.requirement_count,
                })
                .collect();
            let output = JsonSpecsOutput { specs: json_items };
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!("Specs:");
            let padding = "  ";
            let name_width = specs.iter().map(|s| s.id.len()).max().unwrap_or(0);
            for spec in &specs {
                let padded = format!("{:<width$}", spec.id, width = name_width);
                println!(
                    "{}{}     requirements {}",
                    padding, padded, spec.requirement_count
                );
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Information about a single change (changes mode).
#[derive(Debug, Clone)]
struct ChangeInfo {
    name: String,
    completed_tasks: usize,
    total_tasks: usize,
    last_modified: DateTime<Local>,
}

/// Information about a single spec (specs mode).
#[derive(Debug, Clone)]
struct SpecInfo {
    id: String,
    requirement_count: usize,
}

/// Read directory entries from `changes_dir`, returning the names of
/// subdirectories. Returns an empty list when the directory does not exist.
fn read_change_directory_entries(changes_dir: &Path) -> Vec<String> {
    let entries = match fs::read_dir(changes_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(_) => return Vec::new(),
    };

    entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            if file_type.is_dir() {
                Some(entry.file_name().to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Get the most recent modification time of any file in a directory (recursive).
/// Falls back to the directory's own mtime if no files are found.
fn get_last_modified(dir_path: &Path) -> DateTime<Local> {
    let mut latest: Option<std::time::SystemTime> = None;
    walk_for_mtime(dir_path, &mut latest);

    let sys_time = latest.unwrap_or_else(|| {
        fs::metadata(dir_path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });

    DateTime::<Local>::from(sys_time)
}

/// Recursive helper that walks a directory tree tracking the most recent
/// modification time of any file encountered.
fn walk_for_mtime(dir: &Path, latest: &mut Option<std::time::SystemTime>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if file_type.is_dir() {
            walk_for_mtime(&entry.path(), latest);
        } else {
            let modified = match entry.metadata().and_then(|m| m.modified()) {
                Ok(t) => t,
                Err(_) => continue,
            };
            if latest.is_none() || modified > latest.unwrap() {
                *latest = Some(modified);
            }
        }
    }
}

/// Count `### Requirement:` headers in a spec markdown file.
fn count_requirements(content: &str) -> usize {
    let re = Regex::new(r"(?m)^###\s+Requirement\s*:").unwrap();
    re.find_iter(content).count()
}

// ---------------------------------------------------------------------------
// JSON output types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct JsonChangesOutput {
    changes: Vec<JsonChangeItem>,
}

#[derive(Debug, Serialize)]
struct JsonChangeItem {
    name: String,
    completed_tasks: usize,
    total_tasks: usize,
    last_modified: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct JsonSpecsOutput {
    specs: Vec<JsonSpecItem>,
}

#[derive(Debug, Serialize)]
struct JsonSpecItem {
    id: String,
    requirement_count: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn read_change_directory_entries_missing() {
        let entries = read_change_directory_entries(Path::new("/nonexistent/path"));
        assert!(entries.is_empty());
    }

    #[test]
    fn read_change_directory_entries_excludes_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("my-change")).unwrap();
        fs::write(dir.path().join("file.txt"), "content").unwrap();

        let entries = read_change_directory_entries(dir.path());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], "my-change");
    }

    #[test]
    fn get_last_modified_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = get_last_modified(dir.path());
        // Should not panic; returns a valid DateTime.
        assert!(result <= Local::now());
    }

    #[test]
    fn get_last_modified_with_files() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("file.txt"), "content").unwrap();

        let result = get_last_modified(dir.path());
        // Should pick up the nested file's mtime.
        assert!(result <= Local::now());
    }

    #[test]
    fn count_requirements_none() {
        assert_eq!(count_requirements("# Spec\nSome text\n"), 0);
    }

    #[test]
    fn count_requirements_multiple() {
        let content = "\
## Requirements

### Requirement: First
Some text

### Requirement: Second
More text
";
        assert_eq!(count_requirements(content), 2);
    }

    #[test]
    fn list_changes_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let cmd = ListCommand;
        // Should not error on a missing changes dir.
        cmd.execute(dir.path(), ListMode::Changes, &ListOptions::default())
            .unwrap();
    }

    #[test]
    fn list_specs_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let cmd = ListCommand;
        // Should not error on a missing specs dir.
        cmd.execute(dir.path(), ListMode::Specs, &ListOptions::default())
            .unwrap();
    }

    #[test]
    fn list_changes_with_entries() {
        let dir = tempfile::tempdir().unwrap();
        let changes_dir = dir.path().join("speckit").join("changes");
        fs::create_dir_all(changes_dir.join("my-change")).unwrap();
        fs::write(
            changes_dir.join("my-change/tasks.md"),
            "- [x] Done\n- [ ] Todo",
        )
        .unwrap();
        // Create archive dir (should be excluded).
        fs::create_dir_all(changes_dir.join("archive")).unwrap();

        let cmd = ListCommand;
        cmd.execute(dir.path(), ListMode::Changes, &ListOptions::default())
            .unwrap();
    }

    #[test]
    fn list_changes_json_output() {
        let dir = tempfile::tempdir().unwrap();
        let changes_dir = dir.path().join("speckit").join("changes");
        fs::create_dir_all(changes_dir.join("feat-one")).unwrap();
        fs::write(
            changes_dir.join("feat-one/tasks.md"),
            "- [x] Done\n- [ ] Todo",
        )
        .unwrap();

        let cmd = ListCommand;
        let opts = ListOptions {
            json: true,
            ..Default::default()
        };
        cmd.execute(dir.path(), ListMode::Changes, &opts).unwrap();
    }

    #[test]
    fn list_specs_with_entries() {
        let dir = tempfile::tempdir().unwrap();
        let specs_dir = dir.path().join("speckit").join("specs");
        fs::create_dir_all(specs_dir.join("auth")).unwrap();
        fs::write(
            specs_dir.join("auth/spec.md"),
            "# Auth\n## Requirements\n### Requirement: Login\nDetails\n",
        )
        .unwrap();

        let cmd = ListCommand;
        cmd.execute(dir.path(), ListMode::Specs, &ListOptions::default())
            .unwrap();
    }

    #[test]
    fn list_specs_json_output() {
        let dir = tempfile::tempdir().unwrap();
        let specs_dir = dir.path().join("speckit").join("specs");
        fs::create_dir_all(specs_dir.join("web")).unwrap();
        fs::write(
            specs_dir.join("web/spec.md"),
            "# Web\n## Requirements\n### Requirement: Pages\nDetails\n",
        )
        .unwrap();

        let cmd = ListCommand;
        let opts = ListOptions {
            json: true,
            ..Default::default()
        };
        cmd.execute(dir.path(), ListMode::Specs, &opts).unwrap();
    }

    #[test]
    fn list_changes_sort_name() {
        let dir = tempfile::tempdir().unwrap();
        let changes_dir = dir.path().join("speckit").join("changes");
        fs::create_dir_all(changes_dir.join("beta")).unwrap();
        fs::create_dir_all(changes_dir.join("alpha")).unwrap();

        let cmd = ListCommand;
        let opts = ListOptions {
            sort: ListSort::Name,
            ..Default::default()
        };
        cmd.execute(dir.path(), ListMode::Changes, &opts).unwrap();
    }
}
