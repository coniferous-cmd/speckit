//! Workset Command
//!
//! Compose, keep, and open personal working views (purely local).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::shared_output::{emit_failure, print_json};

/// A workset definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workset {
    pub name: String,
    pub members: Vec<WorksetMember>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
}

/// A workset member.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorksetMember {
    pub name: String,
    pub path: String,
}

/// Options for the workset create command.
#[derive(Debug, Clone)]
pub struct WorksetCreateOptions {
    pub member: Vec<String>,
    pub tool: Option<String>,
    pub json: bool,
}

/// Options for the workset open command.
#[derive(Debug, Clone)]
pub struct WorksetOpenOptions {
    pub tool: Option<String>,
    pub json: bool,
}

/// Options for the workset remove command.
#[derive(Debug, Clone)]
pub struct WorksetRemoveOptions {
    pub yes: bool,
    pub json: bool,
}

/// Get the path to the worksets state file.
fn get_worksets_state_path() -> PathBuf {
    // Keep workset persistence on the same cross-platform config path as the
    // rest of Speckit.  In particular, `dirs::config_dir()` ignores
    // XDG_CONFIG_HOME on Windows, which can make tests and isolated CLI
    // invocations read/write the user's real AppData directory.
    speckit_core::global_config::get_global_config_dir().join("worksets.json")
}

/// Read the worksets state.
fn read_worksets_state() -> anyhow::Result<Vec<Workset>> {
    let path = get_worksets_state_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)?;
    let worksets: Vec<Workset> = serde_json::from_str(&content)?;
    Ok(worksets)
}

/// Write the worksets state.
fn write_worksets_state(worksets: &[Workset]) -> anyhow::Result<()> {
    let path = get_worksets_state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(worksets)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Validate a workset name.
fn validate_workset_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        anyhow::bail!("Workset name cannot be empty");
    }
    if name.contains('/') || name.contains('\\') {
        anyhow::bail!("Workset name cannot contain path separators");
    }
    if name.starts_with('.') {
        anyhow::bail!("Workset name cannot start with a dot");
    }
    Ok(())
}

/// Execute the workset create command.
pub async fn workset_create(
    name: Option<&str>,
    options: WorksetCreateOptions,
) -> anyhow::Result<()> {
    let workset_name = match name {
        Some(n) => n.to_string(),
        None => {
            if options.json {
                emit_failure(
                    true,
                    serde_json::json!({ "workset": null, "status": [] }),
                    &anyhow::anyhow!("Pass a workset name."),
                    "workset_name_required",
                );
            }
            anyhow::bail!(
                "Pass a workset name. Usage: speckit workset create <name> --member <path>"
            );
        }
    };

    validate_workset_name(&workset_name)?;

    if options.member.is_empty() {
        if options.json {
            emit_failure(
                true,
                serde_json::json!({ "workset": null, "status": [] }),
                &anyhow::anyhow!("Pass at least one member folder."),
                "workset_members_required",
            );
        }
        anyhow::bail!(
            "Pass at least one member folder.\nUsage: speckit workset create {workset_name} --member <path>"
        );
    }

    let members: Vec<WorksetMember> = options
        .member
        .iter()
        .map(|m| {
            let (name, path) = if let Some(eq_pos) = m.find('=') {
                let name = m[..eq_pos].to_string();
                let path = m[eq_pos + 1..].to_string();
                (name, path)
            } else {
                let path = m.clone();
                let name = PathBuf::from(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                (name, path)
            };
            WorksetMember {
                name,
                path: shellexpand_tilde(&path),
            }
        })
        .collect();

    let workset = Workset {
        name: workset_name.clone(),
        members,
        tool: options.tool,
    };

    // Save the workset
    let mut worksets = read_worksets_state()?;
    worksets.retain(|w| w.name != workset_name);
    worksets.push(workset.clone());
    write_worksets_state(&worksets)?;

    if options.json {
        print_json(&serde_json::json!({
            "workset": workset,
            "status": [],
        }));
        return Ok(());
    }

    println!();
    println!(
        "Saved workset '{}' ({} member{}) to your machine.",
        workset.name,
        workset.members.len(),
        if workset.members.len() == 1 { "" } else { "s" }
    );
    println!(
        "Open it any time with: speckit workset open {}",
        workset.name
    );
    Ok(())
}

/// Execute the workset list command.
pub async fn workset_list(json: bool) -> anyhow::Result<()> {
    let worksets = read_worksets_state()?;

    if json {
        print_json(&serde_json::json!({
            "worksets": worksets,
            "status": [],
        }));
        return Ok(());
    }

    if worksets.is_empty() {
        println!("No worksets saved. Create one with: speckit workset create");
        return Ok(());
    }

    for workset in &worksets {
        let tool_label = workset
            .tool
            .as_ref()
            .map(|t| format!("  (opens in {t})"))
            .unwrap_or_default();
        println!("{}{tool_label}", workset.name);
        for member in &workset.members {
            println!("  {}  {}", member.name, member.path);
        }
    }

    Ok(())
}

/// Execute the workset open command.
pub async fn workset_open(name: &str, options: WorksetOpenOptions) -> anyhow::Result<()> {
    if options.json {
        emit_failure(
            true,
            serde_json::json!({ "status": [] }),
            &anyhow::anyhow!(
                "workset open hands this terminal to the chosen tool and has no JSON mode."
            ),
            "workset_open_json_unsupported",
        );
    }

    let worksets = read_worksets_state()?;
    let workset = worksets
        .iter()
        .find(|w| w.name == name)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Workset '{}' not found.", name))?;

    // Check member availability
    let mut surviving = Vec::new();
    let mut skipped = Vec::new();

    for member in &workset.members {
        if PathBuf::from(&member.path).is_dir() {
            surviving.push(member);
        } else {
            skipped.push(member);
        }
    }

    for member in &skipped {
        eprintln!(
            "Skipped '{}' ({} is not available).",
            member.name, member.path
        );
    }

    if surviving.is_empty() {
        anyhow::bail!(
            "No member folder of workset '{}' exists on this machine.",
            name
        );
    }

    if !surviving.is_empty() && surviving[0].name != workset.members[0].name {
        eprintln!(
            "Using '{}' ({}) as the primary for this open.",
            surviving[0].name, surviving[0].path
        );
    }

    // Determine tool
    let tool = options.tool.or(workset.tool).ok_or_else(|| {
        anyhow::anyhow!("Workset '{}' has no saved tool. Pass --tool <id>.", name)
    })?;

    // Generate workspace file
    let workspace_path = dirs::runtime_dir()
        .or_else(dirs::cache_dir)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("speckit")
        .join("worksets")
        .join(format!("{name}.code-workspace"));

    if let Some(parent) = workspace_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let workspace = serde_json::json!({
        "folders": surviving.iter().map(|m| {
            serde_json::json!({ "name": m.name, "path": m.path })
        }).collect::<Vec<_>>(),
        "settings": {}
    });
    std::fs::write(&workspace_path, serde_json::to_string_pretty(&workspace)?)?;

    // Launch the tool
    match tool.as_str() {
        "code" => {
            println!("Opening '{}' in VS Code...", name);
            let status = std::process::Command::new("code")
                .arg(&workspace_path)
                .status()?;
            if !status.success() {
                eprintln!("VS Code exited with code {}", status.code().unwrap_or(1));
            }
        }
        "cursor" => {
            println!("Opening '{}' in Cursor...", name);
            let status = std::process::Command::new("cursor")
                .arg(&workspace_path)
                .status()?;
            if !status.success() {
                eprintln!("Cursor exited with code {}", status.code().unwrap_or(1));
            }
        }
        _ => {
            anyhow::bail!("Unknown tool '{}'. Supported tools: code, cursor", tool);
        }
    }

    Ok(())
}

/// Execute the workset remove command.
pub async fn workset_remove(name: &str, options: WorksetRemoveOptions) -> anyhow::Result<()> {
    let mut worksets = read_worksets_state()?;
    let workset = worksets.iter().find(|w| w.name == name).cloned();

    let workset = match workset {
        Some(w) => w,
        None => {
            if worksets.is_empty() {
                anyhow::bail!("No worksets saved. Nothing to remove.");
            }
            let available: Vec<&str> = worksets.iter().map(|w| w.name.as_str()).collect();
            anyhow::bail!(
                "Workset '{}' not found. Available worksets:\n  {}",
                name,
                available.join("\n  ")
            );
        }
    };

    if !options.yes {
        if options.json || !atty_is_tty() {
            emit_failure(
                options.json,
                serde_json::json!({ "removed": null, "status": [] }),
                &anyhow::anyhow!("Pass --yes to remove a workset non-interactively."),
                "workset_remove_confirmation_required",
            );
        }

        let confirmed = inquire::Confirm::new(&format!(
            "Remove workset '{}'? ({} members)",
            workset.name,
            workset.members.len()
        ))
        .with_default(false)
        .prompt()
        .map_err(|e| anyhow::anyhow!("Prompt cancelled: {e}"))?;

        if !confirmed {
            anyhow::bail!("Workset remove cancelled.");
        }
    }

    worksets.retain(|w| w.name != name);
    write_worksets_state(&worksets)?;

    if options.json {
        print_json(&serde_json::json!({
            "removed": { "name": name },
            "status": [],
        }));
        return Ok(());
    }

    println!(
        "Removed workset '{}'. Member folders were not touched.",
        name
    );
    Ok(())
}

/// Expand `~` in paths.
fn shellexpand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return format!("{}{}", home.display(), &path[1..]);
        }
    }
    path.to_string()
}

fn atty_is_tty() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}
