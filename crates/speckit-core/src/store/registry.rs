use std::path::{Path, PathBuf};

use crate::store::errors::{StoreError, StoreErrorOptions};
use crate::store::foundation::{
    ResolveGitStoreBackendInput, StoreBackendConfig, StoreGitBackendConfig, StorePathOptions,
    StoreRegistryEntry, StoreRegistryState, canonicalize_existing_path, get_store_metadata_path,
    get_store_root_for_backend, list_store_registry_entries, read_optional_store_metadata_state,
    read_store_registry_state, resolve_git_store_backend_config, update_store_registry_state,
    validate_store_id, write_store_metadata_state,
};

// Re-export for backward compatibility with operations.rs
pub use crate::store::foundation::get_store_root_for_backend as get_store_root;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Input for [`register_store`].
#[derive(Debug, Clone)]
pub struct RegisterStoreInput {
    pub id: String,
    pub local_path: PathBuf,
    pub remote: Option<String>,
    pub branch: Option<String>,
    pub cwd: Option<PathBuf>,
    pub global_data_dir: Option<PathBuf>,
}

/// Input for [`resolve_registered_store`].
#[derive(Debug, Clone)]
pub struct ResolveRegisteredStoreInput {
    pub id: String,
    pub global_data_dir: Option<PathBuf>,
}

/// Input for [`get_registered_store`].
#[derive(Debug, Clone)]
pub struct GetRegisteredStoreInput {
    pub id: String,
    pub global_data_dir: Option<PathBuf>,
    pub expected_backend: Option<StoreGitBackendConfig>,
}

/// Input for [`unregister_store_registration`].
#[derive(Debug, Clone)]
pub struct UnregisterStoreInput {
    pub id: String,
    pub global_data_dir: Option<PathBuf>,
    pub expected_backend: Option<StoreGitBackendConfig>,
}

#[derive(Debug, Clone)]
pub struct RegisteredStoreEntry {
    pub id: String,
    pub backend: StoreBackendConfig,
    pub store_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ResolvedStore {
    pub id: String,
    pub store_root: PathBuf,
    pub backend: StoreGitBackendConfig,
}

#[derive(Debug, Clone)]
pub struct StoreRegistrationCommit {
    pub id: String,
    pub store_root: PathBuf,
    pub backend: StoreGitBackendConfig,
    pub metadata_created: bool,
    pub registry_updated: bool,
    pub already_registered: bool,
}

#[derive(Debug, Clone)]
pub struct CommitStoreRegistrationInput {
    pub id: String,
    pub backend: StoreGitBackendConfig,
    pub write_metadata_if_missing: bool,
    pub global_data_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct RegistrySnapshot {
    /// `None` = the registry is unreadable; `Some(vec![])` = empty or absent.
    pub entries: Option<Vec<StoreRegistryEntry>>,
    pub unreadable: bool,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Normalizes a path for comparison, canonicalizing when possible and
/// falling back to absolute resolution.
fn normalize_path_for_comparison(target: &Path) -> PathBuf {
    dunce::canonicalize(target)
        .or_else(|_| std::path::absolute(target))
        .unwrap_or_else(|_| target.to_path_buf())
}

/// Asserts that registering `id` with `backend` does not collide with an
/// existing entry (same id at a different path, or a different id at the
/// same path).
pub fn assert_no_registered_store_conflict(
    registry: &StoreRegistryState,
    id: &str,
    backend: &StoreGitBackendConfig,
) -> Result<(), StoreError> {
    let next_path = normalize_path_for_comparison(&get_store_root(backend));

    for entry in list_store_registry_entries(registry) {
        let entry_path = normalize_path_for_comparison(&get_store_root(&entry.backend));

        if entry.id == id && entry_path == next_path {
            continue;
        }

        if entry.id == id {
            return Err(StoreError::new(
                format!(
                    "Store '{}' is already registered at {}. One checkout per store id is supported on this machine.",
                    id,
                    get_store_root(&entry.backend).display()
                ),
                "store_id_conflict",
                StoreErrorOptions {
                    target: Some("store.id".into()),
                    fix: Some(format!(
                        "Use the existing registration, or run speckit store unregister {id} first to switch this id to a different checkout."
                    )),
                },
            ));
        }

        if entry_path == next_path {
            return Err(StoreError::new(
                format!("Store path is already registered as '{}'.", entry.id),
                "store_path_conflict",
                StoreErrorOptions {
                    target: Some("store.root".into()),
                    fix: Some(format!(
                        "Use the existing '{}' registration or choose a different path.",
                        entry.id
                    )),
                },
            ));
        }
    }

    Ok(())
}

fn with_registered_store(
    registry: Option<&StoreRegistryState>,
    id: &str,
    backend: &StoreGitBackendConfig,
) -> Result<StoreRegistryState, StoreError> {
    let current = registry.cloned().unwrap_or(StoreRegistryState {
        version: 1,
        stores: Default::default(),
    });
    assert_no_registered_store_conflict(&current, id, backend)?;

    let mut stores = current.stores;
    stores.insert(
        id.to_string(),
        crate::store::foundation::StoreRegistryEntryState {
            backend: backend.clone(),
        },
    );

    Ok(StoreRegistryState { version: 1, stores })
}

fn get_registered_store_or_throw(
    registry: Option<&StoreRegistryState>,
    id: &str,
) -> Result<StoreRegistryEntry, StoreError> {
    let reg = registry.ok_or_else(|| {
        StoreError::new(
            "No store registry found",
            "no_store_registry",
            StoreErrorOptions {
                target: Some("store.id".into()),
                fix: Some("Register a store with speckit store register <path>.".into()),
            },
        )
    })?;

    reg.stores
        .get(id)
        .map(|entry| StoreRegistryEntry {
            id: id.to_string(),
            backend: entry.backend.clone(),
        })
        .ok_or_else(|| {
            StoreError::new(
                format!("Unknown store '{id}'"),
                "store_not_found",
                StoreErrorOptions {
                    target: Some("store.id".into()),
                    fix: Some("Run speckit store list to see registered stores.".into()),
                },
            )
        })
}

/// Same checkout: type, canonical path, and branch — remote excluded.
fn same_checkout(actual: &StoreGitBackendConfig, expected: &StoreGitBackendConfig) -> bool {
    actual.backend_type == expected.backend_type
        && normalize_path_for_comparison(&actual.local_path)
            == normalize_path_for_comparison(&expected.local_path)
        && actual.branch == expected.branch
}

fn store_backends_match(actual: &StoreGitBackendConfig, expected: &StoreGitBackendConfig) -> bool {
    same_checkout(actual, expected) && actual.remote == expected.remote
}

fn assert_expected_registered_backend(
    id: &str,
    actual: &StoreGitBackendConfig,
    expected: Option<&StoreGitBackendConfig>,
) -> Result<(), StoreError> {
    match expected {
        None => Ok(()),
        Some(exp) if store_backends_match(actual, exp) => Ok(()),
        Some(_) => Err(StoreError::new(
            format!("Store '{id}' changed before cleanup completed."),
            "store_registry_changed",
            StoreErrorOptions {
                target: Some("store.registry".into()),
                fix: Some(
                    "Retry the cleanup command after reviewing the current store registration."
                        .into(),
                ),
            },
        )),
    }
}

fn without_registered_store(
    registry: Option<&StoreRegistryState>,
    id: &str,
    expected_backend: Option<&StoreGitBackendConfig>,
) -> Result<(StoreRegistryState, StoreRegistryEntry), StoreError> {
    let removed = get_registered_store_or_throw(registry, id)?;
    assert_expected_registered_backend(id, &removed.backend, expected_backend)?;

    let current = registry.cloned().unwrap_or(StoreRegistryState {
        version: 1,
        stores: Default::default(),
    });
    let mut stores = current.stores;
    stores.remove(id);

    Ok((StoreRegistryState { version: 1, stores }, removed))
}

fn ensure_store_metadata(
    store_root: &Path,
    id: &str,
    write_if_missing: bool,
) -> Result<bool, StoreError> {
    let metadata = read_optional_store_metadata_state(store_root)?;

    match metadata {
        None => {
            if !write_if_missing {
                return Err(StoreError::new(
                    format!(
                        "Registered store '{}' is missing metadata at {}",
                        id,
                        get_store_metadata_path(store_root).display()
                    ),
                    "store_metadata_missing",
                    StoreErrorOptions {
                        target: Some("store.metadata".into()),
                        fix: Some(format!(
                            "Create {} or rerun \"speckit store register <path>\".",
                            get_store_metadata_path(store_root).display()
                        )),
                    },
                ));
            }
            write_store_metadata_state(
                store_root,
                &crate::store::foundation::StoreMetadataState {
                    version: 1,
                    id: id.to_string(),
                    remote: None,
                },
            )?;
            Ok(true)
        }
        Some(meta) => {
            if meta.id != id {
                return Err(StoreError::new(
                    format!(
                        "Store metadata id '{}' does not match registered id '{}'",
                        meta.id, id
                    ),
                    "store_metadata_id_mismatch",
                    StoreErrorOptions {
                        target: Some("store.metadata".into()),
                        fix: Some(
                            "Repair the local registry or store metadata so the ids match.".into(),
                        ),
                    },
                ));
            }
            Ok(false)
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Commits a store registration: writes metadata (if needed) and updates
/// the registry under lock.
pub fn commit_store_registration(
    input: &CommitStoreRegistrationInput,
) -> Result<StoreRegistrationCommit, StoreError> {
    let id = validate_store_id(&input.id)?;
    let store_root = get_store_root(&input.backend);

    let metadata_created =
        ensure_store_metadata(&store_root, &id, input.write_metadata_if_missing)?;

    let opts = StorePathOptions {
        global_data_dir: input.global_data_dir.clone(),
    };

    // We need to check what's in the registry before updating.
    let pre_registry = read_store_registry_state(&opts)?;
    let existing_backend = pre_registry
        .as_ref()
        .and_then(|r| r.stores.get(&id))
        .map(|e| &e.backend);

    let is_rerun = existing_backend
        .map(|eb| same_checkout(eb, &input.backend))
        .unwrap_or(false);
    let up_to_date = is_rerun
        && existing_backend
            .map(|eb| store_backends_match(eb, &input.backend))
            .unwrap_or(false);

    let mut registry_updated = false;

    if !up_to_date {
        let backend_clone = input.backend.clone();
        let id_clone = id.clone();
        update_store_registry_state(
            &|registry| with_registered_store(registry, &id_clone, &backend_clone),
            &opts,
        )?;
        registry_updated = true;
    }

    Ok(StoreRegistrationCommit {
        id: id.clone(),
        store_root,
        backend: input.backend.clone(),
        metadata_created,
        registry_updated,
        already_registered: is_rerun,
    })
}

/// Registers a new store from user input.
pub fn register_store(input: &RegisterStoreInput) -> Result<ResolvedStore, StoreError> {
    let id = validate_store_id(&input.id)?;
    let cwd = input.cwd.as_deref();
    let backend = resolve_git_store_backend_config(
        &ResolveGitStoreBackendInput {
            local_path: input.local_path.clone(),
            remote: input.remote.clone(),
            branch: input.branch.clone(),
        },
        cwd,
    )?;
    let store_root = get_store_root(&backend);

    let committed = commit_store_registration(&CommitStoreRegistrationInput {
        id,
        backend,
        write_metadata_if_missing: true,
        global_data_dir: input.global_data_dir.clone(),
    })?;

    Ok(ResolvedStore {
        id: committed.id,
        store_root: committed.store_root,
        backend: committed.backend,
    })
}

/// Reads a snapshot of the registry for a single command's lifetime.
pub fn read_registry_snapshot(global_data_dir: Option<&Path>) -> RegistrySnapshot {
    let opts = StorePathOptions {
        global_data_dir: global_data_dir.map(Path::to_path_buf),
    };
    match read_store_registry_state(&opts) {
        Ok(Some(registry)) => RegistrySnapshot {
            entries: Some(list_store_registry_entries(&registry)),
            unreadable: false,
        },
        Ok(None) => RegistrySnapshot {
            entries: Some(Vec::new()),
            unreadable: false,
        },
        Err(_) => RegistrySnapshot {
            entries: None,
            unreadable: true,
        },
    }
}

/// Lists all registered stores with their resolved roots.
pub fn list_registered_stores(
    global_data_dir: Option<&Path>,
) -> Result<Vec<RegisteredStoreEntry>, StoreError> {
    let opts = StorePathOptions {
        global_data_dir: global_data_dir.map(Path::to_path_buf),
    };
    let registry = read_store_registry_state(&opts)?;

    match registry {
        None => Ok(Vec::new()),
        Some(reg) => Ok(list_store_registry_entries(&reg)
            .into_iter()
            .map(|entry| RegisteredStoreEntry {
                store_root: get_store_root(&entry.backend),
                id: entry.id,
                backend: entry.backend,
            })
            .collect()),
    }
}

/// Gets a single registered store entry by id.
pub fn get_registered_store(
    input: &GetRegisteredStoreInput,
) -> Result<RegisteredStoreEntry, StoreError> {
    let id = validate_store_id(&input.id)?;
    let opts = StorePathOptions {
        global_data_dir: input.global_data_dir.clone(),
    };
    let registry = read_store_registry_state(&opts)?;
    let entry = get_registered_store_or_throw(registry.as_ref(), &id)?;
    assert_expected_registered_backend(&id, &entry.backend, input.expected_backend.as_ref())?;

    Ok(RegisteredStoreEntry {
        store_root: get_store_root(&entry.backend),
        id: entry.id,
        backend: entry.backend,
    })
}

/// Unregisters a store from the registry.
pub fn unregister_store_registration(
    input: &UnregisterStoreInput,
) -> Result<RegisteredStoreEntry, StoreError> {
    let id = validate_store_id(&input.id)?;
    let opts = StorePathOptions {
        global_data_dir: input.global_data_dir.clone(),
    };
    let expected_backend = input.expected_backend.clone();
    let removed_entry: std::cell::Cell<Option<StoreRegistryEntry>> = std::cell::Cell::new(None);

    {
        let id_clone = id.clone();
        let eb = expected_backend.clone();
        let removed_ref = &removed_entry;
        update_store_registry_state(
            &|registry| {
                let (next, removed) = without_registered_store(registry, &id_clone, eb.as_ref())?;
                removed_ref.set(Some(removed));
                Ok(next)
            },
            &opts,
        )?;
    }

    let removed = removed_entry.into_inner().ok_or_else(|| {
        StoreError::new(
            format!("Unknown store '{id}'"),
            "store_not_found",
            StoreErrorOptions {
                target: Some("store.id".into()),
                fix: Some("Run speckit store list to see registered stores.".into()),
            },
        )
    })?;

    Ok(RegisteredStoreEntry {
        store_root: get_store_root(&removed.backend),
        id: removed.id,
        backend: removed.backend,
    })
}

/// Resolves a registered store by id, verifying its metadata.
pub fn resolve_registered_store(
    input: &ResolveRegisteredStoreInput,
) -> Result<ResolvedStore, StoreError> {
    let id = validate_store_id(&input.id)?;
    let opts = StorePathOptions {
        global_data_dir: input.global_data_dir.clone(),
    };
    let registry = read_store_registry_state(&opts)?;

    if registry.is_none() {
        return Err(StoreError::new(
            "No store registry found",
            "no_store_registry",
            StoreErrorOptions {
                target: Some("store.id".into()),
                fix: Some(
                    "Register a store with speckit store register <path>, then select it with --store <id>.".into(),
                ),
            },
        ));
    }

    let entry = get_registered_store_or_throw(registry.as_ref(), &id)?;
    let store_root = get_store_root(&entry.backend);
    ensure_store_metadata(&store_root, &id, false)?;

    Ok(ResolvedStore {
        id,
        store_root,
        backend: entry.backend,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::foundation::{StoreRegistryEntryState, StoreRegistryState};
    use std::collections::BTreeMap;

    fn make_backend(path: &str) -> StoreGitBackendConfig {
        StoreGitBackendConfig {
            backend_type: "git".into(),
            local_path: PathBuf::from(path),
            remote: None,
            branch: None,
        }
    }

    #[test]
    fn conflict_detection_same_id_different_path() {
        let mut stores = BTreeMap::new();
        stores.insert(
            "my-store".into(),
            StoreRegistryEntryState {
                backend: make_backend("/tmp/store-a"),
            },
        );
        let registry = StoreRegistryState { version: 1, stores };

        let result = assert_no_registered_store_conflict(
            &registry,
            "my-store",
            &make_backend("/tmp/store-b"),
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), "store_id_conflict");
    }

    #[test]
    fn conflict_detection_different_id_same_path() {
        let mut stores = BTreeMap::new();
        stores.insert(
            "other-store".into(),
            StoreRegistryEntryState {
                backend: make_backend("/tmp/store-a"),
            },
        );
        let registry = StoreRegistryState { version: 1, stores };

        let result = assert_no_registered_store_conflict(
            &registry,
            "my-store",
            &make_backend("/tmp/store-a"),
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), "store_path_conflict");
    }

    #[test]
    fn no_conflict_when_empty() {
        let registry = StoreRegistryState {
            version: 1,
            stores: BTreeMap::new(),
        };

        assert!(
            assert_no_registered_store_conflict(&registry, "my-store", &make_backend("/tmp/store"))
                .is_ok()
        );
    }
}
