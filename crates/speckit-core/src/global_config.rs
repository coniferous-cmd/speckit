use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Top-level directory name under the user's config / data home.
const GLOBAL_CONFIG_DIR_NAME: &str = "speckit";
const GLOBAL_CONFIG_FILE_NAME: &str = "config.json";
const GLOBAL_DATA_DIR_NAME: &str = "speckit";

/// Profile controls *which* workflows are installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Profile {
    #[default]
    Core,
    Custom,
}

/// Delivery controls *how* workflows are delivered (skills, commands, or both).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Delivery {
    #[default]
    Both,
    Skills,
    Commands,
}

/// Global Speckit configuration persisted at `~/.config/speckit/config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalConfig {
    #[serde(default)]
    pub feature_flags: HashMap<String, bool>,
    #[serde(default)]
    pub profile: Profile,
    #[serde(default)]
    pub delivery: Delivery,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflows: Option<Vec<String>>,
    /// Machine-level fallback store id, consulted during root resolution only
    /// when no `--store` flag, local root, or project-level store pointer resolves.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_store: Option<String>,
    /// Workset opener rows (slice 7.1); hand-edited, validated on use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openers: Option<serde_json::Value>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            feature_flags: HashMap::new(),
            profile: Profile::Core,
            delivery: Delivery::Both,
            workflows: None,
            default_store: None,
            openers: None,
        }
    }
}

/// Returns the global configuration directory path following XDG Base Directory Specification.
///
/// - All platforms: `$XDG_CONFIG_HOME/speckit/` if `XDG_CONFIG_HOME` is set.
/// - Unix/macOS fallback: `~/.config/speckit/`.
/// - Windows fallback: `%APPDATA%/speckit/`.
pub fn get_global_config_dir() -> PathBuf {
    // XDG_CONFIG_HOME takes precedence on all platforms when explicitly set.
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return PathBuf::from(xdg).join(GLOBAL_CONFIG_DIR_NAME);
    }

    if cfg!(target_os = "windows") {
        if let Ok(app_data) = std::env::var("APPDATA") {
            return PathBuf::from(app_data).join(GLOBAL_CONFIG_DIR_NAME);
        }
        // Fallback for Windows if APPDATA is not set.
        if let Some(home) = dirs::home_dir() {
            return home
                .join("AppData")
                .join("Roaming")
                .join(GLOBAL_CONFIG_DIR_NAME);
        }
    }

    // Unix/macOS fallback: ~/.config
    dirs::home_dir()
        .map(|h| h.join(".config").join(GLOBAL_CONFIG_DIR_NAME))
        .unwrap_or_else(|| PathBuf::from(".config").join(GLOBAL_CONFIG_DIR_NAME))
}

/// Returns the global data directory path following XDG Base Directory Specification.
///
/// - All platforms: `$XDG_DATA_HOME/speckit/` if `XDG_DATA_HOME` is set.
/// - Unix/macOS fallback: `~/.local/share/speckit/`.
/// - Windows fallback: `%LOCALAPPDATA%/speckit/`.
pub fn get_global_data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME")
        && !xdg.is_empty()
    {
        return PathBuf::from(xdg).join(GLOBAL_DATA_DIR_NAME);
    }

    if cfg!(target_os = "windows") {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join(GLOBAL_DATA_DIR_NAME);
        }
        if let Some(home) = dirs::home_dir() {
            return home
                .join("AppData")
                .join("Local")
                .join(GLOBAL_DATA_DIR_NAME);
        }
    }

    dirs::home_dir()
        .map(|h| h.join(".local").join("share").join(GLOBAL_DATA_DIR_NAME))
        .unwrap_or_else(|| PathBuf::from(".local/share").join(GLOBAL_DATA_DIR_NAME))
}

/// Returns the full path to the global config file.
pub fn get_global_config_path() -> PathBuf {
    get_global_config_dir().join(GLOBAL_CONFIG_FILE_NAME)
}

/// Loads the global configuration from disk.
///
/// Returns the default configuration if the file doesn't exist or contains
/// invalid JSON.  Loaded values are merged with defaults so that newly-added
/// fields always have a value.
pub fn get_global_config() -> GlobalConfig {
    let config_path = get_global_config_path();

    if !config_path.exists() {
        return GlobalConfig::default();
    }

    let content = match fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return GlobalConfig::default(),
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Warning: Invalid JSON in {}: {}", config_path.display(), e);
            return GlobalConfig::default();
        }
    };

    // Merge with defaults: deserialise the parsed value; missing fields fall back
    // to the Default impl thanks to `#[serde(default)]`.
    let merged: GlobalConfig = match serde_json::from_value(parsed.clone()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "Warning: could not deserialize config in {}: {}",
                config_path.display(),
                e
            );
            return GlobalConfig::default();
        }
    };

    // Deep-merge feature_flags: start from defaults, overlay loaded values.
    let default_flags = HashMap::new();
    let loaded_flags = merged.feature_flags;

    // Re-read the raw JSON to extract the parsed profile / delivery that may
    // have been absent (serde default fills them in, but the TS code applies
    // defaults only when the key is *missing*, not when it was explicitly null).
    let raw_obj = parsed.as_object();
    let profile = raw_obj
        .and_then(|o| o.get("profile"))
        .map(|_| merged.profile.clone())
        .unwrap_or_default();
    let delivery = raw_obj
        .and_then(|o| o.get("delivery"))
        .map(|_| merged.delivery.clone())
        .unwrap_or_default();

    // Build merged feature_flags map.
    let mut feature_flags = default_flags;
    feature_flags.extend(loaded_flags);

    GlobalConfig {
        feature_flags,
        profile,
        delivery,
        ..merged
    }
}

/// Saves the global configuration to disk.
///
/// Creates the config directory (and any parents) if it doesn't exist.
pub fn save_global_config(config: &GlobalConfig) -> anyhow::Result<()> {
    let config_dir = get_global_config_dir();
    let config_path = get_global_config_path();

    fs::create_dir_all(&config_dir)?;

    let json = serde_json::to_string_pretty(config)?;
    fs::write(&config_path, format!("{json}\n"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_global_config() {
        let cfg = GlobalConfig::default();
        assert_eq!(cfg.profile, Profile::Core);
        assert_eq!(cfg.delivery, Delivery::Both);
        assert!(cfg.feature_flags.is_empty());
    }

    #[test]
    fn profile_serde_roundtrip() {
        assert_eq!(
            serde_json::from_str::<Profile>("\"core\"").unwrap(),
            Profile::Core
        );
        assert_eq!(
            serde_json::from_str::<Profile>("\"custom\"").unwrap(),
            Profile::Custom
        );
    }

    #[test]
    fn delivery_serde_roundtrip() {
        assert_eq!(
            serde_json::from_str::<Delivery>("\"both\"").unwrap(),
            Delivery::Both
        );
        assert_eq!(
            serde_json::from_str::<Delivery>("\"skills\"").unwrap(),
            Delivery::Skills
        );
        assert_eq!(
            serde_json::from_str::<Delivery>("\"commands\"").unwrap(),
            Delivery::Commands
        );
    }

    #[test]
    fn global_config_json_roundtrip() {
        let cfg = GlobalConfig {
            profile: Profile::Custom,
            workflows: Some(vec!["explore".into(), "apply".into()]),
            default_store: Some("my-store".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: GlobalConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.profile, Profile::Custom);
        assert_eq!(back.default_store.as_deref(), Some("my-store"));
    }

    #[test]
    fn get_global_config_dir_falls_back() {
        // Just ensure it doesn't panic and returns something non-empty.
        let dir = get_global_config_dir();
        assert!(!dir.as_os_str().is_empty());
    }
}
