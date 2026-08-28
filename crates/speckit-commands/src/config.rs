//! Config Command
//!
//! View and modify global Speckit configuration.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Global configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalConfig {
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default = "default_delivery")]
    pub delivery: String,
    #[serde(default)]
    pub workflows: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

fn default_profile() -> String {
    "core".to_string()
}

fn default_delivery() -> String {
    "both".to_string()
}

/// Core workflows for the default profile.
pub const CORE_WORKFLOWS: &[&str] = &[
    "propose",
    "explore",
    "new",
    "continue",
    "apply",
    "update",
    "ff",
    "sync",
    "archive",
    "bulk-archive",
    "verify",
    "onboard",
];

/// All available workflows.
pub const ALL_WORKFLOWS: &[&str] = &[
    "propose",
    "explore",
    "new",
    "continue",
    "apply",
    "update",
    "ff",
    "sync",
    "archive",
    "bulk-archive",
    "verify",
    "onboard",
];

/// Get the global config file path.
pub fn get_global_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("speckit")
        .join("config.json")
}

/// Load the global configuration.
pub fn get_global_config() -> GlobalConfig {
    let path = get_global_config_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => GlobalConfig::default(),
    }
}

/// Save the global configuration.
pub fn save_global_config(config: &GlobalConfig) -> anyhow::Result<()> {
    let path = get_global_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Execute the config path command.
pub fn config_path() {
    println!("{}", get_global_config_path().display());
}

/// Execute the config list command.
pub fn config_list(json: bool) {
    let config = get_global_config();

    if json {
        crate::shared_output::print_json(&config);
        return;
    }

    // Read raw config to determine explicit vs defaults
    let raw: serde_json::Value = std::fs::read_to_string(get_global_config_path())
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default();

    println!("profile: {}", config.profile);
    println!("delivery: {}", config.delivery);

    let profile_source = if raw.get("profile").is_some() {
        "(explicit)"
    } else {
        "(default)"
    };
    let delivery_source = if raw.get("delivery").is_some() {
        "(explicit)"
    } else {
        "(default)"
    };

    println!();
    println!("Profile settings:");
    println!("  profile: {} {}", config.profile, profile_source);
    println!("  delivery: {} {}", config.delivery, delivery_source);

    if config.profile == "core" {
        println!(
            "  workflows: {} (from core profile)",
            CORE_WORKFLOWS.join(", ")
        );
    } else if let Some(ref wf) = config.workflows {
        if !wf.is_empty() {
            println!("  workflows: {} (explicit)", wf.join(", "));
        } else {
            println!("  workflows: (none)");
        }
    } else {
        println!("  workflows: (none)");
    }
}

/// Execute the config get command.
pub fn config_get(key: &str) -> anyhow::Result<()> {
    let config = get_global_config();
    let config_value = serde_json::to_value(&config)?;
    let value = get_nested_value(&config_value, key);

    match value {
        Some(v) => {
            if v.is_object() || v.is_array() {
                println!("{}", serde_json::to_string(v)?);
            } else {
                println!("{}", format_json_value(v));
            }
            Ok(())
        }
        None => {
            std::process::exit(1);
        }
    }
}

/// Execute the config set command.
pub fn config_set(
    key: &str,
    value: &str,
    force_string: bool,
    _allow_unknown: bool,
) -> anyhow::Result<()> {
    let config = get_global_config();
    let mut config_value = serde_json::to_value(&config)?;
    let coerced = coerce_value(value, force_string);

    set_nested_value(&mut config_value, key, coerced);
    let new_config: GlobalConfig = serde_json::from_value(config_value)?;
    save_global_config(&new_config)?;

    let display_value = if force_string || value.parse::<serde_json::Value>().is_err() {
        format!("\"{value}\"")
    } else {
        value.to_string()
    };
    println!("Set {key} = {display_value}");
    Ok(())
}

/// Execute the config unset command.
pub fn config_unset(key: &str) -> anyhow::Result<()> {
    let config = get_global_config();
    let mut config_value = serde_json::to_value(&config)?;

    let existed = delete_nested_value(&mut config_value, key);
    if existed {
        let new_config: GlobalConfig = serde_json::from_value(config_value)?;
        save_global_config(&new_config)?;
        println!("Unset {key} (reverted to default)");
    } else {
        println!("Key \"{key}\" was not set");
    }
    Ok(())
}

/// Execute the config reset command.
pub fn config_reset(all: bool, yes: bool) -> anyhow::Result<()> {
    if !all {
        eprintln!("Error: --all flag is required for reset");
        eprintln!("Usage: speckit config reset --all [-y]");
        std::process::exit(1);
    }

    if !yes && atty_is_tty() {
        let confirmed = inquire::Confirm::new("Reset all configuration to defaults?")
            .with_default(false)
            .prompt()
            .map_err(|e| anyhow::anyhow!("Prompt cancelled: {e}"))?;
        if !confirmed {
            println!("Reset cancelled.");
            return Ok(());
        }
    }

    save_global_config(&GlobalConfig::default())?;
    println!("Configuration reset to defaults");
    Ok(())
}

/// Execute the config edit command.
pub fn config_edit() -> anyhow::Result<()> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .map_err(|_| {
            anyhow::anyhow!(
                "No editor configured. Set the EDITOR or VISUAL environment variable.\nExample: export EDITOR=vim"
            )
        })?;

    let config_path = get_global_config_path();
    if !config_path.exists() {
        save_global_config(&GlobalConfig::default())?;
    }

    let status = std::process::Command::new(&editor)
        .arg(&config_path)
        .status()?;

    if !status.success() {
        anyhow::bail!("Editor exited with code {}", status.code().unwrap_or(1));
    }

    // Validate the edited config
    match std::fs::read_to_string(&config_path) {
        Ok(content) => match serde_json::from_str::<GlobalConfig>(&content) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: Invalid JSON in {}: {e}", config_path.display());
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!(
                "Error: Config file not found at {}: {e}",
                config_path.display()
            );
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Execute the config profile command.
pub fn config_profile(preset: Option<&str>) -> anyhow::Result<()> {
    match preset {
        Some("core") => {
            let mut config = get_global_config();
            config.profile = "core".to_string();
            config.workflows = Some(CORE_WORKFLOWS.iter().map(|s| s.to_string()).collect());
            save_global_config(&config)?;
            println!("Config updated. Run `speckit update` in your projects to apply.");
            Ok(())
        }
        Some(other) => {
            eprintln!("Error: Unknown profile preset \"{other}\". Available presets: core");
            std::process::exit(1);
        }
        None => {
            if !atty_is_tty() {
                eprintln!(
                    "Interactive mode required. Use `speckit config profile core` or set config via environment/flags."
                );
                std::process::exit(1);
            }

            let config = get_global_config();
            let current_workflows = config
                .workflows
                .clone()
                .unwrap_or_else(|| CORE_WORKFLOWS.iter().map(|s| s.to_string()).collect());

            println!("Current profile settings:");
            println!("  Delivery: {}", config.delivery);
            println!(
                "  Workflows: {} selected ({})",
                current_workflows.len(),
                config.profile
            );

            let action = inquire::Select::new(
                "What do you want to configure?",
                vec![
                    "Both (delivery and workflows)",
                    "Delivery only",
                    "Workflows only",
                    "Keep current settings (exit)",
                ],
            )
            .prompt()
            .map_err(|e| anyhow::anyhow!("Prompt cancelled: {e}"))?;

            if action == "Keep current settings (exit)" {
                println!("No config changes.");
                return Ok(());
            }

            let mut new_config = config.clone();

            if action.contains("delivery") || action.contains("Both") {
                let delivery_options =
                    vec!["Both (skills + commands)", "Skills only", "Commands only"];
                let delivery = inquire::Select::new(
                    "Delivery mode (how workflows are installed):",
                    delivery_options,
                )
                .prompt()
                .map_err(|e| anyhow::anyhow!("Prompt cancelled: {e}"))?;
                new_config.delivery = match delivery {
                    "Both (skills + commands)" => "both".to_string(),
                    "Skills only" => "skills".to_string(),
                    "Commands only" => "commands".to_string(),
                    _ => "both".to_string(),
                };
            }

            if action.contains("workflows") || action.contains("Both") {
                let workflow_list: Vec<&str> = ALL_WORKFLOWS.to_vec();
                let selected =
                    inquire::MultiSelect::new("Select workflows to make available:", workflow_list)
                        .prompt()
                        .map_err(|e| anyhow::anyhow!("Prompt cancelled: {e}"))?;
                let selected_names: Vec<String> =
                    selected.into_iter().map(|s| s.to_string()).collect();

                let is_core = selected_names.len() == CORE_WORKFLOWS.len()
                    && CORE_WORKFLOWS
                        .iter()
                        .all(|w| selected_names.contains(&w.to_string()));

                new_config.profile = if is_core {
                    "core".to_string()
                } else {
                    "custom".to_string()
                };
                new_config.workflows = Some(selected_names);
            }

            save_global_config(&new_config)?;
            println!("Config updated. Run `speckit update` in your projects to apply.");
            Ok(())
        }
    }
}

// Helper functions for nested value operations

fn get_nested_value<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    let parts: Vec<&str> = key.split('.').collect();
    let mut current = value;
    for part in &parts {
        current = current.get(part)?;
    }
    Some(current)
}

fn set_nested_value(value: &mut serde_json::Value, key: &str, new_value: serde_json::Value) {
    let parts: Vec<&str> = key.split('.').collect();
    let mut current = value;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            if let Some(obj) = current.as_object_mut() {
                obj.insert(part.to_string(), new_value);
            }
            return;
        }
        if current.get(part).is_none() {
            if let Some(obj) = current.as_object_mut() {
                obj.insert(part.to_string(), serde_json::json!({}));
            }
        }
        current = current.get_mut(part).unwrap();
    }
}

fn delete_nested_value(value: &mut serde_json::Value, key: &str) -> bool {
    let parts: Vec<&str> = key.split('.').collect();
    let mut current = value;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            if let Some(obj) = current.as_object_mut() {
                return obj.remove(*part).is_some();
            }
            return false;
        }
        match current.get_mut(*part) {
            Some(next) => current = next,
            None => return false,
        }
    }
    false
}

fn coerce_value(value: &str, force_string: bool) -> serde_json::Value {
    if force_string {
        return serde_json::Value::String(value.to_string());
    }
    // Try boolean
    if value == "true" {
        return serde_json::Value::Bool(true);
    }
    if value == "false" {
        return serde_json::Value::Bool(false);
    }
    // Try number
    if let Ok(n) = value.parse::<i64>() {
        return serde_json::json!(n);
    }
    if let Ok(n) = value.parse::<f64>() {
        return serde_json::json!(n);
    }
    // Default to string
    serde_json::Value::String(value.to_string())
}

fn format_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn atty_is_tty() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}
