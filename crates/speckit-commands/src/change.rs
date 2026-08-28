//! Change Command
//!
//! Manage Speckit change proposals. (Deprecated: prefer verb-first commands.)

use std::path::Path;

use serde::{Deserialize, Serialize};

use speckit_core::root_selection::{ResolveSpeckitRootOptions, resolve_speckit_root};
use speckit_core::validation::{ValidationLevel, Validator};

/// Options for the change show command.
#[derive(Debug, Clone)]
pub struct ChangeShowOptions {
    pub json: bool,
    pub deltas_only: bool,
    pub requirements_only: bool,
    pub no_interactive: bool,
}

/// Options for the change list command.
#[derive(Debug, Clone)]
pub struct ChangeListOptions {
    pub json: bool,
    pub long: bool,
    pub sort: String,
}

/// Options for the change validate command.
#[derive(Debug, Clone)]
pub struct ChangeValidateOptions {
    pub strict: bool,
    pub json: bool,
    pub no_interactive: bool,
}

/// A change entry for list output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeListEntry {
    pub id: String,
    pub title: String,
    pub delta_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_status: Option<TaskStatus>,
}

/// Task progress information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatus {
    pub total: usize,
    pub completed: usize,
}

/// Validation report for a change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeValidationReport {
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
}

/// A validation issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub level: String,
    pub path: String,
    pub message: String,
}

/// Get active change IDs from the changes directory.
pub async fn get_active_change_ids(project_root: &str) -> anyhow::Result<Vec<String>> {
    let changes_dir = Path::new(project_root).join("speckit").join("changes");
    if !changes_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut ids = Vec::new();
    let mut read_dir = tokio::fs::read_dir(&changes_dir).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let ft = entry.file_type().await?;
        if ft.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name != "archive" && !name.starts_with('.') {
                ids.push(name);
            }
        }
    }
    ids.sort();
    Ok(ids)
}

/// Show a change proposal.
pub async fn change_show(
    change_name: Option<&str>,
    options: ChangeShowOptions,
    root_path: Option<&str>,
) -> anyhow::Result<()> {
    let project_root = root_path.map(|s| s.to_string()).unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string()
    });

    let changes_dir = Path::new(&project_root).join("speckit").join("changes");

    let name = match change_name {
        Some(n) => n.to_string(),
        None => {
            let changes = get_active_change_ids(&project_root).await?;
            if changes.is_empty() {
                eprintln!("No change specified. No active changes found.");
                eprintln!("Hint: use \"speckit change list\" to view available changes.");
                std::process::exit(1);
            }
            if options.no_interactive || !atty_is_tty() {
                let available = changes.join(", ");
                eprintln!("No change specified. Available IDs: {available}");
                eprintln!("Hint: use \"speckit change list\" to view available changes.");
                std::process::exit(1);
            }
            let selection = inquire::Select::new("Select a change to show", changes)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Selection cancelled: {e}"))?;
            selection
        }
    };

    let change_dir = changes_dir.join(&name);
    let proposal_path = change_dir.join("proposal.md");

    if !change_dir.is_dir() {
        anyhow::bail!("Change \"{name}\" not found at {}", proposal_path.display());
    }

    if !proposal_path.exists() {
        // Check if directory exists but no proposal
        anyhow::bail!(
            "Change \"{name}\" has no proposal.md yet. Run \"speckit status --change {name}\" to see which artifact comes next."
        );
    }

    if options.json {
        let content = tokio::fs::read_to_string(&proposal_path).await?;
        let title = extract_title(&content, &name);
        let deltas = extract_deltas(&content);

        if options.requirements_only {
            eprintln!("Flag --requirements-only is deprecated; use --deltas-only instead.");
        }

        let output = serde_json::json!({
            "id": name,
            "title": title,
            "delta_count": deltas.len(),
            "deltas": if options.deltas_only { deltas } else { deltas },
        });
        crate::shared_output::print_json(&output);
        return Ok(());
    }

    let content = tokio::fs::read_to_string(&proposal_path).await?;
    println!("{content}");
    Ok(())
}

/// List active changes.
pub async fn change_list(options: ChangeListOptions, store: Option<&str>) -> anyhow::Result<()> {
    let project_root = resolve_project_root(store).await?;
    let changes_dir = Path::new(&project_root).join("speckit").join("changes");

    let changes = get_active_change_ids(&project_root).await?;

    if options.json {
        let mut entries: Vec<ChangeListEntry> = Vec::new();
        for name in &changes {
            let change_dir = changes_dir.join(name);
            let proposal_path = change_dir.join("proposal.md");

            let (title, delta_count) = if proposal_path.exists() {
                match tokio::fs::read_to_string(&proposal_path).await {
                    Ok(content) => {
                        let title = extract_title(&content, name);
                        let deltas = extract_deltas(&content);
                        (title, deltas.len())
                    }
                    Err(_) => ("Unknown".to_string(), 0),
                }
            } else {
                (name.clone(), 0)
            };

            let task_status = get_task_progress(&changes_dir, name).await;

            entries.push(ChangeListEntry {
                id: name.clone(),
                title,
                delta_count,
                task_status,
            });
        }
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        crate::shared_output::print_json(&entries);
        return Ok(());
    }

    if changes.is_empty() {
        println!("No items found");
        return Ok(());
    }

    let mut sorted = changes.clone();
    sorted.sort();

    if !options.long {
        for id in &sorted {
            println!("{id}");
        }
        return Ok(());
    }

    for name in &sorted {
        let change_dir = changes_dir.join(name);
        let proposal_path = change_dir.join("proposal.md");
        let task_status = get_task_progress(&changes_dir, name).await;
        let task_text = match &task_status {
            Some(ts) if ts.total > 0 => format!(" [tasks {}/{}]", ts.completed, ts.total),
            _ => String::new(),
        };

        if !proposal_path.exists() {
            println!("{name}: (no proposal.md yet){task_text}");
            continue;
        }

        match tokio::fs::read_to_string(&proposal_path).await {
            Ok(content) => {
                let title = extract_title(&content, name);
                let deltas = extract_deltas(&content);
                println!("{name}: {title} [deltas {}]{task_text}", deltas.len());
            }
            Err(_) => {
                println!("{name}: (unable to read){task_text}");
            }
        }
    }

    Ok(())
}

/// Validate a change.
pub async fn change_validate(
    change_name: Option<&str>,
    options: ChangeValidateOptions,
) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?.to_string_lossy().to_string();
    let changes_dir = Path::new(&project_root).join("speckit").join("changes");

    let name = match change_name {
        Some(n) => n.to_string(),
        None => {
            let changes = get_active_change_ids(&project_root).await?;
            if changes.is_empty() {
                eprintln!("No change specified. No active changes found.");
                eprintln!("Hint: use \"speckit change list\" to view available changes.");
                std::process::exit(1);
            }
            if options.no_interactive || !atty_is_tty() {
                let available = changes.join(", ");
                eprintln!("No change specified. Available IDs: {available}");
                eprintln!("Hint: use \"speckit change list\" to view available changes.");
                std::process::exit(1);
            }
            inquire::Select::new("Select a change to validate", changes)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Selection cancelled: {e}"))?
        }
    };

    let change_dir = changes_dir.join(&name);
    if !change_dir.is_dir() {
        anyhow::bail!("Change \"{name}\" not found at {}", change_dir.display());
    }

    let report = validate_change(&name, &change_dir, options.strict).await;

    if options.json {
        crate::shared_output::print_json(&report);
    } else if report.valid {
        println!("Change \"{name}\" is valid");
    } else {
        eprintln!("Change \"{name}\" has issues");
        for issue in &report.issues {
            let prefix = match issue.level.as_str() {
                "ERROR" => "\u{2717}",
                "WARNING" => "\u{26A0}",
                _ => "\u{2139}",
            };
            eprintln!(
                "{prefix} [{}] {}: {}",
                issue.level, issue.path, issue.message
            );
        }
        print_next_steps(&report.issues);
    }

    if !report.valid {
        std::process::exit(1);
    }

    Ok(())
}

/// Validate a change directory.
async fn validate_change(_name: &str, change_dir: &Path, strict: bool) -> ChangeValidationReport {
    let mut issues = Vec::new();

    let proposal_path = change_dir.join("proposal.md");
    if !proposal_path.exists() {
        issues.push(ValidationIssue {
            level: "WARNING".to_string(),
            path: "proposal.md".to_string(),
            message: "proposal.md not found".to_string(),
        });
    } else {
        match Validator::new(strict).validate_change(&proposal_path) {
            Ok(report) => issues.extend(report.issues.into_iter().map(|issue| {
                ValidationIssue {
                    level: match issue.level {
                        ValidationLevel::Error => "ERROR",
                        ValidationLevel::Warning => "WARNING",
                        ValidationLevel::Info => "INFO",
                    }
                    .to_string(),
                    path: issue.path,
                    message: issue.message,
                }
            })),
            Err(error) => issues.push(ValidationIssue {
                level: "ERROR".to_string(),
                path: "proposal.md".to_string(),
                message: error.to_string(),
            }),
        }
    }

    // Check for specs
    let specs_dir = change_dir.join("specs");
    if !specs_dir.is_dir() {
        issues.push(ValidationIssue {
            level: "WARNING".to_string(),
            path: "specs/".to_string(),
            message:
                "No specs/ directory found. Use ## ADDED/MODIFIED/REMOVED Requirements headers."
                    .to_string(),
        });
    } else {
        // Check for spec files
        let has_specs = std::fs::read_dir(&specs_dir)
            .ok()
            .map(|mut rd| {
                rd.any(|e| {
                    e.ok()
                        .map(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if !has_specs {
            issues.push(ValidationIssue {
                level: "WARNING".to_string(),
                path: "specs/".to_string(),
                message: "specs/ directory exists but contains no .md files".to_string(),
            });
        }
    }

    // Strict mode: check tasks
    if strict {
        let tasks_path = change_dir.join("tasks.md");
        if !tasks_path.exists() {
            issues.push(ValidationIssue {
                level: "WARNING".to_string(),
                path: "tasks.md".to_string(),
                message: "tasks.md not found".to_string(),
            });
        }
    }

    ChangeValidationReport {
        valid: if strict {
            !issues
                .iter()
                .any(|i| i.level == "ERROR" || i.level == "WARNING")
        } else {
            !issues.iter().any(|i| i.level == "ERROR")
        },
        issues,
    }
}

/// Extract the title from a proposal.md content.
fn extract_title(content: &str, fallback: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            let title = trimmed[2..].trim();
            // Strip "Change: " prefix if present
            return title.strip_prefix("Change: ").unwrap_or(title).to_string();
        }
    }
    fallback.to_string()
}

/// Extract deltas (## headers with ADDED/MODIFIED/REMOVED) from content.
fn extract_deltas(content: &str) -> Vec<String> {
    let mut deltas = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") {
            let header = trimmed[3..].trim().to_string();
            if header.contains("ADDED")
                || header.contains("MODIFIED")
                || header.contains("REMOVED")
                || header.contains("RENAMED")
            {
                deltas.push(header);
            }
        }
    }
    deltas
}

/// Get task progress for a change.
async fn get_task_progress(changes_dir: &Path, change_name: &str) -> Option<TaskStatus> {
    let tasks_path = changes_dir.join(change_name).join("tasks.md");
    if !tasks_path.exists() {
        return None;
    }
    let content = tokio::fs::read_to_string(&tasks_path).await.ok()?;
    let mut total = 0;
    let mut completed = 0;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- [") {
            if rest.len() > 2 && rest.as_bytes()[1] == b']' {
                total += 1;
                if rest.as_bytes()[0] == b'x' || rest.as_bytes()[0] == b'X' {
                    completed += 1;
                }
            }
        }
    }
    Some(TaskStatus { total, completed })
}

/// Print next-step hints after validation failure.
fn print_next_steps(issues: &[ValidationIssue]) {
    let has_skip_specs_conflict = issues.iter().any(|i| i.message.contains("skip_specs"));
    let has_no_deltas = issues
        .iter()
        .any(|i| i.message.contains("No specs/") || i.message.contains("no .md files"));

    eprintln!("Next steps:");
    if has_skip_specs_conflict {
        eprintln!(
            "  - This change declares skip_specs (no spec deltas): delete the files under specs/, or remove skip_specs from .speckit.yaml if requirements do change"
        );
        eprintln!(
            "  - skip_specs is only honored when .speckit.yaml is valid change metadata (schema: <name> is required)"
        );
    } else if has_no_deltas {
        eprintln!(
            "  - Ensure change has deltas in specs/: use headers ## ADDED/MODIFIED/REMOVED/RENAMED Requirements"
        );
        eprintln!("  - Each requirement MUST include at least one #### Scenario: block");
        eprintln!("  - Debug parsed deltas: speckit change show <id> --json --deltas-only");
    }
}

/// Check if stderr is a TTY.
fn atty_is_tty() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stderr())
}

// ---------------------------------------------------------------------------
// Store / root resolution helpers
// ---------------------------------------------------------------------------

/// Resolve the Speckit project root for a command, using the unified store
/// resolver so `--store <id>` actually takes effect instead of silently falling
/// back to the working directory.
pub async fn resolve_project_root(store: Option<&str>) -> anyhow::Result<String> {
    let cwd = std::env::current_dir()
        .map_err(|e| anyhow::anyhow!("Cannot get current directory: {e}"))?;
    let resolved = resolve_speckit_root(&ResolveSpeckitRootOptions {
        store: store.map(|s| s.to_string()),
        store_path: None,
        start_path: Some(cwd),
        allow_implicit_root: Some(true),
        global_data_dir: None,
    })
    .map_err(|e| anyhow::anyhow!("{}", e.diagnostic.message))?;
    Ok(resolved.path.to_string_lossy().into_owned())
}
