//! Context Command
//!
//! Print the working context for the resolved Speckit root.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::shared_gather::{
    ReferenceIndexEntry, assemble_reference_index, gather_relationship_data, read_project_config,
    read_registry_snapshot,
};
use crate::shared_output::{StoreDiagnostic, emit_failure, print_json};

/// Working set: the complete context for a root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingSet {
    pub root: WorkingSetRoot,
    pub members: Vec<WorkingSetMember>,
    pub status: Vec<StoreDiagnostic>,
}

/// The root of the working set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingSetRoot {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_id: Option<String>,
}

/// A member of the working set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingSetMember {
    pub id: String,
    pub path: String,
    pub role: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch: Option<String>,
    pub status: Vec<StoreDiagnostic>,
}

/// Execute the context command.
pub async fn context_command(
    store: Option<&str>,
    json: bool,
    code_workspace: Option<&str>,
    force: bool,
) -> anyhow::Result<()> {
    let project_root = crate::change::resolve_project_root(store).await?;

    let data = gather_relationship_data(&project_root).await;

    // Build the working set
    let mut members = Vec::new();

    for entry in &data.reference_entries {
        let available = entry
            .root
            .as_ref()
            .map_or(false, |root| Path::new(root).join("speckit").is_dir());

        members.push(WorkingSetMember {
            id: entry.store_id.clone(),
            path: entry.root.clone().unwrap_or_default(),
            role: "referenced_store".to_string(),
            available,
            fetch: None,
            status: entry.status.clone(),
        });
    }

    let working_set = WorkingSet {
        root: WorkingSetRoot {
            path: project_root.clone(),
            store_id: store.map(|s| s.to_string()),
        },
        members,
        status: Vec::new(),
    };

    // Write code workspace file if requested
    if let Some(workspace_path) = code_workspace {
        write_code_workspace(&working_set, workspace_path, force)?;
    }

    if json {
        print_json(&working_set);
        return Ok(());
    }

    print_human_working_set(&working_set, &data.project_config);
    Ok(())
}

/// Write a VS Code workspace file.
fn write_code_workspace(
    working_set: &WorkingSet,
    output_path: &str,
    force: bool,
) -> anyhow::Result<()> {
    let resolved = std::path::PathBuf::from(output_path)
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from(output_path));

    if resolved.exists() && !force {
        anyhow::bail!(
            "Refusing to overwrite {}. Pass --force to overwrite, or choose a different path.",
            resolved.display()
        );
    }

    if let Some(parent) = resolved.parent() {
        if !parent.is_dir() {
            anyhow::bail!(
                "Output directory does not exist: {}. Create the directory first.",
                parent.display()
            );
        }
    }

    let root_name_owned = working_set.root.store_id.clone().unwrap_or_else(|| {
        Path::new(&working_set.root.path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "root".to_string())
    });
    let root_name = root_name_owned.as_str();

    let mut folders = vec![serde_json::json!({ "name": root_name, "path": working_set.root.path })];
    for m in working_set.members.iter().filter(|m| m.available) {
        folders.push(serde_json::json!({ "name": m.id, "path": m.path }));
    }
    let workspace = serde_json::json!({
        "folders": folders,
        "settings": {}
    });

    let content = serde_json::to_string_pretty(&workspace)?;
    std::fs::write(&resolved, content)?;

    let available = working_set.members.iter().filter(|m| m.available).count();
    let skipped: Vec<&str> = working_set
        .members
        .iter()
        .filter(|m| !m.available)
        .map(|m| m.id.as_str())
        .collect();

    let summary = if skipped.is_empty() {
        format!("Wrote {} ({} folders)", resolved.display(), available + 1)
    } else {
        format!(
            "Wrote {} ({} folders; not available: {})",
            resolved.display(),
            available + 1,
            skipped.join(", ")
        )
    };
    eprintln!("{summary}");

    Ok(())
}

/// Print working set in human-readable format.
fn print_human_working_set(
    working_set: &WorkingSet,
    project_config: &Option<crate::shared_gather::ProjectConfig>,
) {
    let root_label_owned = working_set.root.store_id.clone().unwrap_or_else(|| {
        Path::new(&working_set.root.path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "root".to_string())
    });
    let root_label = root_label_owned.as_str();

    println!(
        "Working context for {root_label} ({})",
        working_set.root.path
    );
    println!();
    println!("Speckit root");
    println!("  {root_label}  {}", working_set.root.path);

    let available_stores: Vec<&WorkingSetMember> = working_set
        .members
        .iter()
        .filter(|m| m.role == "referenced_store" && m.available)
        .collect();

    let unavailable: Vec<&WorkingSetMember> = working_set
        .members
        .iter()
        .filter(|m| !m.available)
        .collect();

    if !available_stores.is_empty() {
        println!();
        println!("Referenced stores");
        for member in &available_stores {
            println!("  {}  {}", member.id, member.path);
            if let Some(ref fetch) = member.fetch {
                println!("    Fetch: {fetch}");
            }
        }
    }

    let declared_ref_count = project_config
        .as_ref()
        .map(|c| c.references.len())
        .unwrap_or(0);

    if working_set.members.is_empty() {
        println!();
        if declared_ref_count > 0 {
            println!(
                "Declared references all resolve to this root; the working set is this root alone."
            );
        } else {
            println!("No references declared; the working set is this root alone.");
        }
    }

    if !unavailable.is_empty() || !working_set.status.is_empty() {
        println!();
        println!("Not available on this machine");
        for member in &unavailable {
            if member.status.is_empty() {
                println!("  - {}", member.id);
            } else {
                for diagnostic in &member.status {
                    println!("  - {}: {}", member.id, diagnostic.message);
                    if let Some(ref fix) = diagnostic.fix {
                        println!("    Fix: {fix}");
                    }
                }
            }
        }
        for diagnostic in &working_set.status {
            println!("  Note: {}", diagnostic.message);
            if let Some(ref fix) = diagnostic.fix {
                println!("  Fix: {fix}");
            }
        }
    }
}
