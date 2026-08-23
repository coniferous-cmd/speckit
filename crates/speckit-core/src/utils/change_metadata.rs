use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Metadata filename for changes.
pub const METADATA_FILENAME: &str = ".speckit.yaml";

/// Change metadata stored in .speckit.yaml.
///
/// `retire_capabilities` is a boolean: when
/// `true`, the entire change is treated as retiring a capability, and the
/// corresponding spec is removed by archive.  A list value is tolerated as
/// a legacy alias for `true` so older `.speckit.yaml` files keep working.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeMetadata {
    /// Schema name used for this change.
    #[serde(default)]
    pub schema: Option<String>,

    /// Whether to skip spec validation.
    #[serde(default, rename = "skip_specs")]
    pub skip_specs: bool,

    /// Mark this change as retiring a capability.
    #[serde(
        default,
        rename = "retire_capabilities",
        deserialize_with = "deserialize_retire_capabilities"
    )]
    pub retire_capabilities: bool,
}

/// Backwards-compatible deserialization for `retire_capabilities`.
///
/// Accepts:
/// - a plain boolean (`true` / `false`)
/// - a list of capability names (any non-empty list means "retire")
/// - missing / null (defaults to `false`)
fn deserialize_retire_capabilities<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_yaml::Value::deserialize(deserializer)?;
    match value {
        serde_yaml::Value::Bool(b) => Ok(b),
        serde_yaml::Value::Sequence(seq) => Ok(!seq.is_empty()),
        serde_yaml::Value::Null => Ok(false),
        serde_yaml::Value::String(s) => {
            let lowered = s.to_lowercase();
            match lowered.as_str() {
                "true" | "yes" | "1" => Ok(true),
                "false" | "no" | "0" | "" => Ok(false),
                _ => Err(D::Error::custom(format!(
                    "retire_capabilities: unrecognised string value '{s}'"
                ))),
            }
        }
        other => Err(D::Error::custom(format!(
            "retire_capabilities: expected boolean or list, got {other:?}"
        ))),
    }
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
///
/// Returns `Ok(true)` when the change is marked for retirement and
/// `Ok(false)` otherwise.  See [`ChangeMetadata`] for the accepted shapes.
pub fn read_retire_capabilities_marker(change_dir: &Path) -> Result<bool> {
    let metadata_path = change_dir.join(METADATA_FILENAME);
    if !metadata_path.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&metadata_path)
        .with_context(|| format!("Failed to read {}", metadata_path.display()))?;
    let metadata: ChangeMetadata = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", metadata_path.display()))?;
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
            retire_capabilities: false,
        };
        let content = serde_yaml::to_string(&metadata).unwrap();
        fs::write(dir.path().join(METADATA_FILENAME), content).unwrap();

        let result = read_skip_specs_marker(dir.path()).unwrap();
        assert!(result);
    }

    #[test]
    fn test_retire_capabilities_boolean_true() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(METADATA_FILENAME),
            "retire_capabilities: true\n",
        )
        .unwrap();
        assert!(read_retire_capabilities_marker(dir.path()).unwrap());
    }

    #[test]
    fn test_retire_capabilities_boolean_false() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(METADATA_FILENAME),
            "retire_capabilities: false\n",
        )
        .unwrap();
        assert!(!read_retire_capabilities_marker(dir.path()).unwrap());
    }

    #[test]
    fn test_retire_capabilities_missing() {
        let dir = tempdir().unwrap();
        assert!(!read_retire_capabilities_marker(dir.path()).unwrap());
    }

    #[test]
    fn test_retire_capabilities_legacy_list_is_true() {
        // Older writers used a list of capability ids. We
        // still treat any non-empty list as "retire" for backwards
        // compatibility.
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(METADATA_FILENAME),
            "retire_capabilities:\n  - auth\n  - session\n",
        )
        .unwrap();
        assert!(read_retire_capabilities_marker(dir.path()).unwrap());
    }

    #[test]
    fn test_retire_capabilities_empty_list_is_false() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(METADATA_FILENAME),
            "retire_capabilities: []\n",
        )
        .unwrap();
        assert!(!read_retire_capabilities_marker(dir.path()).unwrap());
    }
}
