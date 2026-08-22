//! Doctor Command
//!
//! Report relationship health for the resolved Speckit root.

use serde::{Deserialize, Serialize};
use speckit_core::root_selection::{ResolveSpeckitRootOptions, resolve_speckit_root};

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
///
/// When `store` is `Some`, the unified root resolver picks that store and
/// the report's root/store sections reflect the resolved location.  When
/// `store` is `None`, the working-directory root is used as before.
pub async fn doctor_command(store: Option<&str>, json: bool) -> anyhow::Result<()> {
    let project_root_str = std::env::current_dir()?.to_string_lossy().to_string();

    // Resolve the Speckit root up-front so that an invalid store surfaces as
    // a non-zero exit with a stable error code instead of being silently
    // overridden by the working-directory fallback.
    let resolved_root = match resolve_speckit_root(&ResolveSpeckitRootOptions {
        store: store.map(|s| s.to_string()),
        store_path: None,
        start_path: Some(std::path::PathBuf::from(&project_root_str)),
        allow_implicit_root: Some(true),
        global_data_dir: None,
    }) {
        Ok(root) => root,
        Err(err) => {
            let diag = &err.diagnostic;
            if json {
                print_json(&serde_json::json!({
                    "status": [{
                        "severity": "error",
                        "code": diag.code,
                        "message": diag.message,
                        "fix": diag.fix,
                    }]
                }));
            } else {
                eprintln!("Error: {}", diag.message);
                if let Some(fix) = diag.fix.as_deref() {
                    eprintln!("Fix: {fix}");
                }
            }
            return Err(anyhow::anyhow!(diag.message.clone()));
        }
    };

    let resolved_root_str = resolved_root.path.to_string_lossy().to_string();

    let data = gather_relationship_data(&resolved_root_str).await;

    let root_health = RootHealth {
        path: resolved_root_str.clone(),
        healthy: data.root_inspection.healthy,
        status: data.root_inspection.diagnostics.clone(),
    };

    // Use the store_id from the resolved root so JSON output reflects the
    // actual store that was selected (not the raw CLI argument).
    let store_health = resolved_root.store_id.as_ref().map(|id| StoreHealth {
        id: id.clone(),
        metadata: MetadataHealth { valid: true },
        status: Vec::new(),
    });

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
        store: store_health,
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
