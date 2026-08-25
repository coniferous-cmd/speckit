use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use super::schema::{parse_schema, SchemaValidationError};
use super::types::SchemaYaml;

/// Error thrown when loading a schema from disk fails.
#[derive(Debug, Error)]
pub enum SchemaLoadError {
    #[error("Schema '{name}' not found. Available schemas: {available}")]
    NotFound { name: String, available: String },

    #[error("Failed to read schema at '{path}': {source}")]
    ReadError {
        path: String,
        source: std::io::Error,
    },

    #[error("Invalid schema at '{path}': {source}")]
    ValidationError {
        path: String,
        source: SchemaValidationError,
    },
}

/// Schema metadata for listing available schemas.
#[derive(Debug, Clone)]
pub struct SchemaInfo {
    pub name: String,
    pub description: String,
    pub artifacts: Vec<String>,
    pub source: SchemaSource,
}

/// Where a schema was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaSource {
    /// Found in the project's local schemas directory.
    Project,
    /// Found in the user's global schemas directory.
    User,
    /// Found in the package's built-in schemas directory.
    Package,
}

/// Gets the package's built-in schemas directory path.
///
/// This follows the same package-resource model as OpenSpec: built-in schemas
/// ship in a `schemas/` directory beside the executable. `SPECKIT_SCHEMAS_DIR`
/// remains available for embedders and package managers that install resources
/// elsewhere. During Cargo development and tests we also walk up from the test
/// binary so the repository-root `schemas/` directory is discovered.
pub fn get_package_schemas_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("SPECKIT_SCHEMAS_DIR") {
        let path = PathBuf::from(dir);
        if path.is_dir() {
            return Some(path);
        }
    }
    // Packaged installations place schemas beside the executable. Walking
    // ancestors also makes `cargo run` and `cargo test` find `<repo>/schemas`.
    if let Ok(exe) = std::env::current_exe() {
        for parent in exe.ancestors().skip(1) {
            let candidate = parent.join("schemas");
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Gets the user's schema override directory path.
///
/// Resolves to `$XDG_DATA_HOME/speckit/schemas` or
/// `~/.local/share/speckit/schemas` on Unix.
pub fn get_user_schemas_dir() -> Option<PathBuf> {
    let data_dir = dirs::data_dir()?;
    let path = data_dir.join("speckit").join("schemas");
    if path.is_dir() {
        Some(path)
    } else {
        None
    }
}

/// Gets the project-local schemas directory path.
pub fn get_project_schemas_dir(project_root: &Path) -> PathBuf {
    project_root.join("speckit").join("schemas")
}

/// Determines whether a directory entry represents a schema directory candidate.
///
/// Returns `true` for real directories and symlinks pointing at directories.
/// Excludes transient fork-staging and fork-backup directories.
fn is_schema_dir(parent_dir: &Path, entry: &fs::DirEntry) -> bool {
    let name = entry.file_name();
    let name_str = name.to_string_lossy();

    // Exclude transient fork directories
    if name_str.starts_with(".fork-staging-") || name_str.contains(".fork-backup-") {
        return false;
    }

    let file_type = match entry.file_type() {
        Ok(ft) => ft,
        Err(_) => return false,
    };

    if file_type.is_dir() {
        return true;
    }

    if file_type.is_symlink() {
        // Follow the symlink to check if target is a directory
        let full_path = parent_dir.join(&name);
        return fs::metadata(&full_path)
            .map(|m| m.is_dir())
            .unwrap_or(false);
    }

    false
}

/// Validates that a schema name is safe (no path traversal).
fn is_valid_schema_name(name: &str) -> bool {
    if name.is_empty() || name == "." || name == ".." {
        return false;
    }
    if name.contains('/') || name.contains('\\') {
        return false;
    }
    // Reject Windows drive paths
    if name.len() >= 2 && name.as_bytes()[1] == b':' {
        return false;
    }
    true
}

/// Returns a schema directory only when its schema file exists within it.
fn get_schema_candidate_dir(schemas_dir: &Path, name: &str) -> Option<PathBuf> {
    let schema_dir = schemas_dir.join(name);
    let schema_path = schema_dir.join("schema.yaml");
    if schema_path.exists() {
        Some(schema_dir)
    } else {
        None
    }
}

/// Resolves a schema name to its directory path.
///
/// Resolution order (when project_root is provided):
/// 1. Project-local: `<project_root>/speckit/schemas/<name>/schema.yaml`
/// 2. User override: `$XDG_DATA_HOME/speckit/schemas/<name>/schema.yaml`
/// 3. Package built-in: `<package>/schemas/<name>/schema.yaml`
pub fn get_schema_dir(name: &str, project_root: Option<&Path>) -> Option<PathBuf> {
    if !is_valid_schema_name(name) {
        return None;
    }

    // 1. Check project-local directory
    if let Some(root) = project_root {
        let project_dir = get_project_schemas_dir(root);
        if let Some(dir) = get_schema_candidate_dir(&project_dir, name) {
            return Some(dir);
        }
    }

    // 2. Check user override directory
    if let Some(user_dir) = get_user_schemas_dir() {
        if let Some(dir) = get_schema_candidate_dir(&user_dir, name) {
            return Some(dir);
        }
    }

    // 3. Check package built-in directory
    if let Some(package_dir) = get_package_schemas_dir() {
        if let Some(dir) = get_schema_candidate_dir(&package_dir, name) {
            return Some(dir);
        }
    }

    None
}

/// Resolves a schema name to a `SchemaYaml` object.
///
/// Resolution order is the same as `get_schema_dir`.
pub fn resolve_schema(name: &str, project_root: Option<&Path>) -> Result<SchemaYaml, SchemaLoadError> {
    // Normalize name (remove .yaml extension if provided)
    let normalized = name
        .strip_suffix(".yaml")
        .or_else(|| name.strip_suffix(".yml"))
        .unwrap_or(name);

    let schema_dir = get_schema_dir(normalized, project_root).ok_or_else(|| {
        let available = list_schemas(project_root);
        SchemaLoadError::NotFound {
            name: normalized.to_string(),
            available: available.join(", "),
        }
    })?;

    let schema_path = schema_dir.join("schema.yaml");
    let content = fs::read_to_string(&schema_path).map_err(|e| SchemaLoadError::ReadError {
        path: schema_path.display().to_string(),
        source: e,
    })?;

    parse_schema(&content).map_err(|e| SchemaLoadError::ValidationError {
        path: schema_path.display().to_string(),
        source: e,
    })
}

/// Lists all available schema names.
///
/// Combines project-local, user override, and package built-in schemas.
pub fn list_schemas(project_root: Option<&Path>) -> Vec<String> {
    let mut schemas = BTreeSet::new();

    // Package built-in schemas
    if let Some(package_dir) = get_package_schemas_dir() {
        collect_schema_names(&package_dir, &mut schemas);
    }

    // User override schemas
    if let Some(user_dir) = get_user_schemas_dir() {
        collect_schema_names(&user_dir, &mut schemas);
    }

    // Project-local schemas
    if let Some(root) = project_root {
        let project_dir = get_project_schemas_dir(root);
        collect_schema_names(&project_dir, &mut schemas);
    }

    schemas.into_iter().collect()
}

/// Collects schema names from a schemas directory into the provided set.
fn collect_schema_names(schemas_dir: &Path, names: &mut BTreeSet<String>) {
    let entries = match fs::read_dir(schemas_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if is_schema_dir(schemas_dir, &entry) {
            let name = entry.file_name().to_string_lossy().to_string();
            let schema_path = schemas_dir.join(&name).join("schema.yaml");
            if schema_path.exists() {
                names.insert(name);
            }
        }
    }
}

/// Lists all available schemas with their descriptions and artifact lists.
pub fn list_schemas_with_info(project_root: Option<&Path>) -> Vec<SchemaInfo> {
    let mut schemas = Vec::new();
    let mut seen = BTreeSet::new();

    // Project-local schemas first (highest priority)
    if let Some(root) = project_root {
        let project_dir = get_project_schemas_dir(root);
        collect_schema_infos(&project_dir, SchemaSource::Project, &mut schemas, &mut seen);
    }

    // User override schemas
    if let Some(user_dir) = get_user_schemas_dir() {
        collect_schema_infos(&user_dir, SchemaSource::User, &mut schemas, &mut seen);
    }

    // Package built-in schemas
    if let Some(package_dir) = get_package_schemas_dir() {
        collect_schema_infos(&package_dir, SchemaSource::Package, &mut schemas, &mut seen);
    }

    schemas.sort_by(|a, b| a.name.cmp(&b.name));
    schemas
}

/// Collects schema info entries from a schemas directory.
fn collect_schema_infos(
    schemas_dir: &Path,
    source: SchemaSource,
    infos: &mut Vec<SchemaInfo>,
    seen: &mut BTreeSet<String>,
) {
    let entries = match fs::read_dir(schemas_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if !is_schema_dir(schemas_dir, &entry) {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if seen.contains(&name) {
            continue;
        }

        let schema_path = schemas_dir.join(&name).join("schema.yaml");
        let content = match fs::read_to_string(&schema_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let schema = match parse_schema(&content) {
            Ok(s) => s,
            Err(_) => continue,
        };

        infos.push(SchemaInfo {
            name: name.clone(),
            description: schema.description.unwrap_or_default(),
            artifacts: schema.artifacts.iter().map(|a| a.id.clone()).collect(),
            source,
        });
        seen.insert(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_schema_name_rejects_bad_names() {
        assert!(!is_valid_schema_name(""));
        assert!(!is_valid_schema_name("."));
        assert!(!is_valid_schema_name(".."));
        assert!(!is_valid_schema_name("foo/bar"));
        assert!(!is_valid_schema_name("foo\\bar"));
        assert!(!is_valid_schema_name("C:"));
        assert!(is_valid_schema_name("spec-driven"));
        assert!(is_valid_schema_name("my-schema"));
    }

    #[test]
    fn get_schema_dir_returns_none_for_invalid_name() {
        assert!(get_schema_dir("", None).is_none());
        assert!(get_schema_dir("../escape", None).is_none());
    }

    #[test]
    fn get_project_schemas_dir_jins_correctly() {
        let root = Path::new("/home/user/project");
        assert_eq!(
            get_project_schemas_dir(root),
            PathBuf::from("/home/user/project/speckit/schemas")
        );
    }

    #[test]
    fn package_includes_spec_driven_schema() {
        let schemas_dir = get_package_schemas_dir().expect("package schemas directory");
        assert!(schemas_dir.join("spec-driven/schema.yaml").is_file());
        assert!(list_schemas(None).contains(&"spec-driven".to_string()));
    }
}
