//! Spec Command
//!
//! Manage and view Speckit specifications.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::shared_output::StoreDiagnostic;

/// Options for the spec show command.
#[derive(Debug, Clone)]
pub struct SpecShowOptions {
    pub json: bool,
    pub requirements: bool,
    pub no_scenarios: bool,
    pub requirement: Option<String>,
    pub no_interactive: bool,
}

/// Options for the spec list command.
#[derive(Debug, Clone)]
pub struct SpecListOptions {
    pub json: bool,
    pub long: bool,
}

/// Options for the spec validate command.
#[derive(Debug, Clone)]
pub struct SpecValidateOptions {
    pub strict: bool,
    pub json: bool,
    pub no_interactive: bool,
}

/// A parsed spec for JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecOutput {
    pub id: String,
    pub title: String,
    pub overview: Option<String>,
    pub requirement_count: usize,
    pub requirements: Vec<RequirementOutput>,
    pub metadata: SpecMetadata,
}

/// A requirement in a spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementOutput {
    pub text: String,
    pub scenarios: Vec<ScenarioOutput>,
}

/// A scenario in a requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioOutput {
    pub text: String,
}

/// Spec metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecMetadata {
    pub version: String,
    pub format: String,
}

/// A spec entry for list output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecListEntry {
    pub id: String,
    pub title: String,
    pub requirement_count: usize,
}

/// Validation report for a spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecValidationReport {
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

/// List all spec IDs in the specs directory.
pub async fn get_spec_ids(project_root: &str) -> anyhow::Result<Vec<String>> {
    let specs_dir = Path::new(project_root).join("speckit").join("specs");
    if !specs_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut ids = Vec::new();
    let mut read_dir = tokio::fs::read_dir(&specs_dir).await?;
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
    Ok(ids)
}

/// Show a spec.
pub async fn spec_show(
    spec_id: Option<&str>,
    options: SpecShowOptions,
    root_path: Option<&str>,
) -> anyhow::Result<()> {
    let project_root = root_path.map(|s| s.to_string()).unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string()
    });

    let specs_dir = Path::new(&project_root).join("speckit").join("specs");

    let id = match spec_id {
        Some(s) => s.to_string(),
        None => {
            let ids = get_spec_ids(&project_root).await?;
            if ids.is_empty() {
                anyhow::bail!("No specs found.");
            }
            // In non-interactive mode, show available specs
            if options.no_interactive || !atty_is_tty() {
                let available = ids.join(", ");
                anyhow::bail!("Missing required argument <spec-id>. Available specs: {available}");
            }
            // Interactive selection
            let selection = inquire::Select::new("Select a spec to show", ids.clone())
                .prompt()
                .map_err(|e| anyhow::anyhow!("Selection cancelled: {e}"))?;
            selection
        }
    };

    let spec_path = specs_dir.join(&id).join("spec.md");
    if !spec_path.exists() {
        let display_path = spec_path.display();
        anyhow::bail!("Spec '{id}' not found at {display_path}");
    }

    if options.json {
        let content = tokio::fs::read_to_string(&spec_path).await?;
        let parsed = parse_spec_from_content(&content, &id);

        // Apply filters
        let filtered_requirements = if let Some(ref req_id) = options.requirement {
            let index: usize = req_id
                .parse::<usize>()
                .map_err(|_| anyhow::anyhow!("Requirement {req_id} not found"))?
                - 1;
            if index >= parsed.requirements.len() {
                anyhow::bail!("Requirement {req_id} not found");
            }
            vec![parsed.requirements[index].clone()]
        } else {
            parsed.requirements.clone()
        };

        let filtered: Vec<RequirementOutput> = filtered_requirements
            .into_iter()
            .map(|mut req| {
                if options.requirements || options.no_scenarios {
                    req.scenarios = Vec::new();
                }
                req
            })
            .collect();

        let output = serde_json::json!({
            "id": id,
            "title": parsed.title,
            "overview": parsed.overview,
            "requirement_count": filtered.len(),
            "requirements": filtered,
            "metadata": parsed.metadata,
        });
        crate::shared_output::print_json(&output);
        return Ok(());
    }

    // Text mode: print raw markdown
    let content = tokio::fs::read_to_string(&spec_path).await?;
    println!("{content}");
    Ok(())
}

/// List all specs.
pub async fn spec_list(options: SpecListOptions, root_path: Option<&str>) -> anyhow::Result<()> {
    let project_root = root_path.map(|s| s.to_string()).unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string()
    });

    let specs_dir = Path::new(&project_root).join("speckit").join("specs");
    if !specs_dir.is_dir() {
        if options.json {
            crate::shared_output::print_json(&serde_json::json!([]));
            return Ok(());
        }
        println!("No items found");
        return Ok(());
    }

    let spec_ids = get_spec_ids(&project_root).await?;
    let mut entries: Vec<SpecListEntry> = Vec::new();

    for id in &spec_ids {
        let spec_path = specs_dir.join(id).join("spec.md");
        let (title, req_count) = if spec_path.exists() {
            match tokio::fs::read_to_string(&spec_path).await {
                Ok(content) => {
                    let parsed = parse_spec_from_content(&content, id);
                    (parsed.title, parsed.requirements.len())
                }
                Err(_) => (id.clone(), 0),
            }
        } else {
            (id.clone(), 0)
        };
        entries.push(SpecListEntry {
            id: id.clone(),
            title,
            requirement_count: req_count,
        });
    }

    entries.sort_by(|a, b| a.id.cmp(&b.id));

    if options.json {
        crate::shared_output::print_json(&entries);
        return Ok(());
    }

    if entries.is_empty() {
        println!("No items found");
        return Ok(());
    }

    if options.long {
        for entry in &entries {
            println!(
                "{}: {} [requirements {}]",
                entry.id, entry.title, entry.requirement_count
            );
        }
    } else {
        for entry in &entries {
            println!("{}", entry.id);
        }
    }

    Ok(())
}

/// Validate a spec.
pub async fn spec_validate(
    spec_id: Option<&str>,
    options: SpecValidateOptions,
) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?.to_string_lossy().to_string();
    let specs_dir = Path::new(&project_root).join("speckit").join("specs");

    let id = match spec_id {
        Some(s) => s.to_string(),
        None => {
            let ids = get_spec_ids(&project_root).await?;
            if ids.is_empty() {
                anyhow::bail!("No specs found.");
            }
            if options.no_interactive || !atty_is_tty() {
                let available = ids.join(", ");
                anyhow::bail!("Missing required argument <spec-id>. Available specs: {available}");
            }
            let selection = inquire::Select::new("Select a spec to validate", ids.clone())
                .prompt()
                .map_err(|e| anyhow::anyhow!("Selection cancelled: {e}"))?;
            selection
        }
    };

    let spec_path = specs_dir.join(&id).join("spec.md");
    if !spec_path.exists() {
        anyhow::bail!("Spec '{id}' not found at speckit/specs/{id}/spec.md");
    }

    let content = tokio::fs::read_to_string(&spec_path).await?;
    let report = validate_spec_content(&content, &id, options.strict);

    if options.json {
        crate::shared_output::print_json(&report);
        if !report.valid {
            std::process::exit(1);
        }
        return Ok(());
    }

    if report.valid {
        println!("Specification '{id}' is valid");
    } else {
        eprintln!("Specification '{id}' has issues");
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
    }

    Ok(())
}

/// Parse a spec from its markdown content.
fn parse_spec_from_content(content: &str, spec_id: &str) -> SpecOutput {
    let mut title = spec_id.to_string();
    let mut overview = None;
    let mut requirements = Vec::new();
    let mut current_req_text = String::new();
    let mut current_scenarios = Vec::new();
    let mut in_overview = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Top-level heading = title
        if trimmed.starts_with("# ") && !trimmed.starts_with("## ") {
            title = trimmed[2..].trim().to_string();
            continue;
        }

        if trimmed.to_lowercase() == "## purpose" || trimmed.to_lowercase() == "## overview" {
            in_overview = true;
            continue;
        }

        if trimmed.starts_with("## ") && in_overview {
            in_overview = false;
        }

        if in_overview && !trimmed.is_empty() {
            overview.get_or_insert_with(String::new).push_str(trimmed);
            overview.as_mut().unwrap().push('\n');
            continue;
        }

        // Requirements section headers
        if trimmed.starts_with("### ") {
            // Save previous requirement
            if !current_req_text.is_empty() || !current_scenarios.is_empty() {
                requirements.push(RequirementOutput {
                    text: std::mem::take(&mut current_req_text),
                    scenarios: std::mem::take(&mut current_scenarios),
                });
            }
            current_req_text = trimmed[4..].trim().to_string();
            continue;
        }

        // Scenarios
        if trimmed.starts_with("#### ") {
            let scenario_text = trimmed[5..].trim().to_string();
            current_scenarios.push(ScenarioOutput {
                text: scenario_text,
            });
            continue;
        }
    }

    // Push last requirement
    if !current_req_text.is_empty() || !current_scenarios.is_empty() {
        requirements.push(RequirementOutput {
            text: current_req_text,
            scenarios: current_scenarios,
        });
    }

    SpecOutput {
        id: spec_id.to_string(),
        title,
        overview: overview.map(|o| o.trim().to_string()),
        requirement_count: requirements.len(),
        requirements,
        metadata: SpecMetadata {
            version: "1.0.0".to_string(),
            format: "speckit".to_string(),
        },
    }
}

/// Validate a spec's content.
fn validate_spec_content(content: &str, spec_id: &str, strict: bool) -> SpecValidationReport {
    let mut issues = Vec::new();

    let parsed = parse_spec_from_content(content, spec_id);

    // Check for title
    if parsed.title == spec_id {
        issues.push(ValidationIssue {
            level: "WARNING".to_string(),
            path: "spec.md".to_string(),
            message: "No top-level heading found; using spec ID as title".to_string(),
        });
    }

    // Check for requirements
    if parsed.requirements.is_empty() {
        issues.push(ValidationIssue {
            level: "ERROR".to_string(),
            path: "spec.md".to_string(),
            message: "No requirements found. Use ### Requirement: headers.".to_string(),
        });
    }

    // Strict mode: check for scenarios
    if strict {
        for (i, req) in parsed.requirements.iter().enumerate() {
            if req.scenarios.is_empty() {
                issues.push(ValidationIssue {
                    level: "ERROR".to_string(),
                    path: format!("requirements[{i}]"),
                    message: format!(
                        "Requirement '{}' has no scenarios. Each requirement MUST include at least one #### Scenario: block.",
                        req.text
                    ),
                });
            }
        }
    }

    SpecValidationReport {
        valid: !issues.iter().any(|i| i.level == "ERROR"),
        issues,
    }
}

/// Check if stderr is a TTY.
fn atty_is_tty() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stderr())
}
