use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Metadata filename for changes
pub const METADATA_FILENAME: &str = ".speckit.yaml";

/// Change metadata stored in .speckit.yaml.
///
/// `retire_capabilities` is a boolean. A list
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
/// - any other string → error (no silent defaults)
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

/// Read change metadata and validate its declared schema against the project.
pub fn read_change_metadata(
    change_dir: &Path,
    project_root: &Path,
) -> Result<Option<ChangeMetadata>> {
    let metadata_path = change_dir.join(METADATA_FILENAME);
    if !metadata_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&metadata_path)
        .with_context(|| format!("Failed to read {}", metadata_path.display()))?;
    let metadata: ChangeMetadata = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", metadata_path.display()))?;

    if let Some(schema_name) = metadata.schema.as_deref() {
        if let Err(error) = crate::artifact_graph::resolve_schema(schema_name, Some(project_root)) {
            let mut message = format!(
                "Schema '{schema_name}' declared in {} could not be resolved: {error}",
                metadata_path.display()
            );

            let available = crate::artifact_graph::list_schemas_with_info(Some(project_root))
                .into_iter()
                .map(|schema| {
                    (
                        schema.name,
                        matches!(schema.source, crate::artifact_graph::SchemaSource::Package),
                    )
                })
                .collect::<Vec<_>>();
            let suggestion = crate::project_config::suggest_schemas(schema_name, &available);
            if let Some(candidate) = suggestion.lines().find_map(|line| {
                line.trim()
                    .strip_prefix("- ")
                    .and_then(|value| value.split_whitespace().next())
            }) {
                message.push_str(&format!("\nDid you mean: {candidate}"));
            }

            anyhow::bail!(message);
        }
    }

    Ok(Some(metadata))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_schema(project_root: &Path, name: &str) {
        let schema_dir = project_root.join("speckit").join("schemas").join(name);
        fs::create_dir_all(&schema_dir).unwrap();
        fs::write(
            schema_dir.join("schema.yaml"),
            format!(
                "name: {name}\nversion: 1\nartifacts:\n  - id: plan\n    generates: plan.md\n    description: Plan\n    template: plan.md\n"
            ),
        )
        .unwrap();
    }

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

    #[test]
    fn read_change_metadata_returns_none_when_missing() {
        let dir = tempdir().unwrap();
        assert!(
            read_change_metadata(dir.path(), dir.path())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn read_change_metadata_accepts_a_resolvable_schema() {
        let project = tempdir().unwrap();
        write_schema(project.path(), "custom");
        let change_dir = project.path().join("speckit/changes/example");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(
            change_dir.join(METADATA_FILENAME),
            "schema: custom\nskip_specs: false\n",
        )
        .unwrap();

        let metadata = read_change_metadata(&change_dir, project.path())
            .unwrap()
            .unwrap();
        assert_eq!(metadata.schema.as_deref(), Some("custom"));
    }

    #[test]
    fn read_change_metadata_rejects_unknown_schema_with_suggestion() {
        let project = tempdir().unwrap();
        write_schema(project.path(), "spec-driven");
        let change_dir = project.path().join("speckit/changes/example");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(change_dir.join(METADATA_FILENAME), "schema: spec-drivn\n").unwrap();

        let error = read_change_metadata(&change_dir, project.path())
            .unwrap_err()
            .to_string();
        assert!(error.contains("spec-drivn"));
        assert!(error.contains("Did you mean: spec-driven"), "{error}");
    }

    #[test]
    fn read_change_metadata_rejects_nonexistent_schema_by_name() {
        let project = tempdir().unwrap();
        let change_dir = project.path().join("speckit/changes/example");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(
            change_dir.join(METADATA_FILENAME),
            "schema: definitely-not-installed\n",
        )
        .unwrap();

        let error = read_change_metadata(&change_dir, project.path())
            .unwrap_err()
            .to_string();
        assert!(error.contains("definitely-not-installed"), "{error}");
    }
}
