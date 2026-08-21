use serde::{Deserialize, Serialize};
use validator::Validate;

use super::Requirement;

/// The kind of operation a delta performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DeltaOperation {
    Added,
    Modified,
    Removed,
    Renamed,
}

/// Optional rename descriptor inside a delta.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RenameDescriptor {
    pub from: String,
    pub to: String,
}

/// A single delta entry inside a change.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Delta {
    #[validate(length(min = 1, message = "Spec name cannot be empty"))]
    pub spec: String,

    pub operation: DeltaOperation,

    #[validate(length(min = 1, message = "Delta description cannot be empty"))]
    pub description: String,

    #[serde(default)]
    #[validate(nested)]
    pub requirement: Option<Requirement>,

    #[serde(default)]
    pub requirements: Vec<Requirement>,

    #[serde(default)]
    #[validate(nested)]
    pub rename: Option<RenameDescriptor>,
}

/// Metadata block attached to a change file.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ChangeMetadata {
    #[serde(default = "default_version")]
    pub version: String,

    /// Must always be `"speckit-change"` when present.
    pub format: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// Custom validator: ensure deltas vec is non-empty.
fn require_non_empty_deltas(deltas: &[Delta]) -> Result<(), validator::ValidationError> {
    if deltas.is_empty() {
        Err(validator::ValidationError::new("non_empty"))
    } else {
        Ok(())
    }
}

/// Top-level change document.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[validate(length(min = 1, message = "Change name cannot be empty"))]
    pub name: String,

    #[validate(length(min = 50, max = 1000, message = "Why section length out of bounds"))]
    pub why: String,

    #[validate(length(min = 1, message = "What Changes section cannot be empty"))]
    pub what_changes: String,

    #[validate(custom(function = "require_non_empty_deltas"))]
    pub deltas: Vec<Delta>,

    #[serde(default)]
    #[validate(nested)]
    pub metadata: Option<ChangeMetadata>,
}
