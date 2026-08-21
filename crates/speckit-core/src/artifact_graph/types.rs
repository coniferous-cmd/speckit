use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A single artifact definition from a schema YAML file.
///
/// Each artifact describes one output file (or glob pattern of files) that the
/// workflow produces, along with its dependency requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    /// Unique identifier for this artifact (e.g., "proposal", "specs", "design").
    pub id: String,
    /// Relative path (or glob pattern) for the artifact's output file(s).
    pub generates: String,
    /// Human-readable description of what this artifact contains.
    pub description: String,
    /// Relative path to the template file within the schema's templates directory.
    pub template: String,
    /// Optional guidance for creating this artifact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    /// IDs of artifacts that must be completed before this one can be created.
    #[serde(default)]
    pub requires: Vec<String>,
}

/// Apply phase configuration for schema-aware apply instructions.
///
/// Defines which artifacts must exist before the apply (implementation) phase
/// is available, and optionally tracks progress via a checkbox file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPhase {
    /// Artifact IDs that must exist before apply is available.
    pub requires: Vec<String>,
    /// Path to a file with checkboxes for progress tracking (relative to
    /// change dir), or `None` if no tracking is needed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracks: Option<String>,
    /// Custom guidance for the apply phase.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
}

/// The full structure of a schema YAML file.
///
/// A schema defines the artifact workflow: the ordered set of artifacts that
/// a change must produce, their dependencies, and optionally an apply phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaYaml {
    /// Human-readable schema name (e.g., "spec-driven").
    pub name: String,
    /// Schema version (positive integer).
    pub version: u32,
    /// Optional description of the schema's purpose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Ordered list of artifact definitions.
    pub artifacts: Vec<Artifact>,
    /// Optional apply phase configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply: Option<ApplyPhase>,
}

/// A set of completed artifact IDs, used for tracking change progress.
///
/// Maps artifact IDs to `true` when their generated files exist on disk.
pub type CompletedSet = HashSet<String>;

/// Maps artifact IDs to a list of their unmet dependency IDs.
///
/// Only contains entries for artifacts that are currently blocked.
pub type BlockedArtifacts = HashMap<String, Vec<String>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_deserialization_roundtrip() {
        let artifact = Artifact {
            id: "proposal".to_string(),
            generates: "proposal.md".to_string(),
            description: "High-level proposal".to_string(),
            template: "proposal.md".to_string(),
            instruction: Some("Write a proposal".to_string()),
            requires: vec![],
        };
        let json = serde_json::to_string(&artifact).unwrap();
        let deserialized: Artifact = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "proposal");
        assert_eq!(deserialized.requires.len(), 0);
    }

    #[test]
    fn apply_phase_deserialization_with_optionals() {
        let json = r#"{"requires": ["proposal", "specs"]}"#;
        let phase: ApplyPhase = serde_json::from_str(json).unwrap();
        assert_eq!(phase.requires.len(), 2);
        assert!(phase.tracks.is_none());
        assert!(phase.instruction.is_none());
    }

    #[test]
    fn schema_yaml_deserialization() {
        let json = r#"{
            "name": "spec-driven",
            "version": 1,
            "artifacts": [
                {
                    "id": "proposal",
                    "generates": "proposal.md",
                    "description": "Proposal",
                    "template": "proposal.md"
                }
            ]
        }"#;
        let schema: SchemaYaml = serde_json::from_str(json).unwrap();
        assert_eq!(schema.name, "spec-driven");
        assert_eq!(schema.version, 1);
        assert_eq!(schema.artifacts.len(), 1);
        assert!(schema.apply.is_none());
    }

    #[test]
    fn artifact_requires_defaults_to_empty() {
        let json = r#"{
            "id": "test",
            "generates": "test.md",
            "description": "Test",
            "template": "test.md"
        }"#;
        let artifact: Artifact = serde_json::from_str(json).unwrap();
        assert!(artifact.requires.is_empty());
    }
}
