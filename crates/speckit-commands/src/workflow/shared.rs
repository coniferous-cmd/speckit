//! Shared types and utilities for artifact workflow commands.
//!
//! This module contains types, constants, and validation helpers used across
//! multiple artifact workflow commands.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::shared_gather::ReferenceIndexEntry;

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

/// A status entry for change/workflow commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeCommandStatus {
    pub severity: String,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

/// A single task item from a tasks.md file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub description: String,
    pub done: bool,
}

/// Instructions for applying (implementing) a change's tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyInstructions {
    pub change_name: String,
    pub change_dir: String,
    pub schema_name: String,
    pub context_files: std::collections::HashMap<String, Vec<String>>,
    pub progress: ApplyProgress,
    pub tasks: Vec<TaskItem>,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_artifacts: Option<Vec<String>>,
    pub instruction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<Vec<ReferenceIndexEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_guidance: Option<Vec<String>>,
}

/// Progress tracking for apply instructions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyProgress {
    pub total: usize,
    pub complete: usize,
    pub remaining: usize,
}

/// Instructions for archiving a change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveInstructions {
    pub change_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_guidance: Option<Vec<String>>,
}

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

/// The default workflow schema name.
pub const DEFAULT_SCHEMA: &str = "spec-driven";

// -----------------------------------------------------------------------------
// Utility Functions
// -----------------------------------------------------------------------------

/// Print a value as pretty-printed JSON to stdout.
pub fn print_json(payload: &impl Serialize) {
    crate::shared_output::print_json(payload);
}

/// Convert an error into a `ChangeCommandStatus`.
pub fn status_from_error(error: &anyhow::Error) -> ChangeCommandStatus {
    ChangeCommandStatus {
        severity: "error".to_string(),
        code: "change_error".to_string(),
        message: error.to_string(),
        target: None,
        fix: None,
    }
}

/// Check if color output is disabled.
pub fn is_color_disabled() -> bool {
    std::env::var("NO_COLOR").map_or(false, |v| v == "1" || v == "true")
}

/// Status display: artifact statuses used by status and instructions commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactStatus {
    Done,
    Skipped,
    Ready,
    Blocked,
}

impl ArtifactStatus {
    /// Get a colored status indicator string.
    pub fn indicator(&self) -> &'static str {
        match self {
            ArtifactStatus::Done => "[x]",
            ArtifactStatus::Skipped => "[~]",
            ArtifactStatus::Ready => "[ ]",
            ArtifactStatus::Blocked => "[-]",
        }
    }

    /// Get the ANSI color code for this status (empty string if color is disabled).
    pub fn color_code(&self) -> &'static str {
        if is_color_disabled() {
            return "";
        }
        match self {
            ArtifactStatus::Done => "\x1b[32m",    // green
            ArtifactStatus::Skipped => "\x1b[90m", // dim/gray
            ArtifactStatus::Ready => "\x1b[33m",   // yellow
            ArtifactStatus::Blocked => "\x1b[31m", // red
        }
    }

    pub fn color_reset() -> &'static str {
        if is_color_disabled() { "" } else { "\x1b[0m" }
    }
}

/// Returns the list of available change directory names under speckit/changes/.
/// Excludes the archive directory and hidden directories.
pub async fn get_available_changes(
    project_root: &str,
    changes_dir: Option<&str>,
) -> anyhow::Result<Vec<String>> {
    let changes_path = changes_dir
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(project_root)
                .join("speckit")
                .join("changes")
        });

    let mut entries = Vec::new();
    let mut read_dir = match tokio::fs::read_dir(&changes_path).await {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
        Err(e) => return Err(e.into()),
    };

    while let Some(entry) = read_dir.next_entry().await? {
        let file_type = entry.file_type().await?;
        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name != "archive" && !name.starts_with('.') {
                entries.push(name);
            }
        }
    }

    entries.sort();
    Ok(entries)
}

/// Validate a change name used to look up an existing change directory.
fn validate_change_lookup_name(change_name: &str) -> Option<&'static str> {
    if change_name == "." || change_name == ".." {
        return Some("Change name cannot be a relative path segment");
    }
    if change_name.contains('/') || change_name.contains('\\') {
        return Some("Change name cannot contain path separators");
    }
    if change_name.contains('\0') {
        return Some("Change name cannot contain null characters");
    }
    if change_name.starts_with('.') {
        return Some("Change name cannot start with a dot");
    }
    if change_name == "archive" {
        return Some("'archive' is reserved for archived changes");
    }
    None
}

/// Validates that a change exists and returns its name, or an error with
/// the list of available changes.
pub async fn validate_change_exists(
    change_name: Option<&str>,
    project_root: &str,
    changes_dir: Option<&str>,
    new_change_hint: Option<&str>,
) -> anyhow::Result<String> {
    let hint = new_change_hint.unwrap_or("speckit new change <name>");

    let name = match change_name {
        Some(n) => n,
        None => {
            let available = get_available_changes(project_root, changes_dir).await?;
            if available.is_empty() {
                anyhow::bail!("No changes found. Create one with: {hint}");
            }
            anyhow::bail!(
                "Missing required option --change. Available changes:\n  {}",
                available.join("\n  ")
            );
        }
    };

    if let Some(err) = validate_change_lookup_name(name) {
        anyhow::bail!("Invalid change name '{name}': {err}");
    }

    let change_path = changes_dir
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(project_root)
                .join("speckit")
                .join("changes")
        })
        .join(name);

    let exists = tokio::fs::metadata(&change_path)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);

    if !exists {
        let available = get_available_changes(project_root, changes_dir).await?;
        if available.is_empty() {
            anyhow::bail!("Change '{name}' not found. No changes exist. Create one with: {hint}");
        }
        anyhow::bail!(
            "Change '{name}' not found. Available changes:\n  {}",
            available.join("\n  ")
        );
    }

    Ok(name.to_string())
}

/// Validates that a schema exists and returns its name.
pub fn validate_schema_exists(schema_name: &str, project_root: &str) -> anyhow::Result<String> {
    if get_schema_dir(schema_name, project_root).is_some() {
        return Ok(schema_name.to_string());
    }

    let available = list_schemas(project_root);
    anyhow::bail!(
        "Schema '{schema_name}' not found. Available schemas:\n  {}",
        available.join("\n  ")
    );
}

/// List all available schema names (project, user, and package).
pub fn list_schemas(project_root: &str) -> Vec<String> {
    let mut schemas = Vec::new();
    let mut seen = std::collections::HashSet::new();

    collect_schema_names(
        &Path::new(project_root).join("speckit").join("schemas"),
        &mut schemas,
        &mut seen,
    );

    if let Some(config_dir) = dirs::config_dir() {
        collect_schema_names(
            &config_dir.join("speckit").join("schemas"),
            &mut schemas,
            &mut seen,
        );
    }

    if let Some(package_dir) = get_package_schemas_dir() {
        collect_schema_names(&package_dir, &mut schemas, &mut seen);
    }

    schemas.sort();
    schemas
}

/// Get the schema directory path for a given schema name.
pub fn get_schema_dir(schema_name: &str, project_root: &str) -> Option<String> {
    let project_dir = Path::new(project_root)
        .join("speckit")
        .join("schemas")
        .join(schema_name);
    if project_dir.join("schema.yaml").exists() {
        return Some(project_dir.to_string_lossy().to_string());
    }

    if let Some(config_dir) = dirs::config_dir() {
        let user_dir = config_dir.join("speckit").join("schemas").join(schema_name);
        if user_dir.join("schema.yaml").exists() {
            return Some(user_dir.to_string_lossy().to_string());
        }
    }

    let package_dir = get_package_schemas_dir()?.join(schema_name);
    package_dir
        .join("schema.yaml")
        .exists()
        .then(|| package_dir.to_string_lossy().to_string())
}

/// Locates schemas shipped with the CLI package.
///
/// Release archives and npm platform packages place `schemas/` next to the
/// native binary. Walking ancestors additionally supports `cargo run`, whose
/// executable lives below the repository root.
fn get_package_schemas_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("SPECKIT_SCHEMAS_DIR") {
        let path = PathBuf::from(dir);
        if path.is_dir() {
            return Some(path);
        }
    }

    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .skip(1)
        .map(|dir| dir.join("schemas"))
        .find(|candidate| candidate.is_dir())
}

fn collect_schema_names(
    dir: &Path,
    schemas: &mut Vec<String>,
    seen: &mut std::collections::HashSet<String>,
) {
    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                let schema_file = entry.path().join("schema.yaml");
                if schema_file.exists() && seen.insert(name.clone()) {
                    schemas.push(name);
                }
            }
        }
    }
}
