use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Metadata filename for changes
pub const METADATA_FILENAME: &str = ".speckit.yaml";

/// Change metadata stored in .speckit.yaml.
///
/// `retire_capabilities` is a boolean to mirror OpenSpec's schema.  A list
/// value is tolerated as a legacy alias for `true` so older `.speckit.yaml`
/// files keep working.  An unrecognized string value produces an error rather
/// than silently defaulting to `false`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeMetadata {
    /// Schema name used for this change
    #[serde(default)]
    pub schema: Option<String>,

    /// Whether to skip spec validation
    #[serde(default, rename = "skip_specs")]
    pub skip_specs: bool,

    /// Mark this change as retiring a capability (deletes corresponding specs).
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
/// - the string `"true"` / `"yes"` / `"1"` (case-insensitive) → `true`
/// - the string `"false"` / `"no"` / `"0"` / `""` → `false`
/// - any other string → error (per OpenSpec: no silent defaults)
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

    let content = std::fs::read_to_string(&metadata_path)
        .with_context(|| format!("Failed to read {}", metadata_path.display()))?;
    let metadata: ChangeMetadata = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", metadata_path.display()))?;
    Ok(metadata.skip_specs)
}

/// Read the retire_capabilities marker from change metadata.
///
/// Returns `Ok(true)` when the change is marked for retirement, `Ok(false)`
/// when it is not.  Invalid metadata (malformed YAML, wrong type for
/// `retire_capabilities`) propagates as an error — the archive is blocked
/// rather than silently treating an invalid marker as unset.
///
/// See [`ChangeMetadata`] for the accepted shapes including the legacy
/// `Vec<String>` form.
pub fn read_retire_capabilities_marker(change_dir: &Path) -> Result<bool> {
    let metadata_path = change_dir.join(METADATA_FILENAME);
    if !metadata_path.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&metadata_path)
        .with_context(|| format!("Failed to read {}", metadata_path.display()))?;
    let metadata: ChangeMetadata = serde_yaml::from_str(&content).map_err(|error| {
        anyhow::anyhow!(
            "Failed to parse {}: unrecognised retire_capabilities metadata: {error}",
            metadata_path.display()
        )
    })?;
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

/// Read change metadata from a directory.
pub fn read_change_metadata(change_dir: &Path) -> Result<Option<ChangeMetadata>> {
    let metadata_path = change_dir.join(METADATA_FILENAME);
    if !metadata_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&metadata_path)
        .with_context(|| format!("Failed to read {}", metadata_path.display()))?;
    let metadata: ChangeMetadata = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", metadata_path.display()))?;
    Ok(Some(metadata))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // P0-6: Five retire_capabilities test cases (new, modified, removed, retired, conflict)

    /// Case: retire_capabilities = true → mark is declared
    #[test]
    fn retire_capabilities_boolean_true() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(METADATA_FILENAME),
            "retire_capabilities: true\n",
        )
        .unwrap();
        assert!(read_retire_capabilities_marker(dir.path()).unwrap());
    }

    /// Case: retire_capabilities = false → mark not declared
    #[test]
    fn retire_capabilities_boolean_false() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(METADATA_FILENAME),
            "retire_capabilities: false\n",
        )
        .unwrap();
        assert!(!read_retire_capabilities_marker(dir.path()).unwrap());
    }

    /// Case: missing file → mark not declared (no error)
    #[test]
    fn retire_capabilities_missing_file() {
        let dir = tempdir().unwrap();
        let result = read_retire_capabilities_marker(dir.path()).unwrap();
        assert!(!result);
    }

    /// Case: legacy list form → treated as true (non-empty list)
    #[test]
    fn retire_capabilities_legacy_list_is_true() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(METADATA_FILENAME),
            "retire_capabilities:\n  - auth\n  - session\n",
        )
        .unwrap();
        assert!(read_retire_capabilities_marker(dir.path()).unwrap());
    }

    /// Case: empty list → treated as false (no retirement)
    #[test]
    fn retire_capabilities_empty_list_is_false() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(METADATA_FILENAME),
            "retire_capabilities: []\n",
        )
        .unwrap();
        assert!(!read_retire_capabilities_marker(dir.path()).unwrap());
    }

    /// Case: invalid string value → error (not silently false)
    #[test]
    fn retire_capabilities_invalid_string_is_error() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(METADATA_FILENAME),
            "retire_capabilities: unknown\n",
        )
        .unwrap();
        let result = read_retire_capabilities_marker(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unrecognised") || err.contains("unknown"));
    }

    /// Case: invalid type (integer) → error
    #[test]
    fn retire_capabilities_invalid_type_is_error() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(METADATA_FILENAME),
            "retire_capabilities: 123\n",
        )
        .unwrap();
        let result = read_retire_capabilities_marker(dir.path());
        assert!(result.is_err());
    }
}
