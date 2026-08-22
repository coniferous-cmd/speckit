use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::project_config::{classify_speckit_dir, store_pointer_problem};
use crate::speckit_root::inspect_speckit_root;
use crate::store::errors::{RootSelectionError, StoreError, StoreErrorOptions};
use crate::store::foundation::{
    StorePathOptions, canonicalize_existing_path, get_store_metadata_path,
    list_store_registry_entries, read_optional_store_metadata_state, read_store_registry_state,
    validate_store_id,
};
use crate::store::registry::get_store_root;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpeckitRootSource {
    Store,
    Declared,
    GlobalDefault,
    Nearest,
    Implicit,
    ConfigPointer,
}

impl std::fmt::Display for SpeckitRootSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store => write!(f, "store"),
            Self::Declared => write!(f, "declared"),
            Self::GlobalDefault => write!(f, "global_default"),
            Self::Nearest => write!(f, "nearest"),
            Self::Implicit => write!(f, "implicit"),
            Self::ConfigPointer => write!(f, "config_pointer"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoreSelectorOptions {
    pub store: Option<String>,
    pub store_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolveSpeckitRootOptions {
    pub store: Option<String>,
    pub store_path: Option<String>,
    pub start_path: Option<PathBuf>,
    pub allow_implicit_root: Option<bool>,
    pub global_data_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedSpeckitRoot {
    pub path: PathBuf,
    pub changes_dir: PathBuf,
    pub specs_dir: PathBuf,
    pub archive_dir: PathBuf,
    pub default_schema: String,
    pub source: SpeckitRootSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootOutput {
    pub path: PathBuf,
    pub source: SpeckitRootSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_id: Option<String>,
}

/// Result of inspecting a registered store during root resolution.
#[derive(Debug)]
pub enum RegisteredStoreInspection {
    Ok { canonical_root: PathBuf },
    MetadataError { error: StoreError },
    MetadataMissing { metadata_path: PathBuf },
    MetadataIdMismatch { actual_id: String },
    UnhealthyRoot { problems: String },
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_root(
    root_path: &Path,
    source: SpeckitRootSource,
    store_id: Option<String>,
) -> ResolvedSpeckitRoot {
    ResolvedSpeckitRoot {
        path: root_path.to_path_buf(),
        changes_dir: root_path.join("speckit").join("changes"),
        specs_dir: root_path.join("speckit").join("specs"),
        archive_dir: root_path.join("speckit").join("changes").join("archive"),
        default_schema: "spec-driven".into(),
        source,
        store_id,
    }
}

fn canonical_directory(start_path: &Path) -> PathBuf {
    let resolved = if start_path.is_absolute() {
        start_path.to_path_buf()
    } else {
        std::path::absolute(start_path).unwrap_or_else(|_| start_path.to_path_buf())
    };

    let dir = if resolved.is_dir() {
        resolved
    } else {
        resolved.parent().unwrap_or(Path::new("/")).to_path_buf()
    };

    canonicalize_existing_path(&dir)
}

fn doctor_fix(id: &str) -> String {
    format!("Run speckit store doctor {id} to inspect it.")
}

fn from_store_error(error: StoreError) -> RootSelectionError {
    RootSelectionError::new(
        error.to_string(),
        error.code(),
        StoreErrorOptions {
            target: error.target().map(String::from),
            fix: error.fix().map(String::from),
        },
    )
}

// ---------------------------------------------------------------------------
// Inspect a registered store
// ---------------------------------------------------------------------------

/// The metadata-identity and root-health stages of registered-store
/// resolution, as a non-throwing result.
pub fn inspect_registered_store(id: &str, store_root: &Path) -> RegisteredStoreInspection {
    let metadata_path = get_store_metadata_path(store_root);

    let metadata = match read_optional_store_metadata_state(store_root) {
        Ok(m) => m,
        Err(e) => return RegisteredStoreInspection::MetadataError { error: e },
    };

    let metadata = match metadata {
        Some(m) => m,
        None => {
            return RegisteredStoreInspection::MetadataMissing { metadata_path };
        }
    };

    if metadata.id != id {
        return RegisteredStoreInspection::MetadataIdMismatch {
            actual_id: metadata.id,
        };
    }

    let inspection = inspect_speckit_root(store_root);
    if !inspection.healthy {
        let problems = inspection
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let problems = if problems.is_empty() {
            "Speckit root is missing or incomplete.".into()
        } else {
            problems
        };
        return RegisteredStoreInspection::UnhealthyRoot { problems };
    }

    RegisteredStoreInspection::Ok {
        canonical_root: canonicalize_existing_path(store_root),
    }
}

// ---------------------------------------------------------------------------
// Resolve store root
// ---------------------------------------------------------------------------

fn resolve_store_root(
    id: &str,
    global_data_dir: Option<&Path>,
    source: SpeckitRootSource,
) -> Result<ResolvedSpeckitRoot, RootSelectionError> {
    validate_store_id(id).map_err(from_store_error)?;

    let opts = StorePathOptions {
        global_data_dir: global_data_dir.map(Path::to_path_buf),
    };
    let registry = read_store_registry_state(&opts).map_err(from_store_error)?;
    let entries = registry
        .as_ref()
        .map(list_store_registry_entries)
        .unwrap_or_default();
    let entry = entries.iter().find(|e| e.id == id);

    let entry = match entry {
        Some(e) => e,
        None => {
            if entries.is_empty() {
                return Err(RootSelectionError::new(
                    format!("Unknown store '{id}'. No stores are registered."),
                    "no_registered_stores",
                    StoreErrorOptions {
                        target: Some("store.id".into()),
                        fix: Some(format!(
                            "Run speckit store setup {id} or speckit store register <path> first."
                        )),
                    },
                ));
            }
            let ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
            return Err(RootSelectionError::new(
                format!(
                    "Unknown store '{id}'. Registered stores: {}.",
                    ids.join(", ")
                ),
                "unknown_store",
                StoreErrorOptions {
                    target: Some("store.id".into()),
                    fix: Some("Pass a registered store id, or run speckit store list.".into()),
                },
            ));
        }
    };

    let store_root = get_store_root(&entry.backend);

    match inspect_registered_store(id, &store_root) {
        RegisteredStoreInspection::Ok { canonical_root } => {
            Ok(make_root(&canonical_root, source, Some(id.to_string())))
        }
        RegisteredStoreInspection::MetadataError { error } => Err(from_store_error(error)),
        RegisteredStoreInspection::MetadataMissing { metadata_path } => {
            Err(RootSelectionError::new(
                format!(
                    "Store '{}' is missing identity metadata at {}. {}",
                    id,
                    metadata_path.display(),
                    doctor_fix(id)
                ),
                "store_identity_mismatch",
                StoreErrorOptions {
                    target: Some("store.metadata".into()),
                    fix: Some(doctor_fix(id)),
                },
            ))
        }
        RegisteredStoreInspection::MetadataIdMismatch { actual_id } => {
            Err(RootSelectionError::new(
                format!(
                    "Store '{}' metadata id '{}' does not match its registered id. {}",
                    id,
                    actual_id,
                    doctor_fix(id)
                ),
                "store_identity_mismatch",
                StoreErrorOptions {
                    target: Some("store.metadata".into()),
                    fix: Some(doctor_fix(id)),
                },
            ))
        }
        RegisteredStoreInspection::UnhealthyRoot { problems } => Err(RootSelectionError::new(
            format!(
                "Store '{}' does not have a healthy Speckit root at {}: {} {}",
                id,
                store_root.display(),
                problems,
                doctor_fix(id)
            ),
            "unhealthy_store_root",
            StoreErrorOptions {
                target: Some("speckit.root".into()),
                fix: Some(doctor_fix(id)),
            },
        )),
    }
}

// ---------------------------------------------------------------------------
// Main resolver
// ---------------------------------------------------------------------------

/// Resolves the Speckit root for a normal command.
///
/// Priority:
/// 1. `--store <id>` selects a registered store's root.
/// 2. Nearest ancestor containing a qualifying `speckit/` directory.
/// 3. Global `defaultStore` fallback.
/// 4. Error hint with registered stores, or implicit root.
pub fn resolve_speckit_root(
    options: &ResolveSpeckitRootOptions,
) -> Result<ResolvedSpeckitRoot, RootSelectionError> {
    if options.store_path.is_some() {
        return Err(RootSelectionError::new(
            "--store-path is not supported. Register the path with speckit store register <path>, then select it with --store <id>.",
            "store_path_not_supported",
            StoreErrorOptions {
                target: Some("store.id".into()),
                fix: Some("speckit store register <path>, then rerun with --store <id>.".into()),
            },
        ));
    }

    if let Some(ref store_id) = options.store {
        return resolve_store_root(
            store_id,
            options.global_data_dir.as_deref(),
            SpeckitRootSource::Store,
        );
    }

    let start_path = options
        .start_path
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Nearest root walk (simplified — full planning-home port is out of scope).
    if let Some(nearest_root) = find_qualifying_root_sync(&start_path) {
        return resolve_nearest_or_declared_root(&nearest_root, options.global_data_dir.as_deref());
    }

    // Machine-level fallback: global default store.
    // Try SPECKIT_DEFAULT_STORE first, then fall back to OPENSPEC_DEFAULT_STORE
    // for backward compatibility.
    let default_store = std::env::var("SPECKIT_DEFAULT_STORE")
        .or_else(|_| std::env::var("OPENSPEC_DEFAULT_STORE"))
        .unwrap_or_default();
    if !default_store.is_empty() {
        return resolve_default_store_root(&default_store, options.global_data_dir.as_deref());
    }

    // Check for registered stores to produce a helpful error.
    let opts = StorePathOptions {
        global_data_dir: options.global_data_dir.clone(),
    };
    let registry = read_store_registry_state(&opts).map_err(from_store_error)?;
    let registered_ids: Vec<String> = registry
        .as_ref()
        .map(|r| {
            list_store_registry_entries(r)
                .into_iter()
                .map(|e| e.id)
                .collect()
        })
        .unwrap_or_default();

    if !registered_ids.is_empty() {
        return Err(RootSelectionError::new(
            format!(
                "No Speckit root found in the current directory or its ancestors. Registered stores: {}. Pass --store <id> to use one, or run speckit init to create a local root.",
                registered_ids.join(", ")
            ),
            "no_root_with_registered_stores",
            StoreErrorOptions {
                target: Some("speckit.root".into()),
                fix: Some(format!(
                    "Rerun with --store <id> (registered: {}) or run speckit init.",
                    registered_ids.join(", ")
                )),
            },
        ));
    }

    if options.allow_implicit_root == Some(false) {
        return Err(RootSelectionError::new(
            "No Speckit root found from the current directory.",
            "no_speckit_root",
            StoreErrorOptions {
                target: Some("speckit.root".into()),
                fix: Some("Run speckit init to create a root here.".into()),
            },
        ));
    }

    Ok(make_root(
        &canonical_directory(&start_path),
        SpeckitRootSource::Implicit,
        None,
    ))
}

/// Walks up from `start_path` looking for the nearest ancestor containing
/// a qualifying `speckit/` directory (one that has planning shape: specs/
/// or changes/, or a config file).
fn find_qualifying_root_sync(start_path: &Path) -> Option<PathBuf> {
    let mut candidate = find_repo_planning_root_sync(start_path)?;

    loop {
        let speckit = candidate.join("speckit");
        let has_specs = speckit.join("specs").is_dir();
        let has_changes = speckit.join("changes").is_dir();
        let has_config =
            speckit.join("config.yaml").is_file() || speckit.join("config.yml").is_file();

        if has_specs || has_changes || has_config {
            return Some(candidate);
        }

        let parent = candidate.parent()?;
        if parent == candidate {
            return None;
        }
        candidate = find_repo_planning_root_sync(parent)?;
    }
}

/// Finds the nearest ancestor (including `start_path` itself) that
/// contains an `speckit/` directory.
fn find_repo_planning_root_sync(start_path: &Path) -> Option<PathBuf> {
    let mut current = if start_path.is_dir() {
        start_path.to_path_buf()
    } else {
        start_path.parent().unwrap_or(Path::new("/")).to_path_buf()
    };

    loop {
        if current.join("speckit").is_dir() {
            return Some(current);
        }
        let parent = current.parent()?;
        if parent == current {
            return None;
        }
        current = parent.to_path_buf();
    }
}

fn resolve_nearest_or_declared_root(
    nearest_root: &Path,
    global_data_dir: Option<&Path>,
) -> Result<ResolvedSpeckitRoot, RootSelectionError> {
    let speckit = nearest_root.join("speckit");
    let has_planning_shape = speckit.join("specs").is_dir() || speckit.join("changes").is_dir();

    if has_planning_shape {
        // A real planning root wins; any config pointer is ignored with a warning.
        return Ok(make_root(nearest_root, SpeckitRootSource::Nearest, None));
    }

    // Config-only directory — use the shared targeted parser so malformed
    // pointers cannot silently redirect work to the local directory.
    let classification = classify_speckit_dir(nearest_root);
    if let Some(reason) = classification.pointer.malformed {
        let file = classification
            .pointer
            .file_path
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "speckit/config.yaml".to_string());
        return Err(RootSelectionError::new(
            format!(
                "Invalid store declaration in {file}: {}.",
                store_pointer_problem(&reason)
            ),
            "invalid_store_pointer",
            StoreErrorOptions {
                target: Some("store.pointer".into()),
                fix: Some(format!("Fix the YAML syntax or store value in {file}.")),
            },
        ));
    }

    if let Some(store_id) = classification.pointer.value {
        return resolve_store_root(&store_id, global_data_dir, SpeckitRootSource::ConfigPointer);
    }

    // No store pointer found — treat as a local root.
    Ok(make_root(nearest_root, SpeckitRootSource::Nearest, None))
}

fn resolve_default_store_root(
    id: &str,
    global_data_dir: Option<&Path>,
) -> Result<ResolvedSpeckitRoot, RootSelectionError> {
    match resolve_store_root(id, global_data_dir, SpeckitRootSource::GlobalDefault) {
        Ok(root) => Ok(root),
        Err(e) => {
            let stale_fix = if e.code() == "unknown_store" || e.code() == "no_registered_stores" {
                format!(
                    "Register the store (speckit store register <path> --id {id}) or clear the stale global default (speckit config unset defaultStore)."
                )
            } else {
                e.diagnostic.fix.clone().unwrap_or_default()
            };

            Err(RootSelectionError::new(
                format!("Global defaultStore '{id}': {}", e),
                e.code(),
                StoreErrorOptions {
                    target: e.diagnostic.target.clone(),
                    fix: Some(stale_fix),
                },
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Output helpers
// ---------------------------------------------------------------------------

/// Converts a [`ResolvedSpeckitRoot`] to a serializable [`RootOutput`].
pub fn to_root_output(root: &ResolvedSpeckitRoot) -> RootOutput {
    RootOutput {
        path: root.path.clone(),
        source: root.source.clone(),
        store_id: root.store_id.clone(),
    }
}

/// Returns `true` when the root was selected via an explicit store.
pub fn is_store_selected_root(root: &ResolvedSpeckitRoot) -> bool {
    root.store_id.is_some()
}

/// Emits a human-readable banner to stderr when a store root was selected.
pub fn emit_store_root_banner(root: &ResolvedSpeckitRoot) {
    if let Some(ref store_id) = root.store_id {
        eprintln!("Using Speckit root: {store_id} ({})", root.path.display());
    }
}

/// Appends `--store <id>` to `command` when a store was selected.
pub fn with_store_flag(root: &ResolvedSpeckitRoot, command: &str) -> String {
    match &root.store_id {
        Some(id) => format!("{command} --store {id}"),
        None => command.to_string(),
    }
}

/// CLI adapter shared by supported commands. In JSON mode a resolution
/// failure returns `Ok(None)` after printing the error payload to stdout;
/// in human mode the error propagates normally.
pub fn resolve_root_for_command(
    selector: &StoreSelectorOptions,
    json_mode: bool,
    allow_implicit_root: Option<bool>,
) -> Result<Option<ResolvedSpeckitRoot>, RootSelectionError> {
    let options = ResolveSpeckitRootOptions {
        store: selector.store.clone(),
        store_path: selector.store_path.clone(),
        start_path: None,
        allow_implicit_root,
        global_data_dir: None,
    };

    match resolve_speckit_root(&options) {
        Ok(root) => {
            if !json_mode {
                emit_store_root_banner(&root);
            }
            Ok(Some(root))
        }
        Err(e) => {
            if json_mode {
                let payload = serde_json::json!({
                    "status": [{
                        "severity": "error",
                        "code": e.code(),
                        "message": e.to_string(),
                        "target": e.diagnostic.target,
                        "fix": e.diagnostic.fix,
                    }]
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).unwrap_or_default()
                );
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_root_has_correct_paths() {
        let root = make_root(Path::new("/tmp/my-store"), SpeckitRootSource::Nearest, None);
        assert_eq!(root.path, PathBuf::from("/tmp/my-store"));
        assert_eq!(
            root.changes_dir,
            PathBuf::from("/tmp/my-store/speckit/changes")
        );
        assert_eq!(root.specs_dir, PathBuf::from("/tmp/my-store/speckit/specs"));
        assert_eq!(
            root.archive_dir,
            PathBuf::from("/tmp/my-store/speckit/changes/archive")
        );
        assert_eq!(root.default_schema, "spec-driven");
        assert!(root.store_id.is_none());
    }

    #[test]
    fn to_root_output_omits_store_id_when_absent() {
        let root = make_root(Path::new("/tmp"), SpeckitRootSource::Implicit, None);
        let output = to_root_output(&root);
        assert!(output.store_id.is_none());
    }

    #[test]
    fn to_root_output_includes_store_id_when_present() {
        let root = make_root(
            Path::new("/tmp"),
            SpeckitRootSource::Store,
            Some("my-store".into()),
        );
        let output = to_root_output(&root);
        assert_eq!(output.store_id, Some("my-store".into()));
    }

    #[test]
    fn with_store_flag_appends_when_store_selected() {
        let root = make_root(
            Path::new("/tmp"),
            SpeckitRootSource::Store,
            Some("my-store".into()),
        );
        assert_eq!(
            with_store_flag(&root, "speckit status"),
            "speckit status --store my-store"
        );
    }

    #[test]
    fn with_store_flag_passes_through_when_no_store() {
        let root = make_root(Path::new("/tmp"), SpeckitRootSource::Nearest, None);
        assert_eq!(with_store_flag(&root, "speckit status"), "speckit status");
    }

    #[test]
    fn is_store_selected_root_reflects_store_id() {
        let with_store = make_root(
            Path::new("/tmp"),
            SpeckitRootSource::Store,
            Some("x".into()),
        );
        assert!(is_store_selected_root(&with_store));

        let without = make_root(Path::new("/tmp"), SpeckitRootSource::Nearest, None);
        assert!(!is_store_selected_root(&without));
    }
}
