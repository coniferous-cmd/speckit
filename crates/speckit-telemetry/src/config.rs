use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Telemetry section of the global config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryConfig {
    /// When `false`, telemetry is disabled. Unset means enabled (opt-out model).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Anonymous random UUID; no relation to the user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anonymous_id: Option<String>,
    /// Whether the first-run telemetry notice has been shown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice_seen: Option<bool>,
}

/// Top-level global config file structure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalConfigFile {
    #[serde(default)]
    pub telemetry: Option<TelemetryConfig>,
    /// Preserve other fields.
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

const CONFIG_DIR_NAME: &str = "speckit";
const CONFIG_FILE_NAME: &str = "config.json";

/// Returns the path to the global config directory.
fn get_config_dir() -> PathBuf {
    // XDG_CONFIG_HOME takes precedence.
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty() {
            return PathBuf::from(xdg).join(CONFIG_DIR_NAME);
        }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            return PathBuf::from(app_data).join(CONFIG_DIR_NAME);
        }
    }

    dirs::home_dir()
        .map(|h| h.join(".config").join(CONFIG_DIR_NAME))
        .unwrap_or_else(|| PathBuf::from(".config").join(CONFIG_DIR_NAME))
}

/// Returns the full path to the global config file.
fn get_config_path() -> PathBuf {
    get_config_dir().join(CONFIG_FILE_NAME)
}

/// Returns the legacy config path (`~/.config/speckit/config.json`).
fn get_legacy_config_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| {
            h.join(".config")
                .join(CONFIG_DIR_NAME)
                .join(CONFIG_FILE_NAME)
        })
        .unwrap_or_else(|| {
            PathBuf::from(".config")
                .join(CONFIG_DIR_NAME)
                .join(CONFIG_FILE_NAME)
        })
}

/// Reads the config file and returns its contents.
fn read_config_file(config_path: &PathBuf) -> Option<GlobalConfigFile> {
    let content = fs::read_to_string(config_path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Writes the config file.
fn write_config_file(config_path: &PathBuf, config: &GlobalConfigFile) -> std::io::Result<()> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    fs::write(config_path, format!("{json}\n"))
}

/// Merges legacy telemetry config from the default config path into the current config.
fn migrate_legacy_telemetry(config_path: &PathBuf, config: &mut GlobalConfigFile) -> bool {
    let legacy_path = get_legacy_config_path();
    if dunce::canonicalize(config_path).ok() == dunce::canonicalize(&legacy_path).ok() {
        return false;
    }

    match &config.telemetry {
        Some(t) if t.anonymous_id.is_some() && t.notice_seen.is_some() => return false,
        _ => {}
    };

    let legacy_config = match read_config_file(&legacy_path) {
        Some(c) => c,
        None => return false,
    };

    let legacy_telemetry = match &legacy_config.telemetry {
        Some(t) => t,
        None => return false,
    };

    let current_telemetry = config
        .telemetry
        .get_or_insert_with(TelemetryConfig::default);
    let mut changed = false;

    if current_telemetry.anonymous_id.is_none() && legacy_telemetry.anonymous_id.is_some() {
        current_telemetry.anonymous_id = legacy_telemetry.anonymous_id.clone();
        changed = true;
    }

    if current_telemetry.notice_seen.is_none() && legacy_telemetry.notice_seen.is_some() {
        current_telemetry.notice_seen = legacy_telemetry.notice_seen;
        changed = true;
    }

    changed
}

/// Reads the global config file.
///
/// Returns a default empty config if the file doesn't exist or is invalid.
fn read_config() -> GlobalConfigFile {
    let config_path = get_config_path();
    let mut config = read_config_file(&config_path).unwrap_or_default();

    // Try migrating legacy telemetry.
    if migrate_legacy_telemetry(&config_path, &mut config) {
        let _ = write_config_file(&config_path, &config);
    }

    config
}

/// Writes to the global config file, preserving existing fields.
fn write_config(updates: &GlobalConfigFile) -> std::io::Result<()> {
    let config_path = get_config_path();
    let existing = read_config();

    let merged = GlobalConfigFile {
        telemetry: match (&existing.telemetry, &updates.telemetry) {
            (Some(existing_t), Some(updates_t)) => Some(TelemetryConfig {
                enabled: updates_t.enabled.or(existing_t.enabled),
                anonymous_id: updates_t
                    .anonymous_id
                    .clone()
                    .or_else(|| existing_t.anonymous_id.clone()),
                notice_seen: updates_t.notice_seen.or(existing_t.notice_seen),
            }),
            (None, Some(t)) => Some(t.clone()),
            (Some(t), None) => Some(t.clone()),
            (None, None) => None,
        },
        extra: existing.extra,
    };

    write_config_file(&config_path, &merged)
}

/// Gets the telemetry config section.
pub fn get_telemetry_config() -> TelemetryConfig {
    read_config().telemetry.unwrap_or_default()
}

/// Updates the telemetry config section.
pub fn update_telemetry_config(updates: &TelemetryConfig) -> std::io::Result<()> {
    let existing = get_telemetry_config();

    let merged = TelemetryConfig {
        enabled: updates.enabled.or(existing.enabled),
        anonymous_id: updates
            .anonymous_id
            .clone()
            .or_else(|| existing.anonymous_id.clone()),
        notice_seen: updates.notice_seen.or(existing.notice_seen),
    };

    write_config(&GlobalConfigFile {
        telemetry: Some(merged),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_config_default() {
        let config = TelemetryConfig::default();
        assert!(config.enabled.is_none());
        assert!(config.anonymous_id.is_none());
        assert!(config.notice_seen.is_none());
    }

    #[test]
    fn telemetry_config_serde_roundtrip() {
        let config = TelemetryConfig {
            enabled: Some(false),
            anonymous_id: Some("test-id".to_string()),
            notice_seen: Some(true),
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: TelemetryConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.enabled, Some(false));
        assert_eq!(back.anonymous_id.as_deref(), Some("test-id"));
        assert_eq!(back.notice_seen, Some(true));
    }

    #[test]
    fn global_config_file_serde_roundtrip() {
        let config = GlobalConfigFile {
            telemetry: Some(TelemetryConfig {
                enabled: Some(true),
                ..Default::default()
            }),
            extra: serde_json::json!({"profile": "core"}),
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: GlobalConfigFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.telemetry.unwrap().enabled, Some(true));
    }
}
