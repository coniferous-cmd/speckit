//! Doctor Command
//!
//! Report relationship health for the resolved Speckit root.

use serde::{Deserialize, Serialize};

use crate::shared_gather::{ReferenceIndexEntry, StoreDiagnostic, gather_relationship_data};
use crate::shared_output::print_json;

/// Health report for the Speckit root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipHealth {
    pub root: RootHealth,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<StoreHealth>,
    pub references: Vec<ReferenceHealth>,
    pub status: Vec<StoreDiagnostic>,
}

/// Root health information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootHealth {
    pub path: String,
    pub healthy: bool,
    pub status: Vec<StoreDiagnostic>,
}

/// Store health information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreHealth {
    pub id: String,
    pub metadata: MetadataHealth,
    pub status: Vec<StoreDiagnostic>,
}

/// Metadata health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataHealth {
    pub valid: bool,
}

/// Reference health information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceHealth {
    pub store_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    pub status: Vec<StoreDiagnostic>,
}

/// Execute the doctor command.
pub async fn doctor_command(store: Option<&str>, json: bool) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?.to_string_lossy().to_string();

    let data = gather_relationship_data(&project_root).await;

    let root_health = RootHealth {
        path: project_root.clone(),
        healthy: data.root_inspection.healthy,
        status: data.root_inspection.diagnostics.clone(),
    };

    let references: Vec<ReferenceHealth> = data
        .reference_entries
        .iter()
        .map(|entry| ReferenceHealth {
            store_id: entry.store_id.clone(),
            root: entry.root.clone(),
            status: entry.status.clone(),
        })
        .collect();

    let health = RelationshipHealth {
        root: root_health,
        store: None,
        references,
        status: Vec::new(),
    };

    if json {
        print_json(&health);
        return Ok(());
    }

    print_human_health(&health, &data.project_config);
    Ok(())
}

/// Print health in human-readable format.
fn print_human_health(
    health: &RelationshipHealth,
    project_config: &Option<crate::shared_gather::ProjectConfig>,
) {
    println!("Doctor");
    println!();
    println!("Root");
    println!("  Location: {}", health.root.path);
    println!(
        "  Speckit root: {}",
        if health.root.healthy {
            "ok"
        } else {
            "unhealthy"
        }
    );

    if let Some(ref store) = health.store {
        let metadata_note = if store.metadata.valid {
            "metadata ok"
        } else {
            "metadata invalid"
        };
        println!("  Store: {} ({})", store.id, metadata_note);
    }

    for status in &health.root.status {
        println!("  - {}", status.message);
        if let Some(ref fix) = status.fix {
            println!("    Fix: {fix}");
        }
    }

    let declared_ref_count = project_config
        .as_ref()
        .map(|c| c.references.len())
        .unwrap_or(0);

    println!();
    println!("References");
    if health.references.is_empty() {
        if declared_ref_count > 0 {
            println!("  (declared references all resolve to this root)");
        } else {
            println!("  (none declared)");
        }
    } else {
        for entry in &health.references {
            if entry.status.is_empty() {
                let root_label = entry.root.as_deref().unwrap_or("unknown");
                println!("  - {}: ok ({})", entry.store_id, root_label);
            } else {
                for diagnostic in &entry.status {
                    println!("  - {}: {}", entry.store_id, diagnostic.message);
                    if let Some(ref fix) = diagnostic.fix {
                        println!("    Fix: {fix}");
                    }
                }
            }
        }
    }

    for status in &health.status {
        println!();
        println!("Note: {}", status.message);
        if let Some(ref fix) = status.fix {
            println!("Fix: {fix}");
        }
    }
}
