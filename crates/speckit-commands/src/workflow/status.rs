//! Status Command
//!
//! Displays artifact completion status for a change.

use serde::{Deserialize, Serialize};

use super::shared::{
    ArtifactStatus, DEFAULT_SCHEMA, get_available_changes, print_json, validate_change_exists,
};
use speckit_core::change_metadata::read_skip_specs_marker;

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

/// Options for the status command.
#[derive(Debug, Clone)]
pub struct StatusOptions {
    pub change: Option<String>,
    pub schema: Option<String>,
    pub store: Option<String>,
    pub json: bool,
}

/// Artifact status entry for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactStatusEntry {
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_deps: Option<Vec<String>>,
}

/// The full change status for output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeStatusOutput {
    pub change_name: String,
    pub schema_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_root: Option<String>,
    pub artifacts: Vec<ArtifactStatusEntry>,
    pub is_planning_complete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<serde_json::Value>,
}

// -----------------------------------------------------------------------------
// Command Implementation
// -----------------------------------------------------------------------------

/// Execute the status command.
pub async fn status_command(options: StatusOptions) -> anyhow::Result<()> {
    let project_root = super::resolve_project_root(options.store.as_deref()).await?;

    // Handle no-changes case gracefully
    if options.change.is_none() {
        let available = get_available_changes(&project_root, None).await?;
        if available.is_empty() {
            if options.json {
                print_json(&serde_json::json!({
                    "changes": [],
                    "message": "No active changes.",
                }));
                return Ok(());
            }
            println!("No active changes. Create one with: speckit new change <name>");
            return Ok(());
        }
        anyhow::bail!(
            "Missing required option --change. Available changes:\n  {}",
            available.join("\n  ")
        );
    }

    let change_name = validate_change_exists(
        options.change.as_deref(),
        &project_root,
        None,
        Some("speckit new change <name>"),
    )
    .await?;

    let schema_name = options
        .schema
        .clone()
        .unwrap_or_else(|| DEFAULT_SCHEMA.to_string());

    // Validate schema exists
    super::shared::validate_schema_exists(&schema_name, &project_root)?;

    // Build a minimal status from what we can determine from the filesystem
    let change_dir = std::path::Path::new(&project_root)
        .join("speckit")
        .join("changes")
        .join(&change_name);

    let artifacts = detect_artifact_status(&change_dir).await;
    let done_count = artifacts.iter().filter(|a| a.status == "done").count();
    let skipped_count = artifacts.iter().filter(|a| a.status == "skipped").count();
    let is_planning_complete =
        !artifacts.is_empty() && done_count + skipped_count == artifacts.len();

    let output = ChangeStatusOutput {
        change_name: change_name.clone(),
        schema_name,
        change_root: None,
        artifacts,
        is_planning_complete,
        root: None,
    };

    if options.json {
        print_json(&output);
        return Ok(());
    }

    print_status_text(&output);
    Ok(())
}

/// Detect artifact status from the filesystem.
async fn detect_artifact_status(change_dir: &std::path::Path) -> Vec<ArtifactStatusEntry> {
    let mut artifacts = Vec::new();

    let proposal = change_dir.join("proposal.md");
    let design = change_dir.join("design.md");
    let tasks = change_dir.join("tasks.md");

    artifacts.push(ArtifactStatusEntry {
        id: "proposal".to_string(),
        status: if proposal.exists() {
            "done".to_string()
        } else {
            "ready".to_string()
        },
        missing_deps: None,
    });

    let specs_skipped = read_skip_specs_marker(change_dir).unwrap_or(false);

    artifacts.push(ArtifactStatusEntry {
        id: "specs".to_string(),
        status: if specs_skipped {
            "skipped".to_string()
        } else if has_markdown_file_recursively(&change_dir.join("specs")) {
            "done".to_string()
        } else {
            "ready".to_string()
        },
        missing_deps: None,
    });

    artifacts.push(ArtifactStatusEntry {
        id: "design".to_string(),
        status: if design.exists() {
            "done".to_string()
        } else {
            "ready".to_string()
        },
        missing_deps: None,
    });

    artifacts.push(ArtifactStatusEntry {
        id: "tasks".to_string(),
        status: if tasks.exists() {
            "done".to_string()
        } else {
            "ready".to_string()
        },
        missing_deps: None,
    });

    artifacts
}

/// Print status in human-readable format.
fn print_status_text(status: &ChangeStatusOutput) {
    let skipped_count = status
        .artifacts
        .iter()
        .filter(|a| a.status == "skipped")
        .count();
    let total = status.artifacts.len() - skipped_count;
    let done_count = status
        .artifacts
        .iter()
        .filter(|a| a.status == "done")
        .count();

    println!("Change: {}", status.change_name);
    println!("Schema: {}", status.schema_name);
    if let Some(ref root) = status.change_root {
        println!("Change root: {root}");
    }

    let skipped_suffix = if skipped_count > 0 {
        format!(" ({skipped_count} skipped)")
    } else {
        String::new()
    };
    println!("Progress: {done_count}/{total} artifacts complete{skipped_suffix}");
    println!();

    for artifact in &status.artifacts {
        let status_enum = match artifact.status.as_str() {
            "done" => ArtifactStatus::Done,
            "skipped" => ArtifactStatus::Skipped,
            "blocked" => ArtifactStatus::Blocked,
            _ => ArtifactStatus::Ready,
        };
        let indicator = status_enum.indicator();
        let color = status_enum.color_code();
        let reset = ArtifactStatus::color_reset();
        let mut line = format!("{color}{indicator}{reset} {}", artifact.id);

        if artifact.status == "skipped" {
            line.push_str(&format!(
                "{color} (skipped: change declares skip_specs){reset}"
            ));
        }

        if artifact.status == "blocked" {
            if let Some(ref deps) = artifact.missing_deps {
                if !deps.is_empty() {
                    line.push_str(&format!("{color} (blocked by: {}){reset}", deps.join(", ")));
                }
            }
        }

        println!("{line}");
    }

    if status.is_planning_complete {
        println!();
        println!("\x1b[32mAll planning artifacts complete!\x1b[0m");
    }
}

/// Returns whether `dir` contains at least one Markdown file at any depth.
fn has_markdown_file_recursively(dir: &std::path::Path) -> bool {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    entries.flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            has_markdown_file_recursively(&path)
        } else {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn nested_spec_file_is_reported_as_done() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nested_specs = temp_dir.path().join("specs").join("device-type-service");
        fs::create_dir_all(&nested_specs).unwrap();
        fs::write(nested_specs.join("spec.md"), "content").unwrap();

        let artifacts = detect_artifact_status(temp_dir.path()).await;
        let specs = artifacts
            .iter()
            .find(|artifact| artifact.id == "specs")
            .unwrap();

        assert_eq!(specs.status, "done");
    }
}
