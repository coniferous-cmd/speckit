//! New Change Command
//!
//! Creates a new change directory with optional description and schema.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::shared::{DEFAULT_SCHEMA, print_json, status_from_error, validate_schema_exists};
use crate::shared_output::StoreDiagnostic;

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

/// Options for the new change command.
#[derive(Debug, Clone)]
pub struct NewChangeOptions {
    pub description: Option<String>,
    pub goal: Option<String>,
    pub schema: Option<String>,
    pub store: Option<String>,
    pub json: bool,
}

/// Output payload for a newly created change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewChangeOutput {
    pub change: Option<ChangeInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Vec<ChangeCommandStatus>>,
}

/// Information about the created change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeInfo {
    pub id: String,
    pub path: String,
    pub metadata_path: String,
    pub schema: String,
}

/// A status entry for change commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeCommandStatus {
    pub severity: String,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

// -----------------------------------------------------------------------------
// Command Implementation
// -----------------------------------------------------------------------------

/// Validate a change name for creation (kebab-case).
fn validate_change_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        anyhow::bail!("Change name cannot be empty");
    }
    if name.starts_with('-') || name.ends_with('-') {
        anyhow::bail!("Change name cannot start or end with a hyphen");
    }
    if name.contains("--") {
        anyhow::bail!("Change name cannot contain consecutive hyphens");
    }
    // Check for valid kebab-case characters
    for ch in name.chars() {
        if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() && ch != '-' {
            anyhow::bail!(
                "Change name must be kebab-case (lowercase letters, digits, hyphens). Found: '{ch}'"
            );
        }
    }
    Ok(())
}

/// Execute the new change command.
pub async fn new_change_command(
    name: Option<&str>,
    options: NewChangeOptions,
) -> anyhow::Result<()> {
    let change_name = match name {
        Some(n) => n.to_string(),
        None => {
            if options.json {
                print_json(&NewChangeOutput {
                    change: None,
                    status: Some(vec![ChangeCommandStatus {
                        severity: "error".to_string(),
                        code: "missing_argument".to_string(),
                        message: "Missing required argument <name>".to_string(),
                        fix: None,
                    }]),
                });
                std::process::exit(1);
            }
            anyhow::bail!("Missing required argument <name>");
        }
    };

    // Validate the change name
    if let Err(e) = validate_change_name(&change_name) {
        if options.json {
            print_json(&NewChangeOutput {
                change: None,
                status: Some(vec![ChangeCommandStatus {
                    severity: "error".to_string(),
                    code: "invalid_name".to_string(),
                    message: e.to_string(),
                    fix: None,
                }]),
            });
            std::process::exit(1);
        }
        return Err(e);
    }

    let project_root = std::env::current_dir()?.to_string_lossy().to_string();

    // Validate schema if provided
    if let Some(ref schema) = options.schema {
        validate_schema_exists(schema, &project_root)?;
    }

    let resolved_schema = options
        .schema
        .clone()
        .unwrap_or_else(|| DEFAULT_SCHEMA.to_string());

    let changes_dir = Path::new(&project_root).join("speckit").join("changes");
    let change_dir = changes_dir.join(&change_name);

    // Check if change already exists
    if change_dir.exists() {
        let msg = format!(
            "Change '{change_name}' already exists at {}",
            change_dir.display()
        );
        if options.json {
            print_json(&NewChangeOutput {
                change: None,
                status: Some(vec![ChangeCommandStatus {
                    severity: "error".to_string(),
                    code: "change_exists".to_string(),
                    message: msg,
                    fix: Some(format!("Use 'speckit show {change_name}' to view it.")),
                }]),
            });
            std::process::exit(1);
        }
        anyhow::bail!("{msg}");
    }

    // Create the changes directory if needed
    tokio::fs::create_dir_all(&changes_dir).await?;

    // Create the change directory
    tokio::fs::create_dir_all(&change_dir).await?;

    // Write .speckit.yaml metadata
    let metadata = serde_yaml::to_string(&serde_yaml::Value::Mapping({
        let mut map = serde_yaml::Mapping::new();
        map.insert(
            serde_yaml::Value::String("schema".to_string()),
            serde_yaml::Value::String(resolved_schema.clone()),
        );
        if let Some(ref goal) = options.goal {
            map.insert(
                serde_yaml::Value::String("goal".to_string()),
                serde_yaml::Value::String(goal.clone()),
            );
        }
        map
    }))?;

    let metadata_path = change_dir.join(".speckit.yaml");
    tokio::fs::write(&metadata_path, metadata).await?;

    // Write description to README.md if provided
    if let Some(ref description) = options.description {
        let readme_path = change_dir.join("README.md");
        let readme_content = format!("# {change_name}\n\n{description}\n");
        tokio::fs::write(&readme_path, readme_content).await?;
    }

    let payload = NewChangeOutput {
        change: Some(ChangeInfo {
            id: change_name.clone(),
            path: change_dir.to_string_lossy().to_string(),
            metadata_path: metadata_path.to_string_lossy().to_string(),
            schema: resolved_schema,
        }),
        status: None,
    };

    if options.json {
        print_json(&payload);
        return Ok(());
    }

    let location = format!("{}/", change_dir.display());
    println!("Created change '{change_name}' at {location}");
    if let Some(ref info) = payload.change {
        println!("Schema: {}", info.schema);
    }
    println!("Next: speckit status --change {change_name}");
    Ok(())
}
