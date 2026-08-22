//! Validate Command
//!
//! Validate changes and specs.

use std::path::Path;
use tokio::task::JoinSet;

use serde::{Deserialize, Serialize};

use speckit_core::validation::{ValidationLevel, ValidationReport, Validator};

/// Options for the top-level validate command.
#[derive(Debug, Clone)]
pub struct ValidateOptions {
    pub all: bool,
    pub changes: bool,
    pub specs: bool,
    pub archived: bool,
    pub item_type: Option<String>,
    pub strict: bool,
    pub json: bool,
    pub concurrency: Option<String>,
    pub no_interactive: bool,
    pub store: Option<String>,
}

/// A single bulk validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkItemResult {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
    pub duration_ms: u64,
}

/// A validation issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub level: String,
    pub path: String,
    pub message: String,
}

/// Summary statistics for validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub totals: SummaryTotals,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by_type: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryTotals {
    pub items: usize,
    pub passed: usize,
    pub failed: usize,
}

/// Full validation output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateOutput {
    pub items: Vec<BulkItemResult>,
    pub summary: ValidationSummary,
    pub version: String,
}

/// Execute the top-level validate command.
pub async fn validate_command(
    item_name: Option<&str>,
    options: ValidateOptions,
) -> anyhow::Result<()> {
    let project_root = crate::change::resolve_project_root(options.store.as_deref()).await?;

    let bulk = options.all || options.changes || options.specs;

    // Archived-task linting
    if options.archived {
        return run_archived_task_validation(&project_root, options.json).await;
    }

    // Bulk validation
    if bulk {
        return run_bulk_validation(
            &project_root,
            options.all || options.changes,
            options.all || options.specs,
            options.strict,
            options.json,
            options.concurrency.as_deref(),
        )
        .await;
    }

    // No item and no flags
    if item_name.is_none() {
        if !options.no_interactive && atty_is_tty() {
            return run_interactive_selector(
                &project_root,
                options.strict,
                options.json,
                options.concurrency.as_deref(),
            )
            .await;
        }
        eprintln!("Nothing to validate. Try one of:");
        eprintln!("  speckit validate --all");
        eprintln!("  speckit validate --changes");
        eprintln!("  speckit validate --specs");
        eprintln!("  speckit validate <item-name>");
        eprintln!("Or run in an interactive terminal.");
        std::process::exit(1);
    }

    // Direct item validation
    let name = item_name.unwrap();
    let type_override = options.item_type.as_deref().and_then(normalize_type);

    let changes = crate::change::get_active_change_ids(&project_root).await?;
    let specs = crate::spec::get_spec_ids(&project_root).await?;

    let is_change = changes.contains(&name.to_string());
    let is_spec = specs.contains(&name.to_string());

    let item_type = type_override.or_else(|| {
        if is_change {
            Some("change")
        } else if is_spec {
            Some("spec")
        } else {
            None
        }
    });

    match item_type {
        Some(t) => validate_by_type(&project_root, t, name, options.strict, options.json).await,
        None => {
            let mut all_items = changes.clone();
            all_items.extend(specs.iter().cloned());
            let suggestions = nearest_matches(name, &all_items);
            let msg = if suggestions.is_empty() {
                format!("Unknown item '{name}'.")
            } else {
                format!(
                    "Unknown item '{name}'. Did you mean: {}?",
                    suggestions.join(", ")
                )
            };
            if options.json {
                crate::shared_output::print_json(&serde_json::json!({
                    "status": [{ "severity": "error", "code": "unknown_item", "message": msg }]
                }));
            } else {
                eprintln!("{msg}");
            }
            std::process::exit(1);
        }
    }
}

/// Run interactive selector.
async fn run_interactive_selector(
    project_root: &str,
    strict: bool,
    json: bool,
    concurrency: Option<&str>,
) -> anyhow::Result<()> {
    let choice = inquire::Select::new(
        "What would you like to validate?",
        vec![
            "All (changes + specs)",
            "All changes",
            "All specs",
            "Pick a specific change or spec",
        ],
    )
    .prompt()
    .map_err(|e| anyhow::anyhow!("Selection cancelled: {e}"))?;

    match choice {
        "All (changes + specs)" => {
            run_bulk_validation(project_root, true, true, strict, json, concurrency).await
        }
        "All changes" => {
            run_bulk_validation(project_root, true, false, strict, json, concurrency).await
        }
        "All specs" => {
            run_bulk_validation(project_root, false, true, strict, json, concurrency).await
        }
        "Pick a specific change or spec" => {
            let changes = crate::change::get_active_change_ids(project_root).await?;
            let specs = crate::spec::get_spec_ids(project_root).await?;
            let mut items: Vec<(String, String)> = Vec::new();
            for id in &changes {
                items.push((format!("change/{id}"), id.clone()));
            }
            for id in &specs {
                items.push((format!("spec/{id}"), id.clone()));
            }
            if items.is_empty() {
                eprintln!("No items found to validate.");
                std::process::exit(1);
            }
            let labels: Vec<&str> = items.iter().map(|(l, _)| l.as_str()).collect();
            let picked = inquire::Select::new("Pick an item", labels)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Selection cancelled: {e}"))?;
            let (item_type, id) = items.iter().find(|(l, _)| l == picked).cloned().unwrap();
            let t = if item_type.starts_with("change") {
                "change"
            } else {
                "spec"
            };
            validate_by_type(project_root, t, &id, strict, json).await
        }
        _ => unreachable!(),
    }
}

/// Validate a single item by type.
async fn validate_by_type(
    project_root: &str,
    item_type: &str,
    id: &str,
    strict: bool,
    json: bool,
) -> anyhow::Result<()> {
    let start = std::time::Instant::now();

    let (valid, issues) = if item_type == "change" {
        let change_dir = Path::new(project_root)
            .join("speckit")
            .join("changes")
            .join(id);
        validate_change_dir(&change_dir, strict).await
    } else {
        let spec_file = Path::new(project_root)
            .join("speckit")
            .join("specs")
            .join(id)
            .join("spec.md");
        validate_spec_file(&spec_file, strict).await
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    let result = BulkItemResult {
        id: id.to_string(),
        item_type: item_type.to_string(),
        valid,
        issues,
        duration_ms,
    };

    if json {
        let output = ValidateOutput {
            items: vec![result],
            summary: ValidationSummary {
                totals: SummaryTotals {
                    items: 1,
                    passed: if valid { 1 } else { 0 },
                    failed: if valid { 0 } else { 1 },
                },
                by_type: None,
            },
            version: "1.0".to_string(),
        };
        crate::shared_output::print_json(&output);
    } else if valid {
        let label = if item_type == "change" {
            "Change"
        } else {
            "Specification"
        };
        println!("{label} '{id}' is valid");
    } else {
        let label = if item_type == "change" {
            "Change"
        } else {
            "Specification"
        };
        eprintln!("{label} '{id}' has issues");
        for issue in &result.issues {
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
        print_next_steps(item_type, id, &result.issues);
    }

    if !valid {
        std::process::exit(1);
    }

    Ok(())
}

/// Run bulk validation.
async fn run_bulk_validation(
    project_root: &str,
    validate_changes: bool,
    validate_specs: bool,
    strict: bool,
    json: bool,
    concurrency: Option<&str>,
) -> anyhow::Result<()> {
    let concurrency = resolve_concurrency(concurrency)?;
    let changes = if validate_changes {
        crate::change::get_active_change_ids(project_root).await?
    } else {
        Vec::new()
    };
    let specs = if validate_specs {
        crate::spec::get_spec_ids(project_root).await?
    } else {
        Vec::new()
    };

    if changes.is_empty() && specs.is_empty() {
        if json {
            let output = ValidateOutput {
                items: Vec::new(),
                summary: ValidationSummary {
                    totals: SummaryTotals {
                        items: 0,
                        passed: 0,
                        failed: 0,
                    },
                    by_type: None,
                },
                version: "1.0".to_string(),
            };
            crate::shared_output::print_json(&output);
        } else {
            println!("No items found to validate.");
        }
        return Ok(());
    }

    let mut results: Vec<BulkItemResult> = Vec::new();
    let mut jobs = JoinSet::new();

    for id in changes {
        let root = project_root.to_string();
        jobs.spawn(async move {
            let start = std::time::Instant::now();
            let change_dir = Path::new(&root).join("speckit").join("changes").join(&id);
            let (valid, issues) = validate_change_dir(&change_dir, strict).await;
            BulkItemResult {
                id,
                item_type: "change".to_string(),
                valid,
                issues,
                duration_ms: start.elapsed().as_millis() as u64,
            }
        });
        if jobs.len() >= concurrency {
            results.push(
                jobs.join_next()
                    .await
                    .ok_or_else(|| anyhow::anyhow!("validation worker exited unexpectedly"))??,
            );
        }
    }

    for id in specs {
        let root = project_root.to_string();
        jobs.spawn(async move {
            let start = std::time::Instant::now();
            let spec_file = Path::new(&root)
                .join("speckit")
                .join("specs")
                .join(&id)
                .join("spec.md");
            let (valid, issues) = validate_spec_file(&spec_file, strict).await;
            BulkItemResult {
                id,
                item_type: "spec".to_string(),
                valid,
                issues,
                duration_ms: start.elapsed().as_millis() as u64,
            }
        });
        if jobs.len() >= concurrency {
            results.push(
                jobs.join_next()
                    .await
                    .ok_or_else(|| anyhow::anyhow!("validation worker exited unexpectedly"))??,
            );
        }
    }

    while let Some(result) = jobs.join_next().await {
        results.push(result?);
    }

    results.sort_by(|a, b| a.id.cmp(&b.id));

    let passed = results.iter().filter(|r| r.valid).count();
    let failed = results.len() - passed;

    if json {
        let output = ValidateOutput {
            items: results,
            summary: ValidationSummary {
                totals: SummaryTotals {
                    items: passed + failed,
                    passed,
                    failed,
                },
                by_type: None,
            },
            version: "1.0".to_string(),
        };
        crate::shared_output::print_json(&output);
    } else {
        for result in &results {
            if result.valid {
                println!("\u{2713} {}/{}", result.item_type, result.id);
            } else {
                eprintln!("\u{2717} {}/{}", result.item_type, result.id);
            }
        }
        println!(
            "Totals: {passed} passed, {failed} failed ({} items)",
            passed + failed
        );
        if let Some(first_failure) = results.iter().find(|r| !r.valid) {
            println!(
                "Details: speckit validate {} --type {}",
                first_failure.id, first_failure.item_type
            );
        }
    }

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn resolve_concurrency(value: Option<&str>) -> anyhow::Result<usize> {
    let raw = value
        .map(str::to_owned)
        .or_else(|| std::env::var("SPECKIT_CONCURRENCY").ok())
        .or_else(|| std::env::var("OPENSPEC_CONCURRENCY").ok())
        .unwrap_or_else(|| "6".to_string());
    let parsed = raw
        .parse::<usize>()
        .map_err(|_| anyhow::anyhow!("Invalid concurrency '{raw}'; expected a positive integer"))?;
    if parsed == 0 {
        anyhow::bail!("Concurrency must be a positive integer");
    }
    Ok(parsed)
}

/// Run archived task validation.
async fn run_archived_task_validation(project_root: &str, json: bool) -> anyhow::Result<()> {
    let archive_dir = Path::new(project_root)
        .join("speckit")
        .join("changes")
        .join("archive");

    if !archive_dir.is_dir() {
        if json {
            let output = ValidateOutput {
                items: Vec::new(),
                summary: ValidationSummary {
                    totals: SummaryTotals {
                        items: 0,
                        passed: 0,
                        failed: 0,
                    },
                    by_type: None,
                },
                version: "1.0".to_string(),
            };
            crate::shared_output::print_json(&output);
        } else {
            println!("No archived changes found.");
        }
        return Ok(());
    }

    let mut ids = Vec::new();
    let mut read_dir = tokio::fs::read_dir(&archive_dir).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let ft = entry.file_type().await?;
        if ft.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') {
                ids.push(name);
            }
        }
    }
    ids.sort();

    let mut results = Vec::new();
    for id in &ids {
        let start = std::time::Instant::now();
        let tasks_path = archive_dir.join(id).join("tasks.md");
        let mut issues = Vec::new();

        if tasks_path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&tasks_path).await {
                let mut total: usize = 0;
                let mut completed: usize = 0;
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
                let incomplete = total.saturating_sub(completed);
                if incomplete > 0 {
                    issues.push(ValidationIssue {
                        level: "ERROR".to_string(),
                        path: "tasks.md".to_string(),
                        message: format!(
                            "{incomplete} incomplete task{} ({completed}/{total} completed)",
                            if incomplete == 1 { "" } else { "s" }
                        ),
                    });
                }
            } else {
                issues.push(ValidationIssue {
                    level: "ERROR".to_string(),
                    path: "tasks.md".to_string(),
                    message: "could not read task file".to_string(),
                });
            }
        }

        let valid = issues.is_empty();
        results.push(BulkItemResult {
            id: id.clone(),
            item_type: "change".to_string(),
            valid,
            issues,
            duration_ms: start.elapsed().as_millis() as u64,
        });
    }

    let passed = results.iter().filter(|r| r.valid).count();
    let failed = results.len() - passed;

    if json {
        let output = ValidateOutput {
            items: results,
            summary: ValidationSummary {
                totals: SummaryTotals {
                    items: passed + failed,
                    passed,
                    failed,
                },
                by_type: None,
            },
            version: "1.0".to_string(),
        };
        crate::shared_output::print_json(&output);
    } else {
        for result in &results {
            if result.valid {
                println!("\u{2713} change/{}", result.id);
            } else {
                eprintln!("\u{2717} change/{}", result.id);
                for issue in &result.issues {
                    eprintln!("  \u{2717} {}", issue.message);
                }
            }
        }
        println!(
            "Totals: {passed} passed, {failed} failed ({} items)",
            passed + failed
        );
    }

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Validate a change directory.
async fn validate_change_dir(change_dir: &Path, strict: bool) -> (bool, Vec<ValidationIssue>) {
    let mut issues = Vec::new();

    if !change_dir.is_dir() {
        issues.push(ValidationIssue {
            level: "ERROR".to_string(),
            path: "change/".to_string(),
            message: "Change directory not found".to_string(),
        });
        return (false, issues);
    }

    let proposal = change_dir.join("proposal.md");
    if !proposal.exists() {
        issues.push(ValidationIssue {
            level: "WARNING".to_string(),
            path: "proposal.md".to_string(),
            message: "proposal.md not found".to_string(),
        });
    } else {
        append_core_report(
            &mut issues,
            Validator::new(strict).validate_change(&proposal),
        );
    }

    let specs_dir = change_dir.join("specs");
    if !specs_dir.is_dir() {
        issues.push(ValidationIssue {
            level: "WARNING".to_string(),
            path: "specs/".to_string(),
            message: "No specs/ directory found".to_string(),
        });
    }

    let valid = if strict {
        !issues
            .iter()
            .any(|i| i.level == "ERROR" || i.level == "WARNING")
    } else {
        !issues.iter().any(|i| i.level == "ERROR")
    };
    (valid, issues)
}

/// Validate a spec file.
async fn validate_spec_file(spec_file: &Path, strict: bool) -> (bool, Vec<ValidationIssue>) {
    let mut issues = Vec::new();

    if !spec_file.exists() {
        issues.push(ValidationIssue {
            level: "ERROR".to_string(),
            path: "spec.md".to_string(),
            message: "spec.md not found".to_string(),
        });
        return (false, issues);
    }

    append_core_report(&mut issues, Validator::new(strict).validate_spec(spec_file));

    let valid = if strict {
        !issues
            .iter()
            .any(|i| i.level == "ERROR" || i.level == "WARNING")
    } else {
        !issues.iter().any(|i| i.level == "ERROR")
    };
    (valid, issues)
}

fn append_core_report(issues: &mut Vec<ValidationIssue>, report: anyhow::Result<ValidationReport>) {
    match report {
        Ok(report) => {
            issues.extend(report.issues.into_iter().map(|issue| {
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
            }));
        }
        Err(error) => issues.push(ValidationIssue {
            level: "ERROR".to_string(),
            path: "file".to_string(),
            message: error.to_string(),
        }),
    }
}

/// Print next-step hints.
fn print_next_steps(item_type: &str, _id: &str, _issues: &[ValidationIssue]) {
    eprintln!("Next steps:");
    if item_type == "change" {
        eprintln!(
            "  - Ensure change has deltas in specs/: use headers ## ADDED/MODIFIED/REMOVED/RENAMED Requirements"
        );
        eprintln!("  - Each requirement MUST include at least one #### Scenario: block");
    } else {
        eprintln!("  - Ensure spec includes ## Purpose and ## Requirements sections");
        eprintln!("  - Each requirement MUST include at least one #### Scenario: block");
    }
}

/// Normalize type string.
fn normalize_type(value: &str) -> Option<&'static str> {
    match value.to_lowercase().as_str() {
        "change" => Some("change"),
        "spec" => Some("spec"),
        _ => None,
    }
}

/// Find nearest matches for a string in a list.
fn nearest_matches(query: &str, candidates: &[String]) -> Vec<String> {
    let query_lower = query.to_lowercase();
    let mut scored: Vec<(String, usize)> = candidates
        .iter()
        .map(|c| {
            let c_lower = c.to_lowercase();
            let distance = levenshtein_distance(&query_lower, &c_lower);
            (c.clone(), distance)
        })
        .collect();
    scored.sort_by_key(|(_, d)| *d);
    scored
        .into_iter()
        .take(5)
        .filter(|(_, d)| *d <= 5)
        .map(|(name, _)| name)
        .collect()
}

/// Simple Levenshtein distance.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();

    let mut dp = vec![vec![0; m + 1]; n + 1];
    for i in 0..=n {
        dp[i][0] = i;
    }
    for j in 0..=m {
        dp[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[n][m]
}

fn atty_is_tty() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stderr())
}
