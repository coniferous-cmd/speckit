use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Input document for task numbering validation.
#[derive(Debug, Clone)]
pub struct TaskNumberingDocument {
    pub path: String,
    pub content: String,
}

/// A single task-numbering issue found in a document.
#[derive(Debug, Clone)]
pub struct TaskNumberingIssue {
    pub path: String,
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
struct TaskLocation {
    path: String,
    line: usize,
}

/// Matches any level-2 heading (`## ...`), up to 3 leading spaces.
static LEVEL_TWO_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ {0,3}##[^#](?:[ \t]+|[ \t]*\r?$)").unwrap());

/// Matches a numbered group heading: `## N.` where N is one or more digits.
static NUMBERED_GROUP_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ {0,3}##[ \t]+(\d+)\.(?:[ \t]|\r?$)").unwrap());

/// Matches a task ID at the start of a description: `1.2.3A` etc.
static TASK_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+(?:\.\d+)+(?:[A-Za-z]+)?)(?:(?=\s)|$)").unwrap());

/// Matches a Markdown task (checkbox) line: `- [ ] ...` or `* [x] ...`.
static TASK_LINE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[-*]\s*\[([\sxX])\]\s*(.*)").unwrap());

/// Parse task lines from content, returning `(done, description)` pairs.
fn parse_task_lines(content: &str) -> Vec<(bool, String)> {
    content
        .lines()
        .filter_map(|line| {
            let caps = TASK_LINE_PATTERN.captures(line)?;
            let done = caps[1].eq_ignore_ascii_case("x");
            let description = caps[2].trim().to_string();
            Some((done, description))
        })
        .collect()
}

/// Strip leading zeros from a numeric string for comparison purposes.
fn strip_leading_zeros(s: &str) -> String {
    let stripped = s.trim_start_matches('0');
    if stripped.is_empty() {
        "0".to_string()
    } else {
        stripped.to_string()
    }
}

/// Finds ambiguous task references across the task files tracked by a change.
///
/// Numbering is interpreted only inside `## N.` groups.  Unnumbered sections,
/// unnumbered tasks, and files without numbered groups are intentionally
/// ignored.
pub fn find_task_numbering_issues(documents: &[TaskNumberingDocument]) -> Vec<TaskNumberingIssue> {
    let mut issues: Vec<TaskNumberingIssue> = Vec::new();
    let mut first_location_by_id: HashMap<String, TaskLocation> = HashMap::new();

    for document in documents {
        let lines: Vec<&str> = document.content.lines().collect();

        // Skip documents that have no numbered group headings at all.
        if !lines.iter().any(|l| NUMBERED_GROUP_HEADING.is_match(l)) {
            continue;
        }

        let mut current_group: Option<String> = None;

        for (index, line) in lines.iter().enumerate() {
            if LEVEL_TWO_HEADING.is_match(line) {
                current_group = NUMBERED_GROUP_HEADING
                    .captures(line)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string());
            }

            let group = match &current_group {
                Some(g) => g.clone(),
                None => continue,
            };

            // Extract the task description from a checkbox line.
            let tasks = parse_task_lines(line);
            let task_desc = match tasks.first() {
                Some((_done, desc)) => desc.clone(),
                None => continue,
            };

            let task_id = match TASK_ID.captures(&task_desc).and_then(|c| c.get(1)) {
                Some(m) => m.as_str().to_string(),
                None => continue,
            };

            let line_number = index + 1;

            // Check that the leading group number matches the current heading group.
            let task_group = task_id.split('.').next().unwrap_or("").to_string();
            let normalized_task_group = strip_leading_zeros(&task_group);
            let normalized_current_group = strip_leading_zeros(&group);

            if normalized_task_group != normalized_current_group {
                issues.push(TaskNumberingIssue {
                    path: document.path.clone(),
                    line: line_number,
                    message: format!(
                        "Task \"{task_id}\" is under group {group}, but its leading number points \
                         to group {task_group}. Move it to group {task_group} or renumber it."
                    ),
                });
            }

            // Duplicate detection.
            match first_location_by_id.get(&task_id) {
                Some(first) => {
                    let first_declaration = if first.path == document.path {
                        format!("on line {}", first.line)
                    } else {
                        format!("in {} on line {}", first.path, first.line)
                    };
                    issues.push(TaskNumberingIssue {
                        path: document.path.clone(),
                        line: line_number,
                        message: format!(
                            "Task ID \"{task_id}\" is duplicated; it was first declared {first_declaration}."
                        ),
                    });
                }
                None => {
                    first_location_by_id.insert(
                        task_id,
                        TaskLocation {
                            path: document.path.clone(),
                            line: line_number,
                        },
                    );
                }
            }
        }
    }

    issues
}
