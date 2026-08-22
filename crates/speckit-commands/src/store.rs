//! Store Command
//!
//! Create and manage stores - standalone Speckit repos you register on this machine.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::shared_output::{StoreDiagnostic, emit_failure, print_json};

/// Store information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreInfo {
    pub id: String,
    pub root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_path: Option<String>,
}

/// Store list output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreListOutput {
    pub stores: Vec<StoreInfo>,
    pub status: Vec<StoreDiagnostic>,
}

/// Store mutation output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreMutationOutput {
    pub store: Option<StoreInfo>,
    pub registry: Option<RegistryOutput>,
    pub status: Vec<StoreDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryOutput {
    pub path: String,
    pub registered: bool,
}

/// Store cleanup output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreCleanupOutput {
    pub store: Option<StoreInfo>,
    pub status: Vec<StoreDiagnostic>,
}

/// Execute the store setup command.
pub async fn store_setup(
    id: Option<&str>,
    path: Option<&str>,
    init_git: Option<bool>,
    remote: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let store_id = match id {
        Some(s) => s.to_string(),
        None => {
            if json {
                emit_failure(
                    true,
                    serde_json::json!({ "store": null, "status": [] }),
                    &anyhow::anyhow!("Pass a store name."),
                    "store_setup_id_required",
                );
            }
            anyhow::bail!(
                "Pass a store name. Usage: speckit store setup <id> --path ~/speckit/<id>"
            );
        }
    };

    let store_path = match path {
        Some(p) => shellexpand::tilde(p).to_string(),
        None => {
            if json {
                emit_failure(
                    true,
                    serde_json::json!({ "store": null, "status": [] }),
                    &anyhow::anyhow!("Pass --path with the folder where this store should live."),
                    "store_setup_path_required",
                );
            }
            let default_path = format!("~/speckit/{store_id}");
            anyhow::bail!(
                "Pass --path with the folder where this store should live.\nExample: speckit store setup {store_id} --path {default_path}"
            );
        }
    };

    // Create the store directory
    let store_path_buf = PathBuf::from(&store_path);
    std::fs::create_dir_all(&store_path_buf)?;

    // Create speckit structure
    let speckit_dir = store_path_buf.join("speckit");
    std::fs::create_dir_all(&speckit_dir)?;
    std::fs::create_dir_all(speckit_dir.join("specs"))?;
    std::fs::create_dir_all(speckit_dir.join("changes"))?;

    // Write project.md
    let project_md = speckit_dir.join("project.md");
    if !project_md.exists() {
        std::fs::write(&project_md, format!("# {store_id}\n\nSpeckit store.\n"))?;
    }

    // Write store.yaml metadata
    let store_yaml = store_path_buf.join("store.yaml");
    let metadata = serde_json::json!({
        "id": store_id,
        "version": "1.0",
    });
    std::fs::write(&store_yaml, serde_yaml::to_string(&metadata)?)?;

    // Initialize git if requested (default: true for new stores)
    let should_init_git = init_git.unwrap_or(true);
    if should_init_git && !store_path_buf.join(".git").exists() {
        match std::process::Command::new("git")
            .arg("init")
            .current_dir(&store_path_buf)
            .output()
        {
            Ok(output) if output.status.success() => {
                if !json {
                    println!("Initialized git repository.");
                }
            }
            Ok(_) => {
                if !json {
                    eprintln!("Warning: git init failed (git may not be installed).");
                }
            }
            Err(_) => {
                if !json {
                    eprintln!("Warning: git not found, skipping repository initialization.");
                }
            }
        }

        // Set remote origin if provided
        if let Some(ref remote_url) = remote {
            match std::process::Command::new("git")
                .args(["remote", "add", "origin", remote_url])
                .current_dir(&store_path_buf)
                .output()
            {
                Ok(output) if output.status.success() => {
                    if !json {
                        println!("Set remote origin: {remote_url}");
                    }
                }
                _ => {
                    if !json {
                        eprintln!("Warning: failed to set remote origin.");
                    }
                }
            }
        }
    }

    // Register the store
    register_store_in_registry(&store_id, &store_path)?;

    let payload = StoreMutationOutput {
        store: Some(StoreInfo {
            id: store_id.clone(),
            root: store_path.clone(),
            metadata_path: Some(store_yaml.to_string_lossy().to_string()),
        }),
        registry: Some(RegistryOutput {
            path: get_registry_path().to_string_lossy().to_string(),
            registered: true,
        }),
        status: Vec::new(),
    };

    if json {
        print_json(&payload);
        return Ok(());
    }

    println!("Store ready: {store_id}");
    println!(
        "Location: {}",
        crate::shared_output::format_path_for_human(&store_path)
    );
    println!("Speckit root: ready");
    println!("Registry: registered");
    println!();
    println!("Next: run normal Speckit commands against this store:");
    println!("  speckit new change <change-id> --store {store_id}");
    Ok(())
}

/// Execute the store register command.
pub async fn store_register(
    input_path: Option<&str>,
    id: Option<&str>,
    yes: bool,
    json: bool,
) -> anyhow::Result<()> {
    let store_path = match input_path {
        Some(p) => shellexpand::tilde(p).to_string(),
        None => {
            anyhow::bail!(
                "Pass the path to an existing store. Usage: speckit store register <path>"
            );
        }
    };

    let store_path_buf = PathBuf::from(&store_path);
    if !store_path_buf.is_dir() {
        anyhow::bail!("Path '{}' is not a directory", store_path);
    }

    // Try to read store.yaml for ID
    let store_yaml = store_path_buf.join("store.yaml");
    let store_id = if let Some(provided_id) = id {
        provided_id.to_string()
    } else if store_yaml.exists() {
        let content = std::fs::read_to_string(&store_yaml)?;
        let metadata: serde_yaml::Value = serde_yaml::from_str(&content)?;
        match metadata.get("id").and_then(|v| v.as_str()) {
            Some(id_str) => id_str.to_string(),
            None => store_path_buf
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
        }
    } else {
        store_path_buf
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    };

    if !store_yaml.exists() {
        if !yes {
            if !atty_is_tty() {
                anyhow::bail!(
                    "Registering this store will create identity metadata at {}. Re-run with --yes.",
                    store_yaml.display()
                );
            }
            let confirmed = inquire::Confirm::new(&format!(
                "Create store identity metadata at {}?",
                store_yaml.display()
            ))
            .with_default(false)
            .prompt()
            .map_err(|error| anyhow::anyhow!("Confirmation cancelled: {error}"))?;
            if !confirmed {
                anyhow::bail!("Store registration cancelled.");
            }
        }
        let metadata = serde_json::json!({
            "id": store_id,
            "version": "1.0",
        });
        std::fs::write(&store_yaml, serde_yaml::to_string(&metadata)?)?;
    }

    register_store_in_registry(&store_id, &store_path)?;

    let payload = StoreMutationOutput {
        store: Some(StoreInfo {
            id: store_id.clone(),
            root: store_path.clone(),
            metadata_path: if store_yaml.exists() {
                Some(store_yaml.to_string_lossy().to_string())
            } else {
                None
            },
        }),
        registry: Some(RegistryOutput {
            path: get_registry_path().to_string_lossy().to_string(),
            registered: true,
        }),
        status: Vec::new(),
    };

    if json {
        print_json(&payload);
        return Ok(());
    }

    println!("Store registered: {store_id}");
    println!(
        "Location: {}",
        crate::shared_output::format_path_for_human(&store_path)
    );
    Ok(())
}

/// Execute the store unregister command.
pub async fn store_unregister(id: &str, json: bool) -> anyhow::Result<()> {
    unregister_store_from_registry(id)?;

    let payload = StoreCleanupOutput {
        store: Some(StoreInfo {
            id: id.to_string(),
            root: String::new(),
            metadata_path: None,
        }),
        status: Vec::new(),
    };

    if json {
        print_json(&payload);
        return Ok(());
    }

    println!("Unregistered store: {id}");
    Ok(())
}

/// Execute the store remove command.
pub async fn store_remove(id: &str, yes: bool, json: bool) -> anyhow::Result<()> {
    let registry = read_registry()?;
    let entry = registry
        .iter()
        .find(|e| e.id == id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Store '{id}' not found in registry"))?;

    if !yes && atty_is_tty() {
        let confirmed = inquire::Confirm::new(&format!(
            "Delete local store folder {}?",
            crate::shared_output::format_path_for_human(&entry.root)
        ))
        .with_default(false)
        .prompt()
        .map_err(|e| anyhow::anyhow!("Prompt cancelled: {e}"))?;

        if !confirmed {
            anyhow::bail!("Store remove cancelled.");
        }
    }

    unregister_store_from_registry(id)?;

    // Delete the store directory
    let store_path = PathBuf::from(&entry.root);
    if store_path.is_dir() {
        std::fs::remove_dir_all(&store_path)?;
    }

    let payload = StoreCleanupOutput {
        store: Some(StoreInfo {
            id: id.to_string(),
            root: entry.root,
            metadata_path: None,
        }),
        status: Vec::new(),
    };

    if json {
        print_json(&payload);
        return Ok(());
    }

    println!("Removed store: {id}");
    Ok(())
}

/// Execute the store list command.
pub async fn store_list(json: bool) -> anyhow::Result<()> {
    let registry = read_registry()?;
    let stores: Vec<StoreInfo> = registry
        .iter()
        .map(|e| StoreInfo {
            id: e.id.clone(),
            root: e.root.clone(),
            metadata_path: None,
        })
        .collect();

    let payload = StoreListOutput {
        stores: stores.clone(),
        status: Vec::new(),
    };

    if json {
        print_json(&payload);
        return Ok(());
    }

    if stores.is_empty() {
        println!("No stores registered.");
        println!();
        println!("Next:");
        println!("  speckit store setup team-context --path ~/speckit/team-context");
        println!("  speckit store register /path/to/store");
        return Ok(());
    }

    println!("Speckit stores ({})", stores.len());
    println!();
    println!("{:<16}Location", "ID");
    for store in &stores {
        println!("{:<16}{}", store.id, store.root);
    }
    Ok(())
}

/// Execute the store doctor command.
pub async fn store_doctor(id: Option<&str>, json: bool) -> anyhow::Result<()> {
    let registry = read_registry()?;

    let stores: Vec<&RegistryEntry> = match id {
        Some(store_id) => registry.iter().filter(|e| e.id == store_id).collect(),
        None => registry.iter().collect(),
    };

    if json {
        let output: Vec<serde_json::Value> = stores
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "root": e.root,
                    "metadata": { "valid": true },
                    "speckit_root": { "healthy": PathBuf::from(&e.root).join("speckit").is_dir() },
                })
            })
            .collect();
        print_json(&serde_json::json!({ "stores": output, "status": [] }));
        return Ok(());
    }

    if stores.is_empty() {
        println!("No stores registered.");
        return Ok(());
    }

    println!("Store doctor");
    for store in &stores {
        println!();
        println!("{}", store.id);
        println!("  Location: {}", store.root);
        let speckit_dir = PathBuf::from(&store.root).join("speckit");
        let root_status = if speckit_dir.is_dir() {
            "ok"
        } else {
            "missing"
        };
        println!("  Speckit root: {root_status}");
    }
    Ok(())
}

// Helper functions for registry management

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryEntry {
    id: String,
    root: String,
}

fn get_registry_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("speckit")
        .join("store-registry.json")
}

fn read_registry() -> anyhow::Result<Vec<RegistryEntry>> {
    let path = get_registry_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)?;
    let entries: Vec<RegistryEntry> = serde_json::from_str(&content)?;
    Ok(entries)
}

fn write_registry(entries: &[RegistryEntry]) -> anyhow::Result<()> {
    let path = get_registry_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(entries)?;
    std::fs::write(&path, content)?;
    Ok(())
}

fn register_store_in_registry(id: &str, root: &str) -> anyhow::Result<()> {
    let mut entries = read_registry()?;
    // Remove existing entry with same ID
    entries.retain(|e| e.id != id);
    entries.push(RegistryEntry {
        id: id.to_string(),
        root: root.to_string(),
    });
    write_registry(&entries)
}

fn unregister_store_from_registry(id: &str) -> anyhow::Result<()> {
    let mut entries = read_registry()?;
    entries.retain(|e| e.id != id);
    write_registry(&entries)
}

fn atty_is_tty() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}

/// Expand `~` in paths.
mod shellexpand {
    pub fn tilde(path: &str) -> std::borrow::Cow<'_, str> {
        if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                return format!("{}{}", home.display(), &path[1..]).into();
            }
        }
        path.into()
    }
}
