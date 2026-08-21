use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::file_state::{
    LockErrorData, acquire_file_lock, make_lock_error_factory, path_is_directory, path_is_file,
    release_file_lock, write_file_atomically,
};
use crate::global_config::get_global_data_dir;
use crate::store::errors::{StoreError, StoreErrorOptions};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const STORE_METADATA_DIR_NAME: &str = ".speckit-store";
pub const STORE_METADATA_FILE_NAME: &str = "store.yaml";
pub const STORES_DIR_NAME: &str = "stores";
pub const STORE_REGISTRY_FILE_NAME: &str = "registry.yaml";

// ---------------------------------------------------------------------------
// Kebab-case id validation (ported from id.ts)
// ---------------------------------------------------------------------------

static KEBAB_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9]+(?:-[a-z0-9]+)*$").unwrap());

pub fn is_kebab_id(value: &str) -> bool {
    KEBAB_ID_RE.is_match(value)
}

/// Human-readable description of the kebab-case grammar.
pub const KEBAB_ID_DESCRIPTION: &str =
    "must be kebab-case with lowercase letters, numbers, and single hyphen separators";

/// Companion fix-line for [`KEBAB_ID_DESCRIPTION`].
pub const KEBAB_ID_FIX: &str =
    "Use kebab-case with lowercase letters, numbers, and single hyphen separators.";

/// Checks whether `value` is a folder-safe name (no empty, no `.`, no `..`,
/// no path separators). Returns `Some(problem_description)` when invalid.
pub fn folder_style_name_problem(value: &str, label: &str) -> Option<String> {
    if value.is_empty() {
        return Some(format!("{label} must not be empty"));
    }
    if value == "." || value == ".." {
        return Some(format!("{label} must not be '{value}'"));
    }
    if value.contains('/') || value.contains('\\') {
        return Some(format!("{label} must not contain path separators"));
    }
    None
}

/// Validates a store id, returning it on success or a [`StoreError`] on failure.
pub fn validate_store_id(id: &str) -> Result<String, StoreError> {
    if let Some(problem) = folder_style_name_problem(id, "Store id") {
        return Err(StoreError::new(
            problem,
            "invalid_store_id",
            StoreErrorOptions {
                target: Some("store.id".into()),
                fix: Some(KEBAB_ID_FIX.into()),
            },
        ));
    }
    if !is_kebab_id(id) {
        return Err(StoreError::new(
            format!("Store id {KEBAB_ID_DESCRIPTION}"),
            "invalid_store_id",
            StoreErrorOptions {
                target: Some("store.id".into()),
                fix: Some(KEBAB_ID_FIX.into()),
            },
        ));
    }
    Ok(id.to_string())
}

/// Returns `true` when `id` passes [`validate_store_id`].
pub fn is_valid_store_id(id: &str) -> bool {
    validate_store_id(id).is_ok()
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn global_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("speckit")
}

/// Returns the directory that holds all registered store entries.
pub fn get_stores_dir(global_data_dir: Option<&Path>) -> PathBuf {
    let base = global_data_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| get_global_data_dir());
    base.join(STORES_DIR_NAME)
}

/// Returns the full path to the store registry YAML file.
pub fn get_store_registry_path(global_data_dir: Option<&Path>) -> PathBuf {
    get_stores_dir(global_data_dir).join(STORE_REGISTRY_FILE_NAME)
}

/// Returns the metadata directory for a given store root.
pub fn get_store_metadata_dir(store_root: &Path) -> PathBuf {
    store_root.join(STORE_METADATA_DIR_NAME)
}

/// Returns the full path to a store's metadata YAML.
pub fn get_store_metadata_path(store_root: &Path) -> PathBuf {
    get_store_metadata_dir(store_root).join(STORE_METADATA_FILE_NAME)
}

/// Extracts the filesystem root from a backend config.
pub fn get_store_root_for_backend(backend: &StoreBackendConfig) -> PathBuf {
    backend.local_path.clone()
}

/// Canonicalizes an existing path using `dunce`; falls back to
/// `fs::canonicalize` when `dunce` cannot simplify.
pub fn canonicalize_existing_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path)
        .unwrap_or_else(|_| fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct StoreGitBackendConfig {
    #[serde(rename = "type")]
    pub backend_type: String,
    pub local_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

/// The single backend variant today.
pub type StoreBackendConfig = StoreGitBackendConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreRegistryEntryState {
    pub backend: StoreBackendConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreRegistryState {
    pub version: u32,
    pub stores: BTreeMap<String, StoreRegistryEntryState>,
}

/// A flattened, id-bearing registry entry used by callers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreRegistryEntry {
    pub id: String,
    pub backend: StoreBackendConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreMetadataState {
    pub version: u32,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolveGitStoreBackendInput {
    pub local_path: PathBuf,
    pub remote: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct StorePathOptions {
    pub global_data_dir: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Parsing / serialization
// ---------------------------------------------------------------------------

fn invalid_store_state_error(label: &str, message: &str) -> StoreError {
    let (code, target, fix) = if label.contains("metadata") {
        (
            "invalid_store_metadata",
            "store.metadata",
            "Repair .speckit-store/store.yaml.",
        )
    } else {
        (
            "invalid_store_registry",
            "store.registry",
            "Repair or remove the registry YAML.",
        )
    };

    StoreError::new(
        format!("Invalid {label}: {message}"),
        code,
        StoreErrorOptions {
            target: Some(target.into()),
            fix: Some(fix.into()),
        },
    )
}

/// Parses raw YAML into a [`StoreRegistryState`].
pub fn parse_store_registry_state(content: &str) -> Result<StoreRegistryState, StoreError> {
    let raw: serde_yaml::Value = serde_yaml::from_str(content)
        .map_err(|e| invalid_store_state_error("store registry state", &e.to_string()))?;

    let state: StoreRegistryState = serde_yaml::from_value(raw.clone())
        .map_err(|e| invalid_store_state_error("store registry state", &e.to_string()))?;

    if state.version != 1 {
        return Err(invalid_store_state_error(
            "store registry state",
            &format!("expected version 1, got {}", state.version),
        ));
    }

    // Validate all store ids.
    for id in state.stores.keys() {
        if !is_kebab_id(id) {
            return Err(invalid_store_state_error(
                "store id",
                &format!("'{id}': {KEBAB_ID_DESCRIPTION}"),
            ));
        }
    }

    Ok(state)
}

/// Parses raw YAML into a [`StoreMetadataState`].
pub fn parse_store_metadata_state(content: &str) -> Result<StoreMetadataState, StoreError> {
    let raw: serde_yaml::Value = serde_yaml::from_str(content)
        .map_err(|e| invalid_store_state_error("store metadata state", &e.to_string()))?;

    let state: StoreMetadataState = serde_yaml::from_value(raw.clone())
        .map_err(|e| invalid_store_state_error("store metadata state", &e.to_string()))?;

    if state.version != 1 {
        return Err(invalid_store_state_error(
            "store metadata state",
            &format!("expected version 1, got {}", state.version),
        ));
    }

    validate_store_id(&state.id)?;

    Ok(state)
}

/// Serializes a [`StoreRegistryState`] to YAML.
pub fn serialize_store_registry_state(state: &StoreRegistryState) -> Result<String, StoreError> {
    for id in state.stores.keys() {
        if !is_kebab_id(id) {
            return Err(invalid_store_state_error(
                "store id",
                &format!("'{id}': {KEBAB_ID_DESCRIPTION}"),
            ));
        }
    }

    serde_yaml::to_string(state)
        .map_err(|e| invalid_store_state_error("store registry state", &e.to_string()))
}

/// Serializes a [`StoreMetadataState`] to YAML.
pub fn serialize_store_metadata_state(state: &StoreMetadataState) -> Result<String, StoreError> {
    validate_store_id(&state.id)?;

    serde_yaml::to_string(state)
        .map_err(|e| invalid_store_state_error("store metadata state", &e.to_string()))
}

// ---------------------------------------------------------------------------
// Listing / reading / writing
// ---------------------------------------------------------------------------

/// Returns a sorted, id-bearing list of registry entries.
pub fn list_store_registry_entries(registry: &StoreRegistryState) -> Vec<StoreRegistryEntry> {
    let mut entries: Vec<StoreRegistryEntry> = registry
        .stores
        .iter()
        .map(|(id, entry)| StoreRegistryEntry {
            id: id.clone(),
            backend: entry.backend.clone(),
        })
        .collect();
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    entries
}

/// Returns `true` when `candidate_root` contains a valid store metadata file.
pub fn is_store_root(candidate_root: &Path) -> bool {
    path_is_file(&get_store_metadata_path(candidate_root))
}

/// Reads the store registry from disk, returning `None` when the file is missing.
pub fn read_store_registry_state(
    options: &StorePathOptions,
) -> Result<Option<StoreRegistryState>, StoreError> {
    let path = get_store_registry_path(options.global_data_dir.as_deref());
    if !path_is_file(&path) {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).map_err(|e| {
        StoreError::new(
            format!("Failed to read {}: {e}", path.display()),
            "store_registry_read_failed",
            StoreErrorOptions::default(),
        )
    })?;
    parse_store_registry_state(&content).map(Some)
}

/// Writes the store registry atomically.
pub fn write_store_registry_state(
    state: &StoreRegistryState,
    options: &StorePathOptions,
) -> Result<(), StoreError> {
    let path = get_store_registry_path(options.global_data_dir.as_deref());
    let content = serialize_store_registry_state(state)?;
    write_file_atomically(&path, &content).map_err(|e| {
        StoreError::new(
            format!("Failed to write {}: {e}", path.display()),
            "store_registry_write_failed",
            StoreErrorOptions::default(),
        )
    })
}

static REGISTRY_LOCK_DATA: LazyLock<LockErrorData> = LazyLock::new(|| LockErrorData {
    create_subject: "the registry lock file".into(),
    busy_message: "Store registry is busy.".into(),
    code: "store_registry_busy".into(),
    target: "store.registry".into(),
});

/// Reads the current registry, applies `updater`, and writes the result
/// back under an exclusive file lock.
pub fn update_store_registry_state(
    updater: &dyn Fn(Option<&StoreRegistryState>) -> Result<StoreRegistryState, StoreError>,
    options: &StorePathOptions,
) -> Result<StoreRegistryState, StoreError> {
    let registry_path = get_store_registry_path(options.global_data_dir.as_deref());
    let lock_path = registry_path.with_extension("yaml.lock");

    let factory = make_lock_error_factory(REGISTRY_LOCK_DATA.clone());
    let lock = acquire_file_lock(&lock_path, &*factory)?;

    let current = read_store_registry_state(options)?;
    let next = updater(current.as_ref())?;
    write_store_registry_state(&next, options)?;

    release_file_lock(lock);
    Ok(next)
}

/// Reads the store metadata from disk.
pub fn read_store_metadata_state(store_root: &Path) -> Result<StoreMetadataState, StoreError> {
    let path = get_store_metadata_path(store_root);
    let content = fs::read_to_string(&path).map_err(|e| {
        StoreError::new(
            format!("Failed to read {}: {e}", path.display()),
            "store_metadata_read_failed",
            StoreErrorOptions::default(),
        )
    })?;
    parse_store_metadata_state(&content)
}

/// Reads the store metadata, returning `None` when the file is missing.
pub fn read_optional_store_metadata_state(
    store_root: &Path,
) -> Result<Option<StoreMetadataState>, StoreError> {
    match read_store_metadata_state(store_root) {
        Ok(state) => Ok(Some(state)),
        Err(e)
            if std::io::Error::new(std::io::ErrorKind::NotFound, "")
                .to_string()
                .is_empty() =>
        {
            // This branch will not actually match; we check via is_file below.
            Ok(None)
        }
        Err(e) => {
            // If the file simply does not exist, return None; propagate other errors.
            let path = get_store_metadata_path(store_root);
            if !path_is_file(&path) {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}

/// Writes store metadata atomically.
pub fn write_store_metadata_state(
    store_root: &Path,
    state: &StoreMetadataState,
) -> Result<(), StoreError> {
    let path = get_store_metadata_path(store_root);
    let content = serialize_store_metadata_state(state)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            StoreError::new(
                format!("Failed to create {}: {e}", parent.display()),
                "store_metadata_dir_failed",
                StoreErrorOptions::default(),
            )
        })?;
    }
    fs::write(&path, &content).map_err(|e| {
        StoreError::new(
            format!("Failed to write {}: {e}", path.display()),
            "store_metadata_write_failed",
            StoreErrorOptions::default(),
        )
    })
}

/// Resolves a [`StoreGitBackendConfig`] from user input.
pub fn resolve_git_store_backend_config(
    input: &ResolveGitStoreBackendInput,
    cwd: Option<&Path>,
) -> Result<StoreGitBackendConfig, StoreError> {
    if input.local_path.as_os_str().is_empty() {
        return Err(StoreError::new(
            "Store local path must not be empty.",
            "store_local_path_empty",
            StoreErrorOptions::default(),
        ));
    }

    let resolved = if input.local_path.is_absolute() {
        input.local_path.clone()
    } else {
        let base = cwd.unwrap_or_else(|| Path::new("."));
        dunce::canonicalize(base.join(&input.local_path)).unwrap_or_else(|_| {
            std::path::absolute(base.join(&input.local_path))
                .unwrap_or_else(|_| base.join(&input.local_path))
        })
    };

    if !path_is_directory(&resolved) {
        return Err(StoreError::new(
            format!(
                "Store local path does not exist: {}",
                input.local_path.display()
            ),
            "store_local_path_missing",
            StoreErrorOptions::default(),
        ));
    }

    if let Some(ref remote) = input.remote {
        if remote.is_empty() {
            return Err(StoreError::new(
                "Store backend remote must not be empty when provided.",
                "store_remote_empty",
                StoreErrorOptions::default(),
            ));
        }
    }

    if let Some(ref branch) = input.branch {
        if branch.is_empty() {
            return Err(StoreError::new(
                "Store branch must not be empty when provided.",
                "store_branch_empty",
                StoreErrorOptions::default(),
            ));
        }
    }

    Ok(StoreGitBackendConfig {
        backend_type: "git".into(),
        local_path: canonicalize_existing_path(&resolved),
        remote: input.remote.clone().filter(|s| !s.is_empty()),
        branch: input.branch.clone().filter(|s| !s.is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebab_id_valid() {
        assert!(is_kebab_id("my-store"));
        assert!(is_kebab_id("abc123"));
        assert!(is_kebab_id("a"));
    }

    #[test]
    fn kebab_id_invalid() {
        assert!(!is_kebab_id(""));
        assert!(!is_kebab_id("My-Store"));
        assert!(!is_kebab_id("-leading"));
        assert!(!is_kebab_id("trailing-"));
        assert!(!is_kebab_id("has--double"));
        assert!(!is_kebab_id("has space"));
    }

    #[test]
    fn folder_style_name_problem_detects_issues() {
        assert!(folder_style_name_problem("", "id").is_some());
        assert!(folder_style_name_problem(".", "id").is_some());
        assert!(folder_style_name_problem("..", "id").is_some());
        assert!(folder_style_name_problem("a/b", "id").is_some());
        assert!(folder_style_name_problem("valid-name", "id").is_none());
    }

    #[test]
    fn validate_store_id_accepts_valid() {
        assert!(validate_store_id("my-store").is_ok());
    }

    #[test]
    fn validate_store_id_rejects_invalid() {
        assert!(validate_store_id("").is_err());
        assert!(validate_store_id("Not-Valid").is_err());
    }

    #[test]
    fn registry_state_round_trip() {
        let mut stores = BTreeMap::new();
        stores.insert(
            "my-store".into(),
            StoreRegistryEntryState {
                backend: StoreGitBackendConfig {
                    backend_type: "git".into(),
                    local_path: PathBuf::from("/tmp/test"),
                    remote: None,
                    branch: None,
                },
            },
        );
        let state = StoreRegistryState { version: 1, stores };
        let yaml = serialize_store_registry_state(&state).unwrap();
        let parsed = parse_store_registry_state(&yaml).unwrap();
        assert_eq!(parsed.stores.len(), 1);
        assert!(parsed.stores.contains_key("my-store"));
    }

    #[test]
    fn metadata_state_round_trip() {
        let state = StoreMetadataState {
            version: 1,
            id: "test-store".into(),
            remote: Some("https://example.com/repo.git".into()),
        };
        let yaml = serialize_store_metadata_state(&state).unwrap();
        let parsed = parse_store_metadata_state(&yaml).unwrap();
        assert_eq!(parsed.id, "test-store");
        assert_eq!(parsed.remote, Some("https://example.com/repo.git".into()));
    }

    #[test]
    fn list_entries_returns_sorted() {
        let mut stores = BTreeMap::new();
        stores.insert(
            "z-store".into(),
            StoreRegistryEntryState {
                backend: StoreGitBackendConfig {
                    backend_type: "git".into(),
                    local_path: PathBuf::from("/z"),
                    remote: None,
                    branch: None,
                },
            },
        );
        stores.insert(
            "a-store".into(),
            StoreRegistryEntryState {
                backend: StoreGitBackendConfig {
                    backend_type: "git".into(),
                    local_path: PathBuf::from("/a"),
                    remote: None,
                    branch: None,
                },
            },
        );
        let state = StoreRegistryState { version: 1, stores };
        let entries = list_store_registry_entries(&state);
        assert_eq!(entries[0].id, "a-store");
        assert_eq!(entries[1].id, "z-store");
    }
}
