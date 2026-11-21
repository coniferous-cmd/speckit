use anyhow::{Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::artifact_graph::{resolve_artifact_outputs, resolve_schema};
use crate::{change_metadata, project_config};

static TASK_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:[-*+]|[0-9]{1,9}[.)])\s*\[(\s*[^\]\s]?\s*)\](.*)").unwrap()
});

static PARAGRAPH_BREAK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*(?:$|#{1,6}(?:\s|$)|```|~~~|>|<[!?]|(?:=+|-+)\s*$|(?:(?:-\s*){3,}|(?:\*\s*){3,}|(?:_\s*){3,})$|(?:[-*+]|[0-9]{1,9}[.)])(?:\s|$))",
    )
    .unwrap()
});

#[derive(Debug)]
pub struct ParsedTask {
    pub line: usize,
    pub done: bool,
    pub description: String,
}

/// Parse task checkboxes and the first paragraph of each task description.
pub fn parse_task_lines(content: &str) -> Vec<ParsedTask> {
    let mut tasks: Vec<ParsedTask> = Vec::new();
    let mut in_task = false;

    for (index, line) in content.lines().enumerate() {
        if let Some(captures) = TASK_LINE.captures(line) {
            let marker = &captures[1];
            let description = &captures[2];
            // A one-character Markdown link label can look like a checkbox.
            if !description.starts_with(['(', '['])
                || (!marker.is_empty() && marker.trim().is_empty())
            {
                tasks.push(ParsedTask {
                    line: index + 1,
                    done: marker.trim().eq_ignore_ascii_case("x"),
                    description: description.trim().to_owned(),
                });
                in_task = true;
                continue;
            }
        }

        if PARAGRAPH_BREAK.is_match(line) {
            in_task = false;
        } else if in_task {
            let description = &mut tasks.last_mut().expect("active task").description;
            let backslashes = description
                .bytes()
                .rev()
                .take_while(|b| *b == b'\\')
                .count();
            if backslashes % 2 == 1 {
                description.truncate(description.len() - 1);
                description.truncate(description.trim_end().len());
            }
            if !description.is_empty() {
                description.push(' ');
            }
            description.push_str(line.trim());
        }
    }

    tasks
}

/// Task progress information.
#[derive(Debug, Clone, Default)]
pub struct TaskProgress {
    pub completed: usize,
    pub total: usize,
    pub percentage: f64,
}

/// Get task progress for a change.
pub fn get_task_progress_for_change(
    change_dir: &Path,
    project_root: &Path,
) -> Result<Option<TaskProgress>> {
    let files = resolve_task_files_for_change(change_dir, project_root);
    if files.is_empty() {
        return Ok(None);
    }

    let mut progress = TaskProgress::default();
    for file in files {
        let content = std::fs::read_to_string(&file)
            .with_context(|| format!("Failed to read tracking file {}", file.display()))?;
        let (completed, total) = count_tasks(&content);
        progress.completed += completed;
        progress.total += total;
    }
    if progress.total > 0 {
        progress.percentage = progress.completed as f64 / progress.total as f64 * 100.0;
    }
    Ok(Some(progress))
}

/// Count completed and total tasks in a tasks.md file.
fn count_tasks(content: &str) -> (usize, usize) {
    let tasks = parse_task_lines(content);
    (tasks.iter().filter(|task| task.done).count(), tasks.len())
}

/// Format task status as a string.
pub fn format_task_status(progress: &TaskProgress) -> String {
    if progress.total == 0 {
        "No tasks".to_string()
    } else {
        format!(
            "{}/{} ({:.0}%)",
            progress.completed, progress.total, progress.percentage
        )
    }
}

/// Resolve task files for a change.
fn resolve_task_files_for_change(change_dir: &Path, project_root: &Path) -> Vec<PathBuf> {
    let schema_name = change_metadata::read_change_metadata(change_dir, project_root)
        .ok()
        .flatten()
        .and_then(|metadata| metadata.schema)
        .or_else(|| project_config::read_project_config(project_root).map(|config| config.schema));
    if let Ok(schema) = resolve_schema(
        schema_name.as_deref().unwrap_or("spec-driven"),
        Some(project_root),
    ) {
        let tracking = schema
            .implement
            .as_ref()
            .and_then(|phase| phase.tracks.as_deref())
            .or_else(|| {
                schema
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.id == "tasks")
                    .map(|artifact| artifact.generates.as_str())
            });
        if let Some(tracking) = tracking {
            let files = resolve_artifact_outputs(change_dir, tracking);
            if !files.is_empty() {
                return files;
            }
        }
    }
    resolve_artifact_outputs(change_dir, "tasks.md")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tasks() {
        let content = r#"## 1. Setup
- [x] 1.1 Create module
- [ ] 1.2 Add tests

## 2. Implementation
- [x] 2.1 Write code
- [ ] 2.2 Review
- [ ] 2.3 Deploy
"#;

        let (completed, total) = count_tasks(content);
        assert_eq!(completed, 2);
        assert_eq!(total, 5);
    }

    #[test]
    fn test_format_task_status() {
        let progress = TaskProgress {
            completed: 3,
            total: 10,
            percentage: 30.0,
        };
        assert_eq!(format_task_status(&progress), "3/10 (30%)");
    }

    #[test]
    fn test_format_task_status_no_tasks() {
        let progress = TaskProgress {
            completed: 0,
            total: 0,
            percentage: 0.0,
        };
        assert_eq!(format_task_status(&progress), "No tasks");
    }
}
