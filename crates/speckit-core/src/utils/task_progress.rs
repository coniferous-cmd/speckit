use anyhow::Result;
use std::path::Path;

/// Task progress information.
#[derive(Debug, Clone)]
pub struct TaskProgress {
    pub completed: usize,
    pub total: usize,
    pub percentage: f64,
}

/// Get task progress for a change.
pub fn get_task_progress_for_change(change_dir: &Path) -> Result<TaskProgress> {
    let tasks_file = change_dir.join("tasks.md");

    if !tasks_file.exists() {
        return Ok(TaskProgress {
            completed: 0,
            total: 0,
            percentage: 0.0,
        });
    }

    let content = std::fs::read_to_string(&tasks_file)?;
    let (completed, total) = count_tasks(&content);

    let percentage = if total > 0 {
        (completed as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    Ok(TaskProgress {
        completed,
        total,
        percentage,
    })
}

/// Count completed and total tasks in a tasks.md file.
fn count_tasks(content: &str) -> (usize, usize) {
    let mut completed = 0;
    let mut total = 0;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") {
            completed += 1;
            total += 1;
        } else if trimmed.starts_with("- [ ]") {
            total += 1;
        }
    }

    (completed, total)
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
pub fn resolve_task_files_for_change(change_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    let tasks_file = change_dir.join("tasks.md");

    if tasks_file.exists() {
        files.push(tasks_file);
    }

    Ok(files)
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
