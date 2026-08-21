use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Metadata filename for changes.
pub const METADATA_FILENAME: &str = ".speckit.yaml";

/// Change metadata stored in .speckit.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeMetadata {
    /// Schema name used for this change.
    #[serde(default)]
    pub schema: Option<String>,

    /// Whether to skip spec validation.
    #[serde(default, rename = "skip_specs")]
    pub skip_specs: bool,

    /// Capabilities to retire after archive.
    #[serde(default, rename = "retire_capabilities")]
    pub retire_capabilities: Option<Vec<String>>,
}

/// Read the skip_specs marker from change metadata.
pub fn read_skip_specs_marker(change_dir: &Path) -> Result<bool> {
    let metadata_path = change_dir.join(METADATA_FILENAME);
    if !metadata_path.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&metadata_path)?;
    let metadata: ChangeMetadata = serde_yaml::from_str(&content)?;
    Ok(metadata.skip_specs)
}

/// Read the retire_capabilities marker from change metadata.
pub fn read_retire_capabilities_marker(change_dir: &Path) -> Result<Option<Vec<String>>> {
    let metadata_path = change_dir.join(METADATA_FILENAME);
    if !metadata_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&metadata_path)?;
    let metadata: ChangeMetadata = serde_yaml::from_str(&content)?;
    Ok(metadata.retire_capabilities)
}

/// Resolve the schema for a change from its metadata.
pub fn resolve_schema_for_change(change_dir: &Path) -> Result<Option<String>> {
    let metadata_path = change_dir.join(METADATA_FILENAME);
    if !metadata_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&metadata_path)?;
    let metadata: ChangeMetadata = serde_yaml::from_str(&content)?;
    Ok(metadata.schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_read_skip_specs_marker_no_file() {
        let dir = tempdir().unwrap();
        let result = read_skip_specs_marker(dir.path()).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_read_skip_specs_marker_with_file() {
        let dir = tempdir().unwrap();
        let metadata = ChangeMetadata {
            schema: None,
            skip_specs: true,
            retire_capabilities: None,
        };
        let content = serde_yaml::to_string(&metadata).unwrap();
        fs::write(dir.path().join(METADATA_FILENAME), content).unwrap();

        let result = read_skip_specs_marker(dir.path()).unwrap();
        assert!(result);
    }
}
