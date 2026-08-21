pub mod errors;
pub mod foundation;
pub mod git;
pub mod operations;
pub mod registry;

// Re-export key types and functions for convenience.
pub use errors::{
    RootSelectionError, StoreDiagnostic, StoreDiagnosticSeverity, StoreError, StoreErrorOptions,
    make_store_diagnostic,
};
pub use foundation::{
    KEBAB_ID_DESCRIPTION, KEBAB_ID_FIX, ResolveGitStoreBackendInput, STORE_METADATA_DIR_NAME,
    STORE_METADATA_FILE_NAME, STORE_REGISTRY_FILE_NAME, STORES_DIR_NAME, StoreBackendConfig,
    StoreGitBackendConfig, StoreMetadataState, StorePathOptions, StoreRegistryEntry,
    StoreRegistryEntryState, StoreRegistryState, canonicalize_existing_path,
    folder_style_name_problem, get_store_metadata_dir, get_store_metadata_path,
    get_store_registry_path, get_store_root_for_backend, get_stores_dir, is_kebab_id,
    is_store_root, is_valid_store_id, list_store_registry_entries, parse_store_metadata_state,
    parse_store_registry_state, read_optional_store_metadata_state, read_store_metadata_state,
    read_store_registry_state, resolve_git_store_backend_config, serialize_store_metadata_state,
    serialize_store_registry_state, update_store_registry_state, validate_store_id,
    write_store_metadata_state, write_store_registry_state,
};
pub use git::{
    GitTrackingDrift, assert_git_commit_identity, commit_store_files,
    git_directory_has_tracked_files, git_has_commits, git_has_remote, git_has_uncommitted_changes,
    git_origin_url, git_tracking_drift, init_git_repository, is_git_repository_at_root,
};
pub use operations::{
    CleanupStoreInput, PreparedStoreCleanup, PreparedStoreSetup, RegisterExistingStoreInput,
    SetupStoreInput, StoreCleanupFiles, StoreCleanupRegistryCommit, StoreCleanupResult,
    StoreDoctorResult, StoreInfo, StoreInspection, StoreInspectionGit, StoreInspectionMetadata,
    StoreListResult, StoreMutationGit, StoreMutationRegistryCommit, StoreMutationResult,
    StoreRemotes, doctor_stores, expand_user, list_stores, normalize_store_path_for_comparison,
    prepare_store_cleanup, prepare_store_setup, register_existing_store, remove_store,
    resolve_setup_git_enabled, setup_prepared_store, setup_store, unregister_store,
};
pub use registry::{
    CommitStoreRegistrationInput, GetRegisteredStoreInput, RegisterStoreInput,
    RegisteredStoreEntry, RegistrySnapshot, ResolveRegisteredStoreInput, ResolvedStore,
    StoreRegistrationCommit, UnregisterStoreInput, assert_no_registered_store_conflict,
    commit_store_registration, get_registered_store, get_store_root, list_registered_stores,
    read_registry_snapshot, register_store, resolve_registered_store,
    unregister_store_registration,
};
