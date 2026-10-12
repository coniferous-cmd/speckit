//! Store commands share the core registry and identity implementation with root selection.

use serde_json::{Value, json};
use speckit_core::store::{StoreError, operations as core};

use crate::shared_output::print_json;

fn interactive(json: bool) -> bool {
    use std::io::IsTerminal;
    !json && std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

fn confirm(message: &str) -> anyhow::Result<()> {
    if !inquire::Confirm::new(message)
        .with_default(false)
        .prompt()?
    {
        anyhow::bail!("Store operation cancelled.");
    }
    Ok(())
}

fn finish<T>(result: Result<T, StoreError>, json: bool, mut empty: Value) -> anyhow::Result<T> {
    result.map_err(|error| {
        if json {
            empty["status"] = json!([error.diagnostic]);
            print_json(&empty);
        }
        error.into()
    })
}

fn info(store: &core::StoreInfo) -> Value {
    let mut value = json!({ "id": store.id, "root": store.root });
    if let Some(path) = &store.metadata_path {
        value["metadata_path"] = json!(path);
    }
    value
}

fn mutation_empty() -> Value {
    json!({ "store": null, "registry": null, "git": null, "created_files": [], "status": [] })
}

fn cleanup_empty() -> Value {
    json!({ "store": null, "registry": null, "files": null, "status": [] })
}

fn print_mutation(result: &core::StoreMutationResult, json: bool, title: &str) {
    if json {
        print_json(&json!({
            "store": info(&result.store),
            "registry": {
                "path": result.registry_commit.path,
                "registered": result.registry_commit.registered,
                "already_registered": result.registry_commit.already_registered,
            },
            "git": {
                "is_repository": result.git.is_repository,
                "initialized": result.git.initialized,
                "committed": result.git.committed,
            },
            "created_files": result.created_artifacts,
            "status": result.diagnostics,
        }));
    } else {
        println!("{title}: {}", result.store.id);
        println!("Location: {}", result.store.root.display());
        for diagnostic in &result.diagnostics {
            println!("{}: {}", diagnostic.severity, diagnostic.message);
        }
    }
}

fn print_cleanup(result: &core::StoreCleanupResult, json: bool, title: &str) {
    if json {
        print_json(&json!({
            "store": info(&result.store),
            "registry": { "path": result.registry_commit.path, "removed": result.registry_commit.removed },
            "files": {
                "deleted": result.files.deleted,
                "deleted_path": result.files.deleted_path,
                "left_on_disk": result.files.left_on_disk,
            },
            "status": result.diagnostics,
        }));
    } else {
        println!("{title}: {}", result.store.id);
        for diagnostic in &result.diagnostics {
            println!("{}: {}", diagnostic.severity, diagnostic.message);
        }
    }
}

pub async fn store_setup(
    id: Option<&str>,
    path: Option<&str>,
    init_git: Option<bool>,
    remote: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let input = core::SetupStoreInput {
        id: id.map(str::to_owned),
        path: path.map(str::to_owned),
        init_git,
        allow_inside_git_repository: None,
        remote,
    };
    let prepared = finish(core::prepare_store_setup(&input), json, mutation_empty())?;
    let init_git = core::resolve_setup_git_enabled(&prepared, init_git);
    if interactive(json) {
        confirm(&format!(
            "Set up store '{}' at {}?",
            prepared.id,
            prepared.root.display()
        ))?;
    }
    let result = finish(
        core::setup_prepared_store(&prepared, Some(init_git)),
        json,
        mutation_empty(),
    )?;
    print_mutation(&result, json, "Store ready");
    Ok(())
}

pub async fn store_register(
    input_path: Option<&str>,
    id: Option<&str>,
    yes: bool,
    json: bool,
) -> anyhow::Result<()> {
    let mut input = core::RegisterExistingStoreInput {
        path: input_path.map(str::to_owned),
        id: id.map(str::to_owned),
        allow_create_identity: Some(yes),
    };
    let mut result = core::register_existing_store(&input);
    if let Err(error) = &result {
        if error.code() == "store_register_identity_confirmation_required" && interactive(json) {
            confirm(&format!("{} {}", error, error.fix().unwrap_or_default()))?;
            input.allow_create_identity = Some(true);
            result = core::register_existing_store(&input);
        }
    }
    let result = finish(result, json, mutation_empty())?;
    print_mutation(&result, json, "Store registered");
    Ok(())
}

pub async fn store_unregister(id: &str, json: bool) -> anyhow::Result<()> {
    let result = finish(
        core::unregister_store(&core::CleanupStoreInput {
            id: id.to_owned(),
            global_data_dir: None,
        }),
        json,
        cleanup_empty(),
    )?;
    print_cleanup(&result, json, "Unregistered store");
    Ok(())
}

pub async fn store_remove(id: &str, yes: bool, json: bool) -> anyhow::Result<()> {
    let target = finish(
        core::prepare_store_cleanup(&core::CleanupStoreInput {
            id: id.to_owned(),
            global_data_dir: None,
        }),
        json,
        cleanup_empty(),
    )?;
    if !yes {
        if !interactive(json) {
            return finish(
                Err(StoreError::new(
                    "Deleting a local store folder requires --yes in non-interactive mode.",
                    "store_remove_confirmation_required",
                    Default::default(),
                )),
                json,
                cleanup_empty(),
            );
        }
        confirm(&format!(
            "Delete local store folder {}?",
            target.root.display()
        ))?;
    }
    let result = finish(core::remove_store(&target), json, cleanup_empty())?;
    print_cleanup(&result, json, "Removed store");
    Ok(())
}

pub async fn store_list(json: bool) -> anyhow::Result<()> {
    let result = finish(
        core::list_stores(),
        json,
        json!({ "stores": [], "status": [] }),
    )?;
    if json {
        print_json(
            &json!({ "stores": result.stores.iter().map(info).collect::<Vec<_>>(), "status": [] }),
        );
    } else if result.stores.is_empty() {
        println!("No stores registered.");
    } else {
        println!("Speckit stores ({})", result.stores.len());
        println!("{:<16}Location", "ID");
        for store in result.stores {
            println!("{:<16}{}", store.id, store.root.display());
        }
    }
    Ok(())
}

pub async fn store_doctor(id: Option<&str>, json: bool) -> anyhow::Result<()> {
    let result = finish(
        core::doctor_stores(id),
        json,
        json!({ "stores": [], "status": [] }),
    )?;
    if json {
        let stores: Vec<_> = result
            .stores
            .iter()
            .map(|store| {
                json!({
                    "id": store.id,
                    "root": store.root,
                    "metadata_path": store.metadata_path,
                    "speckit_root": {
                        "present": store.speckit_root.present,
                        "healthy": store.speckit_root.healthy,
                        "config": store.speckit_root.config,
                        "specs": store.speckit_root.specs,
                        "changes": store.speckit_root.changes,
                        "archive": store.speckit_root.archive,
                        "status": store.speckit_root.diagnostics,
                    },
                    "metadata": {
                        "present": store.metadata.present,
                        "valid": store.metadata.valid,
                        "id": store.metadata.id,
                        "remote": store.metadata.remote,
                    },
                    "git": {
                        "is_repository": store.git.is_repository,
                        "has_commits": store.git.has_commits,
                        "has_uncommitted_changes": store.git.has_uncommitted_changes,
                        "has_remote": store.git.has_remote,
                        "origin_url": store.git.origin_url,
                    },
                    "status": store.diagnostics,
                })
            })
            .collect();
        print_json(&json!({ "stores": stores, "status": result.diagnostics }));
    } else {
        if result.stores.is_empty() {
            println!("No stores registered.");
        }
        for store in result.stores {
            println!("{}: {}", store.id, store.root.display());
            for diagnostic in store.diagnostics {
                println!("  {}: {}", diagnostic.severity, diagnostic.message);
                if let Some(fix) = diagnostic.fix {
                    println!("  Fix: {fix}");
                }
            }
        }
    }
    Ok(())
}
