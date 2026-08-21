use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::store::errors::{StoreDiagnostic, StoreDiagnosticSeverity, make_store_diagnostic};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const OPENSPEC_ROOT_DIR: &str = "speckit";
pub const OPENSPEC_CONFIG_YAML: &str = "speckit/config.yaml";
pub const OPENSPEC_CONFIG_YML: &str = "speckit/config.yml";
pub const OPENSPEC_SPECS_DIR: &str = "speckit/specs";
pub const OPENSPEC_CHANGES_DIR: &str = "speckit/changes";
pub const OPENSPEC_ARCHIVE_DIR: &str = "speckit/changes/archive";
pub const DEFAULT_OPENSPEC_SCHEMA: &str = "spec-driven";
pub const DIRECTORY_ANCHOR_FILE_NAME: &str = ".gitkeep";

/// Directories that receive a `.gitkeep` anchor when empty, so Git clones
/// preserve the directory structure.
pub const ANCHORED_OPENSPEC_DIRS: [&str; 2] = [OPENSPEC_SPECS_DIR, OPENSPEC_ARCHIVE_DIR];

// ---------------------------------------------------------------------------
// Path-kind helper
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
        Err(e) => panic!("Unexpected stat error on {}: {e}", path.display()),
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CreatedPathKind {
    Directory,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedPathLedgerEntry {
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub kind: CreatedPathKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionPresence {
    pub present: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl Default for InspectionPresence {
    fn default() -> Self {
        Self {
            present: None,
            path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeckitRootInspection {
    pub present: Option<bool>,
    pub config: InspectionPresence,
    pub specs: InspectionPresence,
    pub changes: InspectionPresence,
    pub archive: InspectionPresence,
    pub healthy: bool,
    pub diagnostics: Vec<StoreDiagnostic>,
}

impl Default for SpeckitRootInspection {
    fn default() -> Self {
        Self {
            present: None,
            config: InspectionPresence::default(),
            specs: InspectionPresence::default(),
            changes: InspectionPresence::default(),
            archive: InspectionPresence::default(),
            healthy: false,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsureSpeckitRootResult {
    pub inspection: SpeckitRootInspection,
    pub created_artifacts: Vec<String>,
    pub created_paths: Vec<CreatedPathLedgerEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct EnsureSpeckitRootOptions {
    pub anchor_empty_directories: bool,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn relative_artifact(relative_path: &str, kind: &CreatedPathKind) -> String {
    let normalized = relative_path.replace('\\', "/");
    match kind {
        CreatedPathKind::Directory => format!("{normalized}/"),
        CreatedPathKind::File => normalized,
    }
}

fn unresolved_inspection() -> SpeckitRootInspection {
    SpeckitRootInspection::default()
}

fn missing_directory_diagnostic(code: &str, message: &str, target: &str) -> StoreDiagnostic {
    make_store_diagnostic(
        StoreDiagnosticSeverity::Error,
        code,
        message,
        Some(target.into()),
        None,
    )
}

fn inspect_optional_planning_directory(
    inspection: &mut SpeckitRootInspection,
    store_root: &Path,
    key: &str,
    relative_path: &str,
    not_directory_code: &str,
    target: &str,
) -> PathKind {
    let kind = path_kind(&store_root.join(relative_path));
    let present = kind == DIRECTORY;

    match key {
        "specs" => inspection.specs.present = Some(present),
        "changes" => inspection.changes.present = Some(present),
        "archive" => inspection.archive.present = Some(present),
        _ => {}
    }

    if kind == DIRECTORY || kind == MISSING {
        return kind;
    }

    inspection.diagnostics.push(missing_directory_diagnostic(
        not_directory_code,
        &format!("{relative_path}/ exists but is not a directory."),
        target,
    ));
    kind
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Inspects an Speckit root directory, reporting the presence and health
/// of the `speckit/` tree, config, specs, changes, and archive directories.
pub fn inspect_speckit_root(store_root: &Path) -> SpeckitRootInspection {
    let root_kind = path_kind(store_root);
    let mut inspection = unresolved_inspection();

    if root_kind == MISSING {
        inspection.diagnostics.push(missing_directory_diagnostic(
            "speckit_store_root_missing",
            "Store root does not exist.",
            "store.root",
        ));
        return inspection;
    }

    if root_kind != DIRECTORY {
        inspection.diagnostics.push(missing_directory_diagnostic(
            "speckit_store_root_not_directory",
            "Store root is not a directory.",
            "store.root",
        ));
        return inspection;
    }

    let speckit_path = store_root.join(OPENSPEC_ROOT_DIR);
    let speckit_kind = path_kind(&speckit_path);
    inspection.present = Some(speckit_kind == DIRECTORY);

    if speckit_kind == MISSING {
        inspection.diagnostics.push(missing_directory_diagnostic(
            "speckit_root_missing",
            "Missing speckit/ directory.",
            "speckit.root",
        ));
        return inspection;
    }

    if speckit_kind != DIRECTORY {
        inspection.diagnostics.push(missing_directory_diagnostic(
            "speckit_root_not_directory",
            "speckit/ exists but is not a directory.",
            "speckit.root",
        ));
        return inspection;
    }

    // Config check
    let config_yaml_kind = path_kind(&store_root.join(OPENSPEC_CONFIG_YAML));
    let config_yml_kind = path_kind(&store_root.join(OPENSPEC_CONFIG_YML));

    if config_yaml_kind == FILE {
        inspection.config = InspectionPresence {
            present: Some(true),
            path: Some(OPENSPEC_CONFIG_YAML.into()),
        };
    } else if config_yml_kind == FILE {
        inspection.config = InspectionPresence {
            present: Some(true),
            path: Some(OPENSPEC_CONFIG_YML.into()),
        };
    } else {
        inspection.config = InspectionPresence {
            present: Some(false),
            path: None,
        };
        if config_yaml_kind != MISSING || config_yml_kind != MISSING {
            inspection.diagnostics.push(missing_directory_diagnostic(
                "speckit_config_not_file",
                "Speckit config path exists but is not a file.",
                "speckit.config",
            ));
        } else {
            inspection.diagnostics.push(missing_directory_diagnostic(
                "speckit_config_missing",
                "Missing speckit/config.yaml or speckit/config.yml.",
                "speckit.config",
            ));
        }
    }

    // Planning directories
    inspect_optional_planning_directory(
        &mut inspection,
        store_root,
        "specs",
        OPENSPEC_SPECS_DIR,
        "speckit_specs_not_directory",
        "speckit.specs",
    );
    let changes_kind = inspect_optional_planning_directory(
        &mut inspection,
        store_root,
        "changes",
        OPENSPEC_CHANGES_DIR,
        "speckit_changes_not_directory",
        "speckit.changes",
    );
    if changes_kind == DIRECTORY {
        inspect_optional_planning_directory(
            &mut inspection,
            store_root,
            "archive",
            OPENSPEC_ARCHIVE_DIR,
            "speckit_archive_not_directory",
            "speckit.archive",
        );
    } else {
        inspection.archive.present = Some(false);
    }

    inspection.healthy = inspection.present == Some(true)
        && inspection.config.present == Some(true)
        && inspection.diagnostics.is_empty();

    inspection
}

fn ensure_directory(
    store_root: &Path,
    relative_path: &str,
    ledger: &mut Vec<CreatedPathLedgerEntry>,
) -> Result<(), String> {
    let absolute_path = store_root.join(relative_path);
    let kind = path_kind(&absolute_path);

    if kind == DIRECTORY {
        return Ok(());
    }
    if kind != MISSING {
        return Err(format!("{relative_path}/ exists but is not a directory."));
    }

    fs::create_dir_all(&absolute_path)
        .map_err(|e| format!("Failed to create {}: {e}", absolute_path.display()))?;

    ledger.push(CreatedPathLedgerEntry {
        relative_path: relative_artifact(relative_path, &CreatedPathKind::Directory),
        absolute_path,
        kind: CreatedPathKind::Directory,
    });

    Ok(())
}

fn ensure_default_config(
    store_root: &Path,
    ledger: &mut Vec<CreatedPathLedgerEntry>,
) -> Result<(), String> {
    let config_yaml_path = store_root.join(OPENSPEC_CONFIG_YAML);
    let config_yml_path = store_root.join(OPENSPEC_CONFIG_YML);
    let yaml_kind = path_kind(&config_yaml_path);
    let yml_kind = path_kind(&config_yml_path);

    if yaml_kind == FILE || yml_kind == FILE {
        return Ok(());
    }
    if yaml_kind != MISSING || yml_kind != MISSING {
        return Err("Speckit config path exists but is not a file.".into());
    }

    let default_config = format!("schema: {DEFAULT_OPENSPEC_SCHEMA}\n");
    fs::write(&config_yaml_path, &default_config)
        .map_err(|e| format!("Failed to write {}: {e}", config_yaml_path.display()))?;

    ledger.push(CreatedPathLedgerEntry {
        relative_path: relative_artifact(OPENSPEC_CONFIG_YAML, &CreatedPathKind::File),
        absolute_path: config_yaml_path,
        kind: CreatedPathKind::File,
    });

    Ok(())
}

fn ensure_directory_anchor(
    store_root: &Path,
    relative_dir: &str,
    ledger: &mut Vec<CreatedPathLedgerEntry>,
) -> Result<(), String> {
    let directory = store_root.join(relative_dir);
    let entries = fs::read_dir(&directory)
        .map_err(|e| format!("Failed to read {}: {e}", directory.display()))?;

    if entries.count() > 0 {
        return Ok(());
    }

    let relative_path = format!("{relative_dir}/{DIRECTORY_ANCHOR_FILE_NAME}");
    let absolute_path = directory.join(DIRECTORY_ANCHOR_FILE_NAME);
    fs::write(&absolute_path, "")
        .map_err(|e| format!("Failed to write {}: {e}", absolute_path.display()))?;

    ledger.push(CreatedPathLedgerEntry {
        relative_path: relative_artifact(&relative_path, &CreatedPathKind::File),
        absolute_path,
        kind: CreatedPathKind::File,
    });

    Ok(())
}

/// Ensures a complete Speckit root exists at `store_root`, creating any
/// missing directories and default config. Returns the inspection result
/// and a ledger of created artifacts.
pub fn ensure_speckit_root(
    store_root: &Path,
    options: &EnsureSpeckitRootOptions,
) -> Result<EnsureSpeckitRootResult, String> {
    let mut ledger: Vec<CreatedPathLedgerEntry> = Vec::new();
    let root_kind = path_kind(store_root);

    if root_kind == MISSING {
        fs::create_dir_all(store_root)
            .map_err(|e| format!("Failed to create {}: {e}", store_root.display()))?;
    } else if root_kind != DIRECTORY {
        return Err("Store root is not a directory.".into());
    }

    ensure_directory(store_root, OPENSPEC_ROOT_DIR, &mut ledger)?;
    ensure_directory(store_root, OPENSPEC_SPECS_DIR, &mut ledger)?;
    ensure_directory(store_root, OPENSPEC_CHANGES_DIR, &mut ledger)?;
    ensure_directory(store_root, OPENSPEC_ARCHIVE_DIR, &mut ledger)?;
    ensure_default_config(store_root, &mut ledger)?;

    if options.anchor_empty_directories {
        for rel_dir in ANCHORED_OPENSPEC_DIRS {
            ensure_directory_anchor(store_root, rel_dir, &mut ledger)?;
        }
    }

    Ok(EnsureSpeckitRootResult {
        inspection: inspect_speckit_root(store_root),
        created_artifacts: ledger.iter().map(|e| e.relative_path.clone()).collect(),
        created_paths: ledger,
    })
}

/// Rollbacks created paths in reverse order (files first, then directories).
pub fn rollback_created_paths(entries: &[CreatedPathLedgerEntry]) {
    for entry in entries.iter().rev() {
        match entry.kind {
            CreatedPathKind::File => {
                let _ = fs::remove_file(&entry.absolute_path);
            }
            CreatedPathKind::Directory => {
                let _ = fs::remove_dir(&entry.absolute_path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn inspect_missing_root() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("nope");
        let result = inspect_speckit_root(&missing);
        assert!(!result.healthy);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "speckit_store_root_missing");
    }

    #[test]
    fn ensure_root_creates_structure() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("my-store");
        let result = ensure_speckit_root(&root, &EnsureSpeckitRootOptions::default()).unwrap();
        assert!(result.inspection.healthy);
        assert!(root.join("speckit/specs").is_dir());
        assert!(root.join("speckit/changes").is_dir());
        assert!(root.join("speckit/changes/archive").is_dir());
        assert!(root.join("speckit/config.yaml").is_file());
    }

    #[test]
    fn ensure_root_with_anchors() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("my-store");
        let result = ensure_speckit_root(
            &root,
            &EnsureSpeckitRootOptions {
                anchor_empty_directories: true,
            },
        )
        .unwrap();
        assert!(result.inspection.healthy);
        assert!(root.join("speckit/specs/.gitkeep").is_file());
        assert!(root.join("speckit/changes/archive/.gitkeep").is_file());
    }
}
