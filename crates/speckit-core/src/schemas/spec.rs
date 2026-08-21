use serde::{Deserialize, Serialize};
use validator::Validate;

use super::Requirement;

/// Custom validator: ensure the requirements vec is non-empty.
fn require_non_empty_requirements(reqs: &[Requirement]) -> Result<(), validator::ValidationError> {
    if reqs.is_empty() {
        Err(validator::ValidationError::new("non_empty"))
    } else {
        Ok(())
    }
}

/// Metadata block attached to a spec file.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SpecMetadata {
    #[serde(default = "default_version")]
    pub version: String,

    /// Must always be `"speckit"` when present.
    pub format: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// Top-level specification document.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    #[validate(length(min = 1, message = "Spec name cannot be empty"))]
    pub name: String,

    #[validate(length(min = 1, message = "Purpose section cannot be empty"))]
    pub overview: String,

    #[validate(custom(function = "require_non_empty_requirements"))]
    pub requirements: Vec<Requirement>,

    #[serde(default)]
    #[validate(nested)]
    pub metadata: Option<SpecMetadata>,
}
