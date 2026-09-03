//! Status Command
//!
//! Displays artifact completion status for a change.

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::shared::{
    ArtifactStatus, DEFAULT_SCHEMA, get_available_changes, print_json, validate_change_exists,
};
use speckit_core::artifact_graph::{
    LoadChangeContextOptions, format_change_status, load_change_context,
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
    /// Artifact IDs required before the implementation phase can start.
    #[serde(rename = "implementRequires")]
    pub implement_requires: Vec<String>,
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

    // Validate schema exists, then resolve the schema definition for the
    // `implement_requires` field on the status output.
    super::shared::validate_schema_exists(&schema_name, &project_root)?;
    let schema_definition = speckit_core::artifact_graph::resolve_schema(
        &schema_name,
        Some(std::path::Path::new(&project_root)),
    )?;

    // Build the change directory used as a fallback context root for the
    // artifact graph loader.
    let change_dir = std::path::Path::new(&project_root)
        .join("speckit")
        .join("changes")
        .join(&change_name);

    // Use the artifact graph rather than a fixed four-file probe. This preserves
    // custom-schema artifacts, concrete glob matches, dependency state, and
    // skip_specs semantics in the agent-facing JSON contract.
    let context = load_change_context(
        Path::new(&project_root),
        &change_name,
        options.schema.as_deref(),
        LoadChangeContextOptions {
            change_dir: Some(change_dir.clone()),
        },
    )
    .map_err(|error| anyhow::anyhow!("Unable to load change status: {error}"))?;
    let status = format_change_status(&context);

    let output = ChangeStatusOutput {
        change_name: status.change_name,
        schema_name: status.schema_name,
        change_root: Some(status.change_root.to_string_lossy().replace('\\', "/")),
        artifacts: status
            .artifacts
            .into_iter()
            .map(|artifact| ArtifactStatusEntry {
                id: artifact.id,
                status: serde_json::to_value(artifact.status)
                    .expect("artifact status is serializable")
                    .as_str()
                    .unwrap_or("blocked")
                    .to_string(),
                missing_deps: artifact.missing_deps,
            })
            .collect(),
        is_planning_complete: status.is_planning_complete,
        implement_requires: schema_definition
            .implement
            .map(|phase| phase.requires)
            .unwrap_or_default(),
        root: Some(serde_json::json!({
            "path": project_root.replace('\\', "/"),
            "source": if options.store.is_some() { "store" } else { "nearest" },
            "storeId": options.store,
        })),
    };

    if options.json {
        let mut payload = serde_json::to_value(&output)?;
        let object = payload.as_object_mut().expect("status output is an object");
        object.insert(
            "planningHome".to_string(),
            serde_json::json!({
                "kind": "repo",
                "root": project_root.replace('\\', "/"),
                "changesDir": Path::new(&project_root).join("speckit").join("changes").to_string_lossy().replace('\\', "/"),
                "defaultSchema": output.schema_name.clone(),
            }),
        );
        object.insert(
            "artifactPaths".to_string(),
            serde_json::to_value(status.artifact_paths)?,
        );
        object.insert(
            "actionContext".to_string(),
            serde_json::json!({
                "projectRoot": project_root.replace('\\', "/"),
                "artifactIds": output.artifacts.iter().map(|artifact| artifact.id.clone()).collect::<Vec<_>>(),
            }),
        );
        print_json(&payload);
        return Ok(());
    }

    print_status_text(&output);
    Ok(())
}

/// Detect artifact status from the filesystem.
#[allow(dead_code)]
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
#[allow(dead_code)]
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
