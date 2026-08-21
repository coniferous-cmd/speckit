//! The relationship-data gather shared by doctor and context: one
//! registry snapshot, the health-mode reference index, and the root
//! inspection. Doctor layers its health-only inputs (store facts,
//! wrong-turn detection) on top.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use crate::shared_output::StoreDiagnostic;

/// A snapshot of the store registry: the registered store entries and
/// whether the registry file itself was readable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySnapshot {
    pub entries: Vec<RegistryEntry>,
    pub unreadable: bool,
}

/// A single store entry in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub id: String,
    pub root: String,
}

/// The project-level config (references, default schema, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    #[serde(default)]
    pub references: Vec<ReferenceDeclaration>,
    #[serde(default)]
    pub default_schema: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// A reference declaration in the project config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceDeclaration {
    pub store_id: String,
    #[serde(default)]
    pub path: Option<String>,
}

/// An entry in the assembled reference index (resolved from declarations + registry).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceIndexEntry {
    pub store_id: String,
    pub root: Option<String>,
    #[serde(default)]
    pub status: Vec<StoreDiagnostic>,
}

/// Inspection result of an Speckit root directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeckitRootInspection {
    pub healthy: bool,
    pub diagnostics: Vec<StoreDiagnostic>,
}

/// The complete relationship data bundle gathered for doctor/context.
#[derive(Debug, Clone)]
pub struct RelationshipData {
    pub registry_snapshot: RegistrySnapshot,
    pub project_config: Option<ProjectConfig>,
    pub store_config_path: String,
    pub reference_entries: Vec<ReferenceIndexEntry>,
    pub root_inspection: SpeckitRootInspection,
}

/// Read the store registry file. Returns a snapshot with entries and
/// an unreadable flag if the file cannot be parsed.
pub async fn read_registry_snapshot() -> RegistrySnapshot {
    // Locate the registry file: ~/.config/speckit/store-registry.json (XDG-aware)
    let registry_path = dirs::config_dir()
        .map(|p| p.join("speckit").join("store-registry.json"))
        .unwrap_or_else(|| PathBuf::from(".speckit/store-registry.json"));

    let content = match tokio::fs::read_to_string(&registry_path).await {
        Ok(c) => c,
        Err(_) => {
            return RegistrySnapshot {
                entries: Vec::new(),
                unreadable: true,
            };
        }
    };

    let entries: Vec<RegistryEntry> = serde_json::from_str(&content).unwrap_or_default();
    RegistrySnapshot {
        entries,
        unreadable: false,
    }
}

/// Read the project config from `<root>/speckit/config.yaml`.
pub fn read_project_config(project_root: &str) -> Option<ProjectConfig> {
    let config_path = std::path::Path::new(project_root)
        .join("speckit")
        .join("config.yaml");
    let content = std::fs::read_to_string(&config_path).ok()?;
    serde_yaml::from_str(&content).ok()
}

/// Resolve the config file path for the project.
pub fn resolve_config_file_path(project_root: &str) -> Option<String> {
    let config_path = std::path::Path::new(project_root)
        .join("speckit")
        .join("config.yaml");
    if config_path.exists() {
        Some(config_path.to_string_lossy().to_string())
    } else {
        None
    }
}

/// Assemble a reference index from declared references and the registry.
pub async fn assemble_reference_index(
    references: &[ReferenceDeclaration],
    registry_entries: &[RegistryEntry],
    project_root: &str,
) -> Vec<ReferenceIndexEntry> {
    let mut entries = Vec::new();
    for decl in references {
        // Skip self-references
        if let Some(reg_entry) = registry_entries.iter().find(|e| e.id == decl.store_id) {
            // If the resolved root is the same as this project root, omit it
            if reg_entry.root == project_root {
                continue;
            }
            entries.push(ReferenceIndexEntry {
                store_id: decl.store_id.clone(),
                root: Some(reg_entry.root.clone()),
                status: Vec::new(),
            });
        } else {
            entries.push(ReferenceIndexEntry {
                store_id: decl.store_id.clone(),
                root: decl.path.clone(),
                status: vec![StoreDiagnostic {
                    severity: "warning".to_string(),
                    code: "reference_not_registered".to_string(),
                    message: format!("Store '{}' is not registered", decl.store_id),
                    fix: Some(format!(
                        "speckit store register <path> --id {}",
                        decl.store_id
                    )),
                }],
            });
        }
    }
    entries
}

/// Inspect an Speckit root: check for project.md, specs/, changes/, etc.
pub async fn inspect_speckit_root(project_root: &str) -> SpeckitRootInspection {
    let root_path = std::path::Path::new(project_root);
    let speckit_dir = root_path.join("speckit");
    let project_md = speckit_dir.join("project.md");
    let specs_dir = speckit_dir.join("specs");
    let changes_dir = speckit_dir.join("changes");

    let mut diagnostics = Vec::new();
    let mut healthy = true;

    if !speckit_dir.is_dir() {
        diagnostics.push(StoreDiagnostic {
            severity: "error".to_string(),
            code: "speckit_dir_missing".to_string(),
            message: "speckit/ directory not found".to_string(),
            fix: Some("Run `speckit init` to initialize this project.".to_string()),
        });
        healthy = false;
    } else {
        if !project_md.is_file() {
            diagnostics.push(StoreDiagnostic {
                severity: "warning".to_string(),
                code: "project_md_missing".to_string(),
                message: "project.md not found in speckit/".to_string(),
                fix: None,
            });
        }
        if !specs_dir.is_dir() {
            diagnostics.push(StoreDiagnostic {
                severity: "warning".to_string(),
                code: "specs_dir_missing".to_string(),
                message: "specs/ directory not found".to_string(),
                fix: None,
            });
        }
        if !changes_dir.is_dir() {
            diagnostics.push(StoreDiagnostic {
                severity: "warning".to_string(),
                code: "changes_dir_missing".to_string(),
                message: "changes/ directory not found".to_string(),
                fix: None,
            });
        }
    }

    SpeckitRootInspection {
        healthy,
        diagnostics,
    }
}

/// Gather all relationship data for doctor/context commands.
pub async fn gather_relationship_data(project_root: &str) -> RelationshipData {
    let registry_snapshot = read_registry_snapshot().await;
    let project_config = read_project_config(project_root);
    let store_config_path = resolve_config_file_path(project_root).unwrap_or_else(|| {
        std::path::Path::new(project_root)
            .join("speckit")
            .join("config.yaml")
            .to_string_lossy()
            .to_string()
    });

    let references = project_config
        .as_ref()
        .map(|c| c.references.clone())
        .unwrap_or_default();

    let reference_entries =
        assemble_reference_index(&references, &registry_snapshot.entries, project_root).await;

    let root_inspection = inspect_speckit_root(project_root).await;

    RelationshipData {
        registry_snapshot,
        project_config,
        store_config_path,
        reference_entries,
        root_inspection,
    }
}
