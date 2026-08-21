use std::fs;
use std::path::{Path, PathBuf};

use crate::store::errors::{
    StoreDiagnostic, StoreDiagnosticSeverity, StoreError, StoreErrorOptions, make_store_diagnostic,
};
use crate::store::foundation::{
    KEBAB_ID_DESCRIPTION, ResolveGitStoreBackendInput, StoreBackendConfig, StoreGitBackendConfig,
    StoreMetadataState, StorePathOptions, StoreRegistryState, canonicalize_existing_path,
    get_store_metadata_dir, get_store_metadata_path, get_store_registry_path,
    get_store_root_for_backend, is_kebab_id, list_store_registry_entries,
    read_optional_store_metadata_state, read_store_registry_state,
    resolve_git_store_backend_config, validate_store_id, write_store_metadata_state,
};
use crate::store::git::{
    assert_git_commit_identity, commit_store_files, git_directory_has_tracked_files,
    git_has_commits, git_has_remote, git_has_uncommitted_changes, git_origin_url,
    init_git_repository, is_git_repository_at_root,
};
use crate::store::registry::{
    CommitStoreRegistrationInput, RegisteredStoreEntry, UnregisterStoreInput,
    assert_no_registered_store_conflict, commit_store_registration, get_registered_store,
    get_store_root, list_registered_stores, unregister_store_registration,
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type PathKind = &'static str;
const MISSING: PathKind = "missing";
const DIRECTORY: PathKind = "directory";
const FILE: PathKind = "file";
const OTHER: PathKind = "other";

fn path_kind(path: &Path) -> PathKind {
    match fs::metadata(path) {
        Ok(m) if m.is_dir() => DIRECTORY,
        Ok(m) if m.is_file() => FILE,
        Ok(_) => OTHER,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => MISSING,
        Err(e) => {
            // Propagate unexpected I/O errors as a panic since callers
            // don't handle this case in the TS source either.
            panic!("Unexpected stat error on {}: {e}", path.display());
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoreInfo {
    pub id: String,
    pub root: PathBuf,
    pub metadata_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct StoreRemotes {
    pub canonical: Option<String>,
    pub observed: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoreMutationRegistryCommit {
    pub path: PathBuf,
    pub registered: bool,
    pub already_registered: bool,
}

#[derive(Debug, Clone)]
pub struct StoreMutationGit {
    pub is_repository: bool,
    pub initialized: bool,
    pub committed: bool,
}

#[derive(Debug, Clone)]
pub struct StoreMutationResult {
    pub store: StoreInfo,
    pub remotes: Option<StoreRemotes>,
    pub registry_commit: StoreMutationRegistryCommit,
    pub git: StoreMutationGit,
    pub created_artifacts: Vec<String>,
    pub diagnostics: Vec<StoreDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct StoreCleanupResult {
    pub store: StoreInfo,
    pub registry_commit: StoreCleanupRegistryCommit,
    pub files: StoreCleanupFiles,
    pub diagnostics: Vec<StoreDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct StoreCleanupRegistryCommit {
    pub path: PathBuf,
    pub removed: bool,
}

#[derive(Debug, Clone)]
pub struct StoreCleanupFiles {
    pub deleted: bool,
    pub deleted_path: Option<PathBuf>,
    pub left_on_disk: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct StoreListResult {
    pub stores: Vec<StoreInfo>,
}

#[derive(Debug, Clone)]
pub struct StoreDoctorResult {
    pub stores: Vec<StoreInspection>,
    pub diagnostics: Vec<StoreDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct StoreInspectionMetadata {
    pub present: Option<bool>,
    pub valid: Option<bool>,
    pub id: Option<String>,
    pub remote: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoreInspectionGit {
    pub is_repository: Option<bool>,
    pub has_commits: Option<bool>,
    pub has_uncommitted_changes: Option<bool>,
    pub has_remote: Option<bool>,
    pub origin_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoreInspection {
    pub id: String,
    pub root: PathBuf,
    pub metadata_path: PathBuf,
    pub metadata: StoreInspectionMetadata,
    pub git: StoreInspectionGit,
    pub diagnostics: Vec<StoreDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct SetupStoreInput {
    pub id: Option<String>,
    pub path: Option<String>,
    pub init_git: Option<bool>,
    pub allow_inside_git_repository: Option<bool>,
    pub remote: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegisterExistingStoreInput {
    pub path: Option<String>,
    pub id: Option<String>,
    pub allow_create_identity: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct CleanupStoreInput {
    pub id: String,
    pub global_data_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct PreparedStoreCleanup {
    pub id: String,
    pub root: PathBuf,
    pub metadata_path: PathBuf,
    pub backend: StoreGitBackendConfig,
    pub global_data_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct PreparedStoreSetup {
    pub id: String,
    pub root: PathBuf,
    pub root_kind: PathKind,
    pub backend: Option<StoreGitBackendConfig>,
    pub registry: Option<StoreRegistryState>,
    pub remote: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_directory_empty(directory: &Path) -> bool {
    fs::read_dir(directory)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
}

fn read_store_metadata_for_operation(
    store_root: &Path,
) -> Result<Option<StoreMetadataState>, StoreError> {
    read_optional_store_metadata_state(store_root).map_err(|e| {
        StoreError::new(
            e.to_string(),
            "invalid_store_metadata",
            StoreErrorOptions {
                target: Some("store.metadata".into()),
                fix: Some(format!(
                    "Repair {}.",
                    get_store_metadata_path(store_root).display()
                )),
            },
        )
    })
}

fn is_git_only_directory(store_root: &Path) -> bool {
    match fs::read_dir(store_root) {
        Ok(mut entries) => {
            let first = entries.next();
            first.is_some()
                && entries.next().is_none()
                && first.unwrap().unwrap().file_name() == ".git"
                && is_git_repository_at_root(store_root)
        }
        Err(_) => false,
    }
}

fn already_registered_diagnostic(id: &str) -> StoreDiagnostic {
    make_store_diagnostic(
        StoreDiagnosticSeverity::Info,
        "store_already_registered",
        format!("Store '{id}' is already registered at this path."),
        Some("store.registry".into()),
        None,
    )
}

fn mutate_info(id: &str, store_root: &Path) -> StoreInfo {
    StoreInfo {
        id: id.to_string(),
        root: store_root.to_path_buf(),
        metadata_path: Some(get_store_metadata_path(store_root)),
    }
}

fn mutation_payload(
    id: &str,
    store_root: &Path,
    git: StoreMutationGit,
    created_files: Vec<String>,
    registry: (bool, bool),
    diagnostics: Vec<StoreDiagnostic>,
    remotes: Option<StoreRemotes>,
) -> StoreMutationResult {
    StoreMutationResult {
        store: mutate_info(id, store_root),
        remotes,
        registry_commit: StoreMutationRegistryCommit {
            path: get_store_registry_path(None),
            registered: registry.0,
            already_registered: registry.1,
        },
        git,
        created_artifacts: created_files,
        diagnostics,
    }
}

fn doctor_status_for_error(
    error: &StoreError,
    code: &str,
    target: &str,
    fix: Option<&str>,
) -> StoreDiagnostic {
    make_store_diagnostic(
        StoreDiagnosticSeverity::Error,
        code,
        error.to_string(),
        Some(target.into()),
        fix.map(String::from),
    )
}

fn is_registered_at_path(
    registry: &Option<StoreRegistryState>,
    id: &str,
    store_root: &Path,
) -> bool {
    let normalized = normalize_for_comparison(store_root);
    registry
        .as_ref()
        .and_then(|r| r.stores.get(id))
        .map(|e| normalize_for_comparison(&get_store_root_for_backend(&e.backend)) == normalized)
        .unwrap_or(false)
}

fn normalize_for_comparison(target: &Path) -> PathBuf {
    dunce::canonicalize(target)
        .or_else(|_| std::path::absolute(target))
        .unwrap_or_else(|_| target.to_path_buf())
}

fn expand_user_path(input: &str) -> PathBuf {
    let trimmed = input.trim();
    if trimmed == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    }
    if trimmed.starts_with("~/") || trimmed.starts_with("~\\") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&trimmed[2..]);
        }
    }
    PathBuf::from(trimmed)
}

fn resolve_setup_root(id: &str, input_path: Option<&str>) -> Result<PathBuf, StoreError> {
    let raw = input_path.unwrap_or("");
    if raw.trim().is_empty() {
        return Err(StoreError::new(
            "Pass --path with the folder where this store should live.",
            "store_setup_path_required",
            StoreErrorOptions {
                target: Some("store.root".into()),
                fix: Some(format!("speckit store setup {id} --path ~/speckit/{id}")),
            },
        ));
    }
    Ok(std::path::absolute(expand_user_path(raw)).unwrap_or_else(|_| expand_user_path(raw)))
}

fn resolve_register_root(input_path: Option<&str>) -> Result<PathBuf, StoreError> {
    let raw = input_path.unwrap_or("");
    if raw.trim().is_empty() {
        return Err(StoreError::new(
            "Pass a store path.",
            "store_path_required",
            StoreErrorOptions {
                target: Some("store.root".into()),
                fix: Some("speckit store register /path/to/store".into()),
            },
        ));
    }
    Ok(std::path::absolute(expand_user_path(raw)).unwrap_or_else(|_| expand_user_path(raw)))
}

fn infer_store_id_from_path(store_root: &Path) -> Result<String, StoreError> {
    let name = store_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    validate_store_id(&name)
}

fn nearest_existing_directory(target: &Path) -> Option<PathBuf> {
    let mut current = std::path::absolute(target).ok()?;
    loop {
        match fs::metadata(&current) {
            Ok(m) if m.is_dir() => return Some(current),
            Ok(_) => return None,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let parent = current.parent()?;
                if parent == current {
                    return None;
                }
                current = parent.to_path_buf();
            }
            Err(_) => return None,
        }
    }
}

fn resolve_backend_with_observed_origin(
    store_root: &Path,
) -> Result<StoreGitBackendConfig, StoreError> {
    let origin = if is_git_repository_at_root(store_root) {
        git_origin_url(store_root)
    } else {
        None
    };

    resolve_git_store_backend_config(
        &ResolveGitStoreBackendInput {
            local_path: store_root.to_path_buf(),
            remote: origin,
            branch: None,
        },
        None,
    )
}

fn remote_requires_hand_edit_error(id: &str, store_root: &Path) -> StoreError {
    StoreError::new(
        format!("Store '{id}' already has an identity file; --remote cannot change it."),
        "store_remote_requires_hand_edit",
        StoreErrorOptions {
            target: Some("store.metadata".into()),
            fix: Some(format!(
                "Edit {} and commit it.",
                get_store_metadata_path(store_root).display()
            )),
        },
    )
}

/// Common pre-check: a store root declared with a config-only pointer is
/// not a real store root.
///
/// A config-only root is a directory that has a `speckit/config.yaml` (or `.yml`)
/// with a `store:` key pointing elsewhere — it is not itself a store.
fn assert_not_config_only_pointer_root(store_root: &Path) -> Result<(), StoreError> {
    let speckit_dir = store_root.join("speckit");
    let config_candidates = [
        speckit_dir.join("config.yaml"),
        speckit_dir.join("config.yml"),
    ];

    for config_path in &config_candidates {
        if config_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(config_path) {
                if let Ok(config) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    if config.get("store").is_some() {
                        return Err(StoreError::new(
                            format!(
                                "{} is a config-only root (it declares a store: pointer). \
                                 Use the target store directly instead.",
                                store_root.display()
                            ),
                            "config_only_pointer_root",
                            StoreErrorOptions {
                                target: Some("store.root".into()),
                                fix: Some(format!(
                                    "Run the command against the store that '{}' points to, \
                                     or remove the store: key from {}.",
                                    store_root.display(),
                                    config_path.display()
                                )),
                            },
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Public API: expand_user_path
// ---------------------------------------------------------------------------

/// Expands `~` and `~/` prefixes to the user's home directory.
pub fn expand_user(input: &str) -> PathBuf {
    expand_user_path(input)
}

// ---------------------------------------------------------------------------
// Public API: resolveSetupGitEnabled
// ---------------------------------------------------------------------------

/// Resolves the effective Git mode for a prepared setup: on by default for
/// new stores, off for reruns of an already-registered store.
pub fn resolve_setup_git_enabled(prepared: &PreparedStoreSetup, init_git: Option<bool>) -> bool {
    init_git
        .unwrap_or_else(|| !is_registered_at_path(&prepared.registry, &prepared.id, &prepared.root))
}

// ---------------------------------------------------------------------------
// Public API: prepareStoreSetup / setupPreparedStore / setupStore
// ---------------------------------------------------------------------------

/// Prepares a store setup plan without making any changes.
pub fn prepare_store_setup(input: &SetupStoreInput) -> Result<PreparedStoreSetup, StoreError> {
    let id_str = input.id.as_deref().unwrap_or("");
    let id = validate_store_id(id_str)?;

    if let Some(ref remote) = input.remote {
        if remote.is_empty() {
            return Err(StoreError::new(
                "Store remote must not be empty when provided.",
                "store_remote_empty",
                StoreErrorOptions {
                    target: Some("store.metadata".into()),
                    fix: Some("Pass a clone URL: --remote <url>.".into()),
                },
            ));
        }
    }

    let store_root = resolve_setup_root(&id, input.path.as_deref())?;
    let kind = path_kind(&store_root);

    if kind == FILE || kind == OTHER {
        return Err(StoreError::new(
            format!(
                "Store setup path is not a directory: {}",
                store_root.display()
            ),
            "store_setup_path_not_directory",
            StoreErrorOptions {
                target: Some("store.root".into()),
                fix: Some("Choose an empty directory or an existing healthy Speckit root.".into()),
            },
        ));
    }

    if input.allow_inside_git_repository != Some(true) {
        // Simple heuristic: walk up from the store root's parent looking
        // for a .git directory.
        if let Some(parent) = store_root.parent() {
            if let Some(nearest) = nearest_existing_directory(parent) {
                let mut cur = nearest.clone();
                loop {
                    if is_git_repository_at_root(&cur)
                        && normalize_for_comparison(&cur) != normalize_for_comparison(&store_root)
                    {
                        return Err(StoreError::new(
                            format!(
                                "Store setup path is inside another Git repository: {}",
                                cur.display()
                            ),
                            "store_setup_inside_git_repo",
                            StoreErrorOptions {
                                target: Some("store.root".into()),
                                fix: Some("Choose a path outside that Git repository.".into()),
                            },
                        ));
                    }
                    match cur.parent() {
                        Some(p) if p != cur => cur = p.to_path_buf(),
                        _ => break,
                    }
                }
            }
        }
    }

    let mut backend: Option<StoreGitBackendConfig> = None;

    if kind == DIRECTORY {
        assert_not_config_only_pointer_root(&store_root)?;
        let metadata = read_store_metadata_for_operation(&store_root)?;

        if let Some(ref meta) = metadata {
            if meta.id != id {
                return Err(StoreError::new(
                    format!(
                        "Store metadata id '{}' does not match requested id '{}'.",
                        meta.id, id
                    ),
                    "store_metadata_id_mismatch",
                    StoreErrorOptions {
                        target: Some("store.metadata".into()),
                        fix: Some(format!(
                            "Use id '{}' or choose a different setup path.",
                            meta.id
                        )),
                    },
                ));
            }
            if input.remote.is_some() {
                return Err(remote_requires_hand_edit_error(&id, &store_root));
            }
        } else {
            let safe_fresh = is_directory_empty(&store_root) || is_git_only_directory(&store_root);
            if !safe_fresh {
                // A non-empty directory without metadata or a healthy root
                // is not safe to set up.
                return Err(StoreError::new(
                    "Store setup does not support initializing a non-empty folder that is not a healthy Speckit root.",
                    "store_setup_non_empty_directory",
                    StoreErrorOptions {
                        target: Some("store.root".into()),
                        fix: Some("Choose an empty folder, a Git-only folder, or an existing healthy Speckit root.".into()),
                    },
                ));
            }
        }

        backend = Some(resolve_backend_with_observed_origin(&store_root)?);
    }

    let registry = read_store_registry_state(&StorePathOptions::default())?;
    let conflict_backend = backend.clone().unwrap_or(StoreGitBackendConfig {
        backend_type: "git".into(),
        local_path: canonicalize_existing_path(&store_root),
        remote: None,
        branch: None,
    });

    assert_no_registered_store_conflict(
        &registry.clone().unwrap_or(StoreRegistryState {
            version: 1,
            stores: Default::default(),
        }),
        &id,
        &conflict_backend,
    )?;

    Ok(PreparedStoreSetup {
        id,
        root: store_root,
        root_kind: kind,
        backend,
        registry,
        remote: input.remote.clone(),
    })
}

/// Executes a prepared store setup plan.
pub fn setup_prepared_store(
    prepared: &PreparedStoreSetup,
    init_git: Option<bool>,
) -> Result<StoreMutationResult, StoreError> {
    let id = &prepared.id;
    let store_root = &prepared.root;
    let kind = prepared.root_kind;
    let registry = &prepared.registry;
    let git_enabled = init_git.unwrap_or_else(|| !is_registered_at_path(registry, id, store_root));
    let already_registered = is_registered_at_path(registry, id, store_root);

    // Re-assert that the path didn't appear while we waited.
    if kind == MISSING && store_root.exists() {
        return Err(StoreError::new(
            format!(
                "The path {} was created while setup was waiting for confirmation.",
                store_root.display()
            ),
            "store_setup_path_changed",
            StoreErrorOptions {
                target: Some("store.root".into()),
                fix: Some("Rerun speckit store setup to re-evaluate the directory.".into()),
            },
        ));
    }

    let repo_existed = is_git_repository_at_root(store_root);

    if git_enabled {
        let probe_cwd = nearest_existing_directory(store_root)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        assert_git_commit_identity(&probe_cwd)?;
    }

    let mut created_files: Vec<String> = Vec::new();
    let mut git_initialized = false;
    let mut committed = false;

    // Ensure the Speckit root directory structure exists.
    if kind == MISSING {
        fs::create_dir_all(store_root).map_err(|e| {
            StoreError::new(
                format!("Failed to create {}: {e}", store_root.display()),
                "store_setup_create_failed",
                StoreErrorOptions::default(),
            )
        })?;
    }

    // Create the .speckit-store metadata dir if missing.
    let metadata_dir = get_store_metadata_dir(store_root);
    if path_kind(&metadata_dir) == MISSING {
        fs::create_dir_all(&metadata_dir).map_err(|e| {
            StoreError::new(
                format!("Failed to create {}: {e}", metadata_dir.display()),
                "store_setup_metadata_dir_failed",
                StoreErrorOptions::default(),
            )
        })?;
        created_files.push(".speckit-store/".into());
    }

    // Write metadata.
    let existing_metadata = read_store_metadata_for_operation(store_root)?;
    if existing_metadata.is_some() && prepared.remote.is_some() {
        return Err(remote_requires_hand_edit_error(id, store_root));
    }
    if existing_metadata.is_none() {
        write_store_metadata_state(
            store_root,
            &StoreMetadataState {
                version: 1,
                id: id.clone(),
                remote: prepared.remote.clone(),
            },
        )?;
        created_files.push(".speckit-store/store.yaml".into());
    }

    // Git init + commit.
    if git_enabled {
        git_initialized = init_git_repository(store_root).unwrap_or(false);
    }
    let is_repository = git_initialized || repo_existed;

    let commit_pathspecs: Vec<&str> = if git_initialized {
        vec!["speckit", ".speckit-store"]
    } else {
        created_files
            .iter()
            .map(|s| s.as_str())
            .filter(|s| !s.ends_with('/'))
            .collect()
    };

    if git_enabled && is_repository && !commit_pathspecs.is_empty() {
        committed = commit_store_files(store_root, id, &commit_pathspecs).unwrap_or(false);
    }

    // Register.
    let backend = prepared
        .backend
        .clone()
        .unwrap_or_else(|| StoreGitBackendConfig {
            backend_type: "git".into(),
            local_path: canonicalize_existing_path(store_root),
            remote: None,
            branch: None,
        });

    let registered = commit_store_registration(&CommitStoreRegistrationInput {
        id: id.clone(),
        backend,
        write_metadata_if_missing: false,
        global_data_dir: None,
    })?;

    let diagnostics = if registered.already_registered && created_files.is_empty() {
        vec![already_registered_diagnostic(id)]
    } else {
        Vec::new()
    };

    let canonical = prepared
        .remote
        .clone()
        .or_else(|| existing_metadata.as_ref().and_then(|m| m.remote.clone()));
    let observed = prepared.backend.as_ref().and_then(|b| b.remote.clone());
    let remotes = if canonical.is_some() || observed.is_some() {
        Some(StoreRemotes {
            canonical,
            observed,
        })
    } else {
        None
    };

    Ok(mutation_payload(
        id,
        &registered.store_root,
        StoreMutationGit {
            is_repository,
            initialized: git_initialized,
            committed,
        },
        created_files,
        (registered.registry_updated, registered.already_registered),
        diagnostics,
        remotes,
    ))
}

/// Full setup flow: prepare and execute in one call.
pub fn setup_store(input: &SetupStoreInput) -> Result<StoreMutationResult, StoreError> {
    let prepared = prepare_store_setup(input)?;
    setup_prepared_store(&prepared, input.init_git)
}

// ---------------------------------------------------------------------------
// Public API: registerExistingStore
// ---------------------------------------------------------------------------

/// Registers an existing Speckit root as a store.
pub fn register_existing_store(
    input: &RegisterExistingStoreInput,
) -> Result<StoreMutationResult, StoreError> {
    let store_root = resolve_register_root(input.path.as_deref())?;
    let kind = path_kind(&store_root);

    if kind == MISSING {
        return Err(StoreError::new(
            format!("Store path does not exist: {}", store_root.display()),
            "store_path_missing",
            StoreErrorOptions {
                target: Some("store.root".into()),
                fix: Some("Clone or create the store folder before registering it.".into()),
            },
        ));
    }

    if kind != DIRECTORY {
        return Err(StoreError::new(
            format!("Store path is not a directory: {}", store_root.display()),
            "store_path_not_directory",
            StoreErrorOptions {
                target: Some("store.root".into()),
                fix: Some("Pass an existing store directory.".into()),
            },
        ));
    }

    assert_not_config_only_pointer_root(&store_root)?;

    let metadata = read_store_metadata_for_operation(&store_root)?;
    let explicit_id = match input.id.as_deref() {
        Some(raw) => Some(validate_store_id(raw)?),
        None => None,
    };

    if let (Some(meta), Some(explicit)) = (&metadata, &explicit_id) {
        if meta.id != *explicit {
            let current_registry = read_store_registry_state(&StorePathOptions::default())?;
            let registered_elsewhere = current_registry
                .as_ref()
                .and_then(|r| r.stores.get(&meta.id))
                .is_some();

            return Err(StoreError::new(
                format!(
                    "Store metadata id '{}' does not match --id '{}'. The id comes from the store's committed .speckit-store/store.yaml.",
                    meta.id, explicit
                ),
                "store_metadata_id_mismatch",
                StoreErrorOptions {
                    target: Some("store.id".into()),
                    fix: Some(if registered_elsewhere {
                        format!(
                            "One checkout per store id is supported, and '{}' is already registered. Run speckit store unregister {} first to register this checkout instead.",
                            meta.id, meta.id
                        )
                    } else {
                        format!("Use --id {} or register a different folder.", meta.id)
                    }),
                },
            ));
        }
    }

    let id = metadata
        .as_ref()
        .map(|m| m.id.clone())
        .or(explicit_id)
        .map(Ok)
        .unwrap_or_else(|| infer_store_id_from_path(&store_root))?;

    if metadata.is_none() && input.allow_create_identity != Some(true) {
        return Err(StoreError::new(
            format!("Turn this Speckit root into store '{id}'?"),
            "store_register_identity_confirmation_required",
            StoreErrorOptions {
                target: Some("store.metadata".into()),
                fix: Some(format!(
                    "Run interactively or pass --yes to create {}.",
                    get_store_metadata_path(&store_root).display()
                )),
            },
        ));
    }

    let backend = resolve_backend_with_observed_origin(&store_root)?;
    let registry = read_store_registry_state(&StorePathOptions::default())?;
    assert_no_registered_store_conflict(
        &registry.clone().unwrap_or(StoreRegistryState {
            version: 1,
            stores: Default::default(),
        }),
        &id,
        &backend,
    )?;

    let is_repository = is_git_repository_at_root(&store_root);
    let mut created_files: Vec<String> = Vec::new();

    let registered = commit_store_registration(&CommitStoreRegistrationInput {
        id: id.clone(),
        backend,
        write_metadata_if_missing: true,
        global_data_dir: None,
    })?;

    if registered.metadata_created {
        created_files.push(".speckit-store/store.yaml".into());
    }

    let diagnostics = if registered.already_registered && created_files.is_empty() {
        vec![already_registered_diagnostic(&id)]
    } else {
        Vec::new()
    };

    let canonical = metadata.as_ref().and_then(|m| m.remote.clone());
    let observed = registered.backend.remote.clone();
    let remotes = if canonical.is_some() || observed.is_some() {
        Some(StoreRemotes {
            canonical,
            observed,
        })
    } else {
        None
    };

    Ok(mutation_payload(
        &id,
        &registered.store_root,
        StoreMutationGit {
            is_repository,
            initialized: false,
            committed: false,
        },
        created_files,
        (registered.registry_updated, registered.already_registered),
        diagnostics,
        remotes,
    ))
}

// ---------------------------------------------------------------------------
// Public API: prepareStoreCleanup / unregisterStore / removeStore
// ---------------------------------------------------------------------------

/// Prepares cleanup metadata for a store.
pub fn prepare_store_cleanup(
    input: &CleanupStoreInput,
) -> Result<PreparedStoreCleanup, StoreError> {
    let id = validate_store_id(&input.id)?;
    let entry = get_registered_store(&crate::store::registry::GetRegisteredStoreInput {
        id: id.clone(),
        global_data_dir: input.global_data_dir.clone(),
        expected_backend: None,
    })?;

    Ok(PreparedStoreCleanup {
        id: entry.id,
        root: entry.store_root.clone(),
        metadata_path: get_store_metadata_path(&entry.store_root),
        backend: entry.backend,
        global_data_dir: input.global_data_dir.clone(),
    })
}

/// Unregisters a store (removes the registry entry only, files stay on disk).
pub fn unregister_store(input: &CleanupStoreInput) -> Result<StoreCleanupResult, StoreError> {
    let target = prepare_store_cleanup(input)?;
    let removed = unregister_store_registration(&UnregisterStoreInput {
        id: target.id.clone(),
        global_data_dir: target.global_data_dir.clone(),
        expected_backend: Some(target.backend.clone()),
    })?;

    Ok(StoreCleanupResult {
        store: mutate_info(&removed.id, &removed.store_root),
        registry_commit: StoreCleanupRegistryCommit {
            path: get_store_registry_path(target.global_data_dir.as_deref()),
            removed: true,
        },
        files: StoreCleanupFiles {
            deleted: false,
            deleted_path: None,
            left_on_disk: Some(removed.store_root),
        },
        diagnostics: Vec::new(),
    })
}

/// Removes a store: unregisters it and deletes the directory from disk.
pub fn remove_store(target: &PreparedStoreCleanup) -> Result<StoreCleanupResult, StoreError> {
    let id = validate_store_id(&target.id)?;
    let mut diagnostics: Vec<StoreDiagnostic> = Vec::new();
    let mut deleted = false;
    let mut root_missing = false;

    let removed = unregister_store_registration(&UnregisterStoreInput {
        id: id.clone(),
        global_data_dir: target.global_data_dir.clone(),
        expected_backend: Some(target.backend.clone()),
    })?;

    // Safety check: verify the root still looks like our store.
    let kind = path_kind(&removed.store_root);
    match kind {
        MISSING => {
            root_missing = true;
        }
        DIRECTORY => {
            let metadata = read_store_metadata_for_operation(&removed.store_root)?;
            if metadata.is_none() {
                return Err(StoreError::new(
                    "Store remove refuses to delete a folder without store metadata.",
                    "store_remove_metadata_missing",
                    StoreErrorOptions {
                        target: Some("store.metadata".into()),
                        fix: Some(
                            "Run \"speckit store unregister <id>\" if you only want to forget this local registry entry."
                                .into(),
                        ),
                    },
                ));
            }
            if let Some(ref meta) = metadata {
                if meta.id != id {
                    return Err(StoreError::new(
                        format!(
                            "Store metadata id '{}' does not match requested id '{}'.",
                            meta.id, id
                        ),
                        "store_metadata_id_mismatch",
                        StoreErrorOptions {
                            target: Some("store.metadata".into()),
                            fix: Some(
                                "Repair the registry or run store unregister instead of deleting this folder."
                                    .into(),
                            ),
                        },
                    ));
                }
            }
        }
        _ => {
            return Err(StoreError::new(
                format!(
                    "Store path is not a directory: {}",
                    removed.store_root.display()
                ),
                "store_remove_path_not_directory",
                StoreErrorOptions {
                    target: Some("store.root".into()),
                    fix: Some(
                        "Run \"speckit store unregister <id>\" if you only want to forget this local registry entry."
                            .into(),
                    ),
                },
            ));
        }
    }

    if root_missing {
        diagnostics.push(make_store_diagnostic(
            StoreDiagnosticSeverity::Warning,
            "store_root_missing",
            "Store files were already missing.",
            Some("store.root".into()),
            None,
        ));
    } else {
        match fs::remove_dir_all(&removed.store_root) {
            Ok(()) => {
                deleted = true;
            }
            Err(e) => {
                diagnostics.push(make_store_diagnostic(
                    StoreDiagnosticSeverity::Warning,
                    "store_files_left_on_disk",
                    format!(
                        "The registration was removed, but deleting {} failed ({e}).",
                        removed.store_root.display()
                    ),
                    Some("store.root".into()),
                    Some(format!(
                        "Delete the folder manually: {}",
                        removed.store_root.display()
                    )),
                ));
            }
        }
    }

    Ok(StoreCleanupResult {
        store: mutate_info(&removed.id, &removed.store_root),
        registry_commit: StoreCleanupRegistryCommit {
            path: get_store_registry_path(target.global_data_dir.as_deref()),
            removed: true,
        },
        files: StoreCleanupFiles {
            deleted,
            deleted_path: if deleted {
                Some(removed.store_root.clone())
            } else {
                None
            },
            left_on_disk: if !deleted && !root_missing {
                Some(removed.store_root)
            } else {
                None
            },
        },
        diagnostics,
    })
}

// ---------------------------------------------------------------------------
// Public API: listStores / doctorStores
// ---------------------------------------------------------------------------

/// Lists all registered stores.
pub fn list_stores() -> Result<StoreListResult, StoreError> {
    let entries = list_registered_stores(None)?;
    Ok(StoreListResult {
        stores: entries
            .into_iter()
            .map(|e| StoreInfo {
                id: e.id,
                root: e.store_root,
                metadata_path: None,
            })
            .collect(),
    })
}

fn inspect_store(entry_id: &str, backend: &StoreGitBackendConfig) -> StoreInspection {
    let root = get_store_root_for_backend(backend);
    let metadata_path = get_store_metadata_path(&root);
    let mut diagnostics: Vec<StoreDiagnostic> = Vec::new();

    let kind = path_kind(&root);
    let mut metadata = StoreInspectionMetadata {
        present: None,
        valid: None,
        id: None,
        remote: None,
    };
    let mut git_info = StoreInspectionGit {
        is_repository: None,
        has_commits: None,
        has_uncommitted_changes: None,
        has_remote: None,
        origin_url: None,
    };

    if kind == MISSING {
        diagnostics.push(make_store_diagnostic(
            StoreDiagnosticSeverity::Error,
            "store_root_missing",
            "Store location does not exist.",
            Some("store.root".into()),
            Some(format!(
                "Run speckit store register /path/to/{entry_id} --id {entry_id}."
            )),
        ));
    } else if kind != DIRECTORY {
        diagnostics.push(make_store_diagnostic(
            StoreDiagnosticSeverity::Error,
            "store_root_not_directory",
            "Store location is not a directory.",
            Some("store.root".into()),
            Some("Register a directory path for this store.".into()),
        ));
    } else {
        match read_optional_store_metadata_state(&root) {
            Ok(None) => {
                metadata = StoreInspectionMetadata {
                    present: Some(false),
                    valid: Some(false),
                    id: None,
                    remote: None,
                };
                diagnostics.push(make_store_diagnostic(
                    StoreDiagnosticSeverity::Error,
                    "store_metadata_missing",
                    "Store metadata is missing.",
                    Some("store.metadata".into()),
                    Some(format!(
                        "Create {} or rerun store register.",
                        metadata_path.display()
                    )),
                ));
            }
            Ok(Some(parsed)) => {
                if parsed.id != entry_id {
                    metadata = StoreInspectionMetadata {
                        present: Some(true),
                        valid: Some(false),
                        id: Some(parsed.id.clone()),
                        remote: None,
                    };
                    diagnostics.push(make_store_diagnostic(
                        StoreDiagnosticSeverity::Error,
                        "store_metadata_id_mismatch",
                        format!(
                            "Store metadata id '{}' does not match registry id '{}'.",
                            parsed.id, entry_id
                        ),
                        Some("store.metadata".into()),
                        Some(
                            "Repair the local registry or store metadata so the ids match.".into(),
                        ),
                    ));
                } else {
                    metadata = StoreInspectionMetadata {
                        present: Some(true),
                        valid: Some(true),
                        id: Some(parsed.id),
                        remote: parsed.remote,
                    };
                }
            }
            Err(e) => {
                metadata = StoreInspectionMetadata {
                    present: Some(true),
                    valid: Some(false),
                    id: None,
                    remote: None,
                };
                diagnostics.push(doctor_status_for_error(
                    &e,
                    "store_metadata_invalid",
                    "store.metadata",
                    Some(&format!("Repair {}.", metadata_path.display())),
                ));
            }
        }

        let is_repo = is_git_repository_at_root(&root);
        git_info = StoreInspectionGit {
            is_repository: Some(is_repo),
            has_commits: None,
            has_uncommitted_changes: None,
            has_remote: None,
            origin_url: None,
        };

        if is_repo {
            git_info.has_commits = git_has_commits(&root);
            git_info.has_uncommitted_changes = git_has_uncommitted_changes(&root);
            git_info.has_remote = git_has_remote(&root);
            git_info.origin_url = git_origin_url(&root);

            if git_info.has_commits == Some(false) {
                diagnostics.push(make_store_diagnostic(
                    StoreDiagnosticSeverity::Warning,
                    "store_git_no_commits",
                    "Git repository has no commits yet; clones of this store will be empty until an initial commit exists.",
                    Some("store.git".into()),
                    Some("Commit the store files, then push to share them.".into()),
                ));
            } else if git_info.has_commits == Some(true) {
                // Check for fragile directories.
                let anchored = vec!["speckit/specs", "speckit/changes/archive"];
                let mut fragile: Vec<String> = Vec::new();
                for rel_dir in anchored {
                    let dir = root.join(rel_dir);
                    if path_kind(&dir) == DIRECTORY {
                        if git_directory_has_tracked_files(&root, rel_dir) == Some(false) {
                            fragile.push(format!("{rel_dir}/"));
                        }
                    }
                }
                if !fragile.is_empty() {
                    diagnostics.push(make_store_diagnostic(
                        StoreDiagnosticSeverity::Warning,
                        "store_clone_fragile_directories",
                        format!(
                            "These directories contain no tracked files and will be lost in clones: {}.",
                            fragile.join(", ")
                        ),
                        Some("store.git".into()),
                        Some("Track a file in each directory (for example .gitkeep) and commit it.".into()),
                    ));
                }
            }
        }
    }

    StoreInspection {
        id: entry_id.to_string(),
        root,
        metadata_path,
        metadata,
        git: git_info,
        diagnostics,
    }
}

/// Runs the doctor diagnostic on all registered stores, or a single one.
pub fn doctor_stores(selected_id: Option<&str>) -> Result<StoreDoctorResult, StoreError> {
    let registry = read_store_registry_state(&StorePathOptions::default())?;

    let reg = match registry {
        None => {
            if let Some(id) = selected_id {
                return Err(StoreError::new(
                    format!("Unknown store '{id}'."),
                    "store_not_found",
                    StoreErrorOptions {
                        target: Some("store.id".into()),
                        fix: Some("Run speckit store list to see registered stores.".into()),
                    },
                ));
            }
            return Ok(StoreDoctorResult {
                stores: Vec::new(),
                diagnostics: Vec::new(),
            });
        }
        Some(r) => r,
    };

    let entries = list_store_registry_entries(&reg);
    let selected: Vec<_> = match selected_id {
        Some(id) => {
            let filtered: Vec<_> = entries.iter().filter(|e| e.id == id).collect();
            if filtered.is_empty() {
                return Err(StoreError::new(
                    format!("Unknown store '{id}'."),
                    "store_not_found",
                    StoreErrorOptions {
                        target: Some("store.id".into()),
                        fix: Some("Run speckit store list to see registered stores.".into()),
                    },
                ));
            }
            filtered
        }
        None => entries.iter().collect(),
    };

    let stores: Vec<StoreInspection> = selected
        .iter()
        .map(|entry| inspect_store(&entry.id, &entry.backend))
        .collect();

    Ok(StoreDoctorResult {
        stores,
        diagnostics: Vec::new(),
    })
}

/// Normalizes a store path for comparison.
pub fn normalize_store_path_for_comparison(target: &Path) -> PathBuf {
    canonicalize_existing_path(target)
}
