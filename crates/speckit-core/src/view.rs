//! View Command: displays a dashboard summary of the Speckit project.
//!
//! Shows summary metrics, progress bars for active changes, and a spec list,
//! mirroring the TypeScript `ViewCommand`.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::root_selection::{ResolveSpeckitRootOptions, resolve_speckit_root};
use crate::utils::task_progress::get_task_progress_for_change;

/// The name of the Speckit directory within a project.
const SPECKIT_DIR_NAME: &str = "speckit";

/// Task progress values extracted for internal sorting/display.
#[derive(Debug, Clone)]
struct ChangeProgress {
    total: usize,
    completed: usize,
}

/// A change entry for the dashboard.
#[derive(Debug, Clone)]
struct ChangeEntry {
    name: String,
    progress: ChangeProgress,
}

/// A spec entry for the dashboard.
#[derive(Debug, Clone)]
struct SpecEntry {
    name: String,
    requirement_count: usize,
}

/// Categorized changes data.
#[derive(Debug, Default)]
struct ChangesData {
    draft: Vec<ChangeEntry>,
    active: Vec<ChangeEntry>,
    completed: Vec<ChangeEntry>,
}

/// The view command implementation.
pub struct ViewCommand;

impl Default for ViewCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewCommand {
    /// Create a new view command.
    pub fn new() -> Self {
        Self
    }

    /// Execute the view command, displaying a dashboard for the project at `target_path`.
    ///
    /// When `store_id` is `Some`, the dashboard is built from the matching
    /// registered store instead of the local working directory; the store id
    /// is shown in the header so the operator can confirm which project they
    /// are looking at.
    pub fn execute(&self, target_path: &Path, store_id: Option<&str>) -> Result<()> {
        let resolved = resolve_speckit_root(&ResolveSpeckitRootOptions {
            store: store_id.map(|s| s.to_string()),
            store_path: None,
            start_path: Some(target_path.to_path_buf()),
            allow_implicit_root: Some(true),
            global_data_dir: None,
        })?;

        let speckit_dir = resolved.path.join(SPECKIT_DIR_NAME);

        if !speckit_dir.exists() {
            anyhow::bail!(
                "No speckit directory found at {} (resolved via {:?}{})",
                resolved.path.display(),
                resolved.source,
                match resolved.store_id.as_deref() {
                    Some(id) => format!(", store={id}"),
                    None => String::new(),
                }
            );
        }

        println!();
        println!("{}", "Speckit Dashboard".bold());
        if let Some(id) = &resolved.store_id {
            println!("  {} {}", "Store:".dimmed(), id.cyan());
        }
        println!(
            "  {} {}",
            "Root:".dimmed(),
            resolved.path.display().to_string().cyan()
        );
        println!("{}", "═".repeat(60));

        let changes_data = self.get_changes_data(&speckit_dir)?;
        let specs_data = self.get_specs_data(&speckit_dir)?;

        self.display_summary(&changes_data, &specs_data);
        self.display_draft_changes(&changes_data);
        self.display_active_changes(&changes_data);
        self.display_completed_changes(&changes_data);
        self.display_specifications(&specs_data);

        println!();
        println!("{}", "═".repeat(60));
        println!(
            "\n{}",
            format!(
                "Use {} or {} for detailed views",
                "speckit list --changes".white(),
                "speckit list --specs".white()
            )
            .dimmed()
        );

        Ok(())
    }

    /// Collect changes data from the speckit/changes/ directory.
    fn get_changes_data(&self, speckit_dir: &Path) -> Result<ChangesData> {
        let changes_dir = speckit_dir.join("changes");

        if !changes_dir.exists() {
            return Ok(ChangesData::default());
        }

        let mut draft = Vec::new();
        let mut active = Vec::new();
        let mut completed = Vec::new();

        let entries = fs::read_dir(&changes_dir).with_context(|| {
            format!(
                "Failed to read changes directory: {}",
                changes_dir.display()
            )
        })?;

        for entry in entries {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            if name == "archive" {
                continue;
            }

            let change_dir = changes_dir.join(&name);
            let progress = match get_task_progress_for_change(&change_dir) {
                Ok(p) => ChangeProgress {
                    total: p.total,
                    completed: p.completed,
                },
                Err(_) => ChangeProgress {
                    total: 0,
                    completed: 0,
                },
            };

            if progress.total == 0 {
                // No tasks defined yet — still in planning/draft phase
                draft.push(ChangeEntry { name, progress });
            } else if progress.completed == progress.total {
                // All tasks complete
                completed.push(ChangeEntry { name, progress });
            } else {
                // Has tasks but not all complete
                active.push(ChangeEntry { name, progress });
            }
        }

        // Sort draft and completed by name for deterministic ordering
        draft.sort_by(|a, b| a.name.cmp(&b.name));
        completed.sort_by(|a, b| a.name.cmp(&b.name));

        // Sort active by completion percentage (ascending), then by name
        active.sort_by(|a, b| {
            let pct_a = if a.progress.total > 0 {
                a.progress.completed as f64 / a.progress.total as f64
            } else {
                0.0
            };
            let pct_b = if b.progress.total > 0 {
                b.progress.completed as f64 / b.progress.total as f64
            } else {
                0.0
            };
            pct_a
                .partial_cmp(&pct_b)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.name.cmp(&b.name))
        });

        Ok(ChangesData {
            draft,
            active,
            completed,
        })
    }

    /// Collect specs data from the speckit/specs/ directory.
    ///
    /// Recursively discovers every `spec.md` under the specs root, counting
    /// `### Requirement:` headers in each file.
    fn get_specs_data(&self, speckit_dir: &Path) -> Result<Vec<SpecEntry>> {
        let specs_dir = speckit_dir.join("specs");

        if !specs_dir.exists() {
            return Ok(Vec::new());
        }

        let mut specs = Vec::new();
        self.discover_specs(&specs_dir, &specs_dir, &mut specs)?;

        // Sort by name for deterministic output
        specs.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(specs)
    }

    /// Recursively walk a specs directory, collecting spec entries.
    ///
    /// A `spec.md` sitting directly in the root is ignored (specs must live in
    /// a capability folder). Dot-directories are skipped.
    fn discover_specs(
        &self,
        specs_root: &Path,
        dir: &Path,
        specs: &mut Vec<SpecEntry>,
    ) -> Result<()> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("Failed to read specs directory: {}", dir.display()));
            }
        };

        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip dot-directories
            if name.starts_with('.') {
                continue;
            }

            let file_type = entry.file_type()?;
            let path = entry.path();

            if file_type.is_dir() {
                self.discover_specs(specs_root, &path, specs)?;
            } else if name == "spec.md" {
                // Only count spec.md files that are inside a capability folder
                // (i.e., at least one level deep from the specs root)
                if let Ok(relative) = path.strip_prefix(specs_root) {
                    let components: Vec<_> = relative.components().collect();
                    if components.len() >= 2 {
                        // Build the spec id from the path relative to specs_root,
                        // excluding the trailing "spec.md" component.
                        let id_components: Vec<String> = components[..components.len() - 1]
                            .iter()
                            .map(|c| c.as_os_str().to_string_lossy().into_owned())
                            .collect();
                        let spec_id = id_components.join("/");
                        let requirement_count = count_requirements_in_file(&path);
                        specs.push(SpecEntry {
                            name: spec_id,
                            requirement_count,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Display summary metrics.
    fn display_summary(&self, changes_data: &ChangesData, specs_data: &[SpecEntry]) {
        let total_specs = specs_data.len();
        let total_requirements: usize = specs_data.iter().map(|s| s.requirement_count).sum();

        // Calculate total task progress across active changes
        let total_tasks: usize = changes_data.active.iter().map(|c| c.progress.total).sum();
        let completed_tasks: usize = changes_data
            .active
            .iter()
            .map(|c| c.progress.completed)
            .sum();

        println!();
        println!("{}", "Summary:".bold());
        println!(
            "  {} {}",
            "●".cyan(),
            format!(
                "Specifications: {} specs, {} requirements",
                total_specs.to_string().bold(),
                total_requirements.to_string().bold()
            )
        );

        if !changes_data.draft.is_empty() {
            println!(
                "  {} {}",
                "●".truecolor(107, 114, 128), // muted gray
                format!(
                    "Draft Changes: {}",
                    changes_data.draft.len().to_string().bold()
                )
            );
        }

        println!(
            "  {} {}",
            "●".yellow(),
            format!(
                "Active Changes: {} in progress",
                changes_data.active.len().to_string().bold()
            )
        );

        println!(
            "  {} {}",
            "●".green(),
            format!(
                "Completed Changes: {}",
                changes_data.completed.len().to_string().bold()
            )
        );

        if total_tasks > 0 {
            let overall_progress =
                ((completed_tasks as f64 / total_tasks as f64) * 100.0).round() as usize;
            println!(
                "  {} {}",
                "●".magenta(),
                format!(
                    "Task Progress: {}/{} ({}% complete)",
                    completed_tasks.to_string().bold(),
                    total_tasks.to_string().bold(),
                    overall_progress.to_string().bold()
                )
            );
        }
    }

    /// Display draft changes section.
    fn display_draft_changes(&self, changes_data: &ChangesData) {
        if changes_data.draft.is_empty() {
            return;
        }

        println!();
        println!("{}", "Draft Changes".bold().truecolor(107, 114, 128));
        println!("{}", "─".repeat(60));
        for change in &changes_data.draft {
            println!("  {} {}", "○".truecolor(107, 114, 128), change.name);
        }
    }

    /// Display active changes section with progress bars.
    fn display_active_changes(&self, changes_data: &ChangesData) {
        if changes_data.active.is_empty() {
            return;
        }

        println!();
        println!("{}", "Active Changes".bold().cyan());
        println!("{}", "─".repeat(60));
        for change in &changes_data.active {
            let progress_bar =
                create_progress_bar(change.progress.completed, change.progress.total, 20);
            let percentage = if change.progress.total > 0 {
                ((change.progress.completed as f64 / change.progress.total as f64) * 100.0).round()
                    as usize
            } else {
                0
            };

            println!(
                "  {} {} {} {}",
                "◉".yellow(),
                format!("{:<30}", change.name).bold(),
                progress_bar,
                format!("{}%", percentage).dimmed()
            );
        }
    }

    /// Display completed changes section.
    fn display_completed_changes(&self, changes_data: &ChangesData) {
        if changes_data.completed.is_empty() {
            return;
        }

        println!();
        println!("{}", "Completed Changes".bold().green());
        println!("{}", "─".repeat(60));
        for change in &changes_data.completed {
            println!("  {} {}", "✓".green(), change.name);
        }
    }

    /// Display specifications section.
    fn display_specifications(&self, specs_data: &[SpecEntry]) {
        if specs_data.is_empty() {
            return;
        }

        println!();
        println!("{}", "Specifications".bold().blue());
        println!("{}", "─".repeat(60));

        // Sort by requirement count descending
        let mut sorted_specs = specs_data.to_vec();
        sorted_specs.sort_by(|a, b| b.requirement_count.cmp(&a.requirement_count));

        for spec in &sorted_specs {
            let req_label = if spec.requirement_count == 1 {
                "requirement"
            } else {
                "requirements"
            };
            println!(
                "  {} {} {}",
                "▪".blue(),
                format!("{:<30}", spec.name).bold(),
                format!("{} {}", spec.requirement_count, req_label).dimmed()
            );
        }
    }
}

/// Count `### Requirement:` headers in a spec file.
fn count_requirements_in_file(path: &Path) -> usize {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return 0,
    };

    content
        .lines()
        .filter(|line| line.starts_with("### Requirement:"))
        .count()
}

/// Create a colored progress bar string.
fn create_progress_bar(completed: usize, total: usize, width: usize) -> String {
    if total == 0 {
        return format!("[{}]", "─".repeat(width).dimmed());
    }

    let percentage = completed as f64 / total as f64;
    let filled = (percentage * width as f64).round() as usize;
    let empty = width - filled;

    let filled_bar = "█".repeat(filled).green();
    let empty_bar = "░".repeat(empty).dimmed();

    format!("[{}{}]", filled_bar, empty_bar)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn execute_fails_without_speckit_dir() {
        let temp = setup_temp_dir();
        let cmd = ViewCommand::new();
        let result = cmd.execute(temp.path(), None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No speckit directory found")
        );
    }

    #[test]
    fn empty_speckit_dir_shows_dashboard() {
        let temp = setup_temp_dir();
        let speckit = temp.path().join("speckit");
        fs::create_dir_all(&speckit).unwrap();

        let cmd = ViewCommand::new();
        let result = cmd.execute(temp.path(), None);
        assert!(result.is_ok());
    }

    #[test]
    fn draft_changes_have_no_tasks() {
        let temp = setup_temp_dir();
        let changes = temp.path().join("speckit").join("changes");
        fs::create_dir_all(changes.join("empty-change")).unwrap();

        let data = ViewCommand::new()
            .get_changes_data(&temp.path().join("speckit"))
            .unwrap();

        assert_eq!(data.draft.len(), 1);
        assert_eq!(data.draft[0].name, "empty-change");
        assert!(data.active.is_empty());
        assert!(data.completed.is_empty());
    }

    #[test]
    fn completed_changes_have_all_tasks_done() {
        let temp = setup_temp_dir();
        let changes = temp.path().join("speckit").join("changes");
        let change_dir = changes.join("done-change");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(change_dir.join("tasks.md"), "- [x] Done task\n").unwrap();

        let data = ViewCommand::new()
            .get_changes_data(&temp.path().join("speckit"))
            .unwrap();

        assert!(data.draft.is_empty());
        assert!(data.active.is_empty());
        assert_eq!(data.completed.len(), 1);
        assert_eq!(data.completed[0].name, "done-change");
    }

    #[test]
    fn active_changes_have_partial_tasks() {
        let temp = setup_temp_dir();
        let changes = temp.path().join("speckit").join("changes");
        let change_dir = changes.join("wip-change");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(change_dir.join("tasks.md"), "- [x] Done\n- [ ] Not done\n").unwrap();

        let data = ViewCommand::new()
            .get_changes_data(&temp.path().join("speckit"))
            .unwrap();

        assert!(data.draft.is_empty());
        assert_eq!(data.active.len(), 1);
        assert_eq!(data.active[0].name, "wip-change");
        assert_eq!(data.active[0].progress.total, 2);
        assert_eq!(data.active[0].progress.completed, 1);
        assert!(data.completed.is_empty());
    }

    #[test]
    fn active_changes_sorted_by_percentage_ascending() {
        let temp = setup_temp_dir();
        let changes = temp.path().join("speckit").join("changes");

        // 33% done
        let d1 = changes.join("gamma-change");
        fs::create_dir_all(&d1).unwrap();
        fs::write(d1.join("tasks.md"), "- [x] Done\n- [ ] A\n- [ ] B\n").unwrap();

        // 50% done
        let d2 = changes.join("beta-change");
        fs::create_dir_all(&d2).unwrap();
        fs::write(d2.join("tasks.md"), "- [x] Done\n- [ ] Left\n").unwrap();

        // 50% done (tie-breaker: name)
        let d3 = changes.join("delta-change");
        fs::create_dir_all(&d3).unwrap();
        fs::write(d3.join("tasks.md"), "- [x] Done\n- [ ] Left\n").unwrap();

        // 0% done
        let d4 = changes.join("alpha-change");
        fs::create_dir_all(&d4).unwrap();
        fs::write(d4.join("tasks.md"), "- [ ] One\n- [ ] Two\n").unwrap();

        let data = ViewCommand::new()
            .get_changes_data(&temp.path().join("speckit"))
            .unwrap();

        let names: Vec<&str> = data.active.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "alpha-change",
                "gamma-change",
                "beta-change",
                "delta-change"
            ]
        );
    }

    #[test]
    fn specs_discovered_recursively() {
        let temp = setup_temp_dir();
        let specs = temp.path().join("speckit").join("specs");
        let spec_dir = specs.join("my-spec");
        fs::create_dir_all(&spec_dir).unwrap();
        fs::write(
            spec_dir.join("spec.md"),
            "# My Spec\n\nSome overview.\n\n### Requirement: First\n### Requirement: Second\n",
        )
        .unwrap();

        let nested = specs.join("area").join("nested-spec");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            nested.join("spec.md"),
            "# Nested\n\n### Requirement: Only one\n",
        )
        .unwrap();

        let specs_data = ViewCommand::new()
            .get_specs_data(&temp.path().join("speckit"))
            .unwrap();

        assert_eq!(specs_data.len(), 2);
        let my_spec = specs_data.iter().find(|s| s.name == "my-spec").unwrap();
        assert_eq!(my_spec.requirement_count, 2);
        let nested_spec = specs_data
            .iter()
            .find(|s| s.name == "area/nested-spec")
            .unwrap();
        assert_eq!(nested_spec.requirement_count, 1);
    }

    #[test]
    fn count_requirements_in_file_works() {
        let temp = setup_temp_dir();
        let path = temp.path().join("spec.md");
        fs::write(
            &path,
            "# Title\n\n### Requirement: A\nSome text\n### Requirement: B\nMore text\n",
        )
        .unwrap();

        assert_eq!(count_requirements_in_file(&path), 2);
    }

    #[test]
    fn count_requirements_in_file_no_requirements() {
        let temp = setup_temp_dir();
        let path = temp.path().join("spec.md");
        fs::write(&path, "# Title\n\nJust a spec with no requirements.\n").unwrap();

        assert_eq!(count_requirements_in_file(&path), 0);
    }

    #[test]
    fn create_progress_bar_zero_total() {
        let bar = create_progress_bar(0, 0, 20);
        assert!(bar.contains("─"));
    }

    #[test]
    fn create_progress_bar_half() {
        let bar = create_progress_bar(5, 10, 20);
        // Should contain 10 filled and 10 empty chars
        assert!(bar.contains(&"█".repeat(10)));
        assert!(bar.contains(&"░".repeat(10)));
    }

    #[test]
    fn create_progress_bar_full() {
        let bar = create_progress_bar(10, 10, 20);
        assert!(bar.contains(&"█".repeat(20)));
    }
}
