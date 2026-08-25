use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Global config schema types  (mirrors GlobalConfigSchema in config-schema.ts)
// ---------------------------------------------------------------------------

/// Schema-validated shape of the global configuration file.
///
/// Uses `serde(flatten)` on the top level to preserve unknown top-level
/// fields for forward compatibility (the Rust equivalent of Zod's
/// `passthrough()`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalConfigSchema {
    #[serde(default)]
    pub feature_flags: HashMap<String, bool>,
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default = "default_delivery")]
    pub delivery: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflows: Option<Vec<String>>,
    /// Store id used as fallback root when no explicit `--store`, local root,
    /// or project-level store pointer resolves.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_store: Option<String>,
    /// Catch-all for forward-compatible top-level fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_profile() -> String {
    "core".into()
}
fn default_delivery() -> String {
    "both".into()
}

/// Default config values (mirrors `DEFAULT_CONFIG` in config-schema.ts).
pub fn default_global_config_schema() -> GlobalConfigSchema {
    GlobalConfigSchema {
        feature_flags: HashMap::new(),
        profile: "core".into(),
        delivery: "both".into(),
        workflows: None,
        default_store: None,
        extra: HashMap::new(),
    }
}

// ---------------------------------------------------------------------------
// Key-path validation for CLI `set` operations
// ---------------------------------------------------------------------------

/// Key segments that would reach the prototype chain instead of the config
/// object.  Never valid as configuration keys, so rejecting them costs nothing.
const UNSAFE_KEY_SEGMENTS: &[&str] = &["__proto__", "constructor", "prototype"];

fn has_unsafe_segment(keys: &[&str]) -> bool {
    keys.iter().any(|k| UNSAFE_KEY_SEGMENTS.contains(k))
}

/// Returns `true` when a dot-notation key path contains a prototype-reaching
/// segment.  Callers that bypass key validation (e.g. `--allow-unknown`) still
/// must not bypass this.
pub fn has_unsafe_key_segment(path: &str) -> bool {
    let keys: Vec<&str> = path.split('.').collect();
    has_unsafe_segment(&keys)
}

/// Validation result for a config key path.
#[derive(Debug, Clone)]
pub struct KeyPathValidation {
    pub valid: bool,
    pub reason: Option<String>,
}

/// Validate a config key path for CLI set operations.
///
/// Unknown top-level keys are rejected unless explicitly allowed by the caller.
pub fn validate_config_key_path(path: &str) -> KeyPathValidation {
    let raw_keys: Vec<&str> = path.split('.').collect();

    if raw_keys.is_empty() || raw_keys.iter().any(|k| k.trim().is_empty()) {
        return KeyPathValidation {
            valid: false,
            reason: Some("Key path must not be empty".into()),
        };
    }

    if let Some(unsafe_key) = raw_keys.iter().find(|k| UNSAFE_KEY_SEGMENTS.contains(k)) {
        return KeyPathValidation {
            valid: false,
            reason: Some(format!("Key segment \"{unsafe_key}\" is not allowed")),
        };
    }

    let root_key = raw_keys[0];
    let known_top_level = [
        "feature_flags",
        "profile",
        "delivery",
        "workflows",
        "default_store",
    ];
    // Accept both camelCase and snake_case root keys for ergonomics.
    let root_key_norm = root_key.replace('-', "_");
    if !known_top_level.contains(&root_key_norm.as_str()) {
        return KeyPathValidation {
            valid: false,
            reason: Some(format!("Unknown top-level key \"{root_key}\"")),
        };
    }

    if root_key_norm == "feature_flags" {
        if raw_keys.len() > 2 {
            return KeyPathValidation {
                valid: false,
                reason: Some(
                    "featureFlags values are booleans and do not support nested keys".into(),
                ),
            };
        }
        return KeyPathValidation {
            valid: true,
            reason: None,
        };
    }

    if raw_keys.len() > 1 {
        return KeyPathValidation {
            valid: false,
            reason: Some(format!("\"{root_key}\" does not support nested keys")),
        };
    }

    KeyPathValidation {
        valid: true,
        reason: None,
    }
}

// ---------------------------------------------------------------------------
// Nested value helpers (get / set / delete on serde_json::Value)
// ---------------------------------------------------------------------------

/// Get a nested value from a JSON object using dot notation.
pub fn get_nested_value<'a>(
    obj: &'a serde_json::Value,
    path: &str,
) -> Option<&'a serde_json::Value> {
    let keys: Vec<&str> = path.split('.').collect();
    if has_unsafe_segment(&keys) {
        return None;
    }
    let mut current = obj;
    for key in keys {
        current = current.get(key)?;
    }
    Some(current)
}

/// Set a nested value in a mutable JSON object using dot notation.
/// Creates intermediate objects as needed.
pub fn set_nested_value(obj: &mut serde_json::Value, path: &str, value: serde_json::Value) {
    let keys: Vec<&str> = path.split('.').collect();

    // Reject unsafe segments before any mutation.
    if has_unsafe_segment(&keys) {
        return;
    }

    let mut current = obj;
    for i in 0..keys.len() - 1 {
        let key = keys[i];
        if !current.is_object()
            || current.get(key).is_none()
            || !current.get(key).unwrap().is_object()
        {
            current[key] = serde_json::Value::Object(serde_json::Map::new());
        }
        current = &mut current[key];
    }

    if let Some(last_key) = keys.last() {
        current[*last_key] = value;
    }
}

/// Delete a nested value from a JSON object using dot notation.
/// Returns `true` if the key existed and was deleted.
pub fn delete_nested_value(obj: &mut serde_json::Value, path: &str) -> bool {
    let keys: Vec<&str> = path.split('.').collect();

    if has_unsafe_segment(&keys) {
        return false;
    }

    let mut current = obj;
    for i in 0..keys.len() - 1 {
        let key = keys[i];
        match current.get_mut(key) {
            Some(v) if v.is_object() => current = v,
            _ => return false,
        }
    }

    if let Some(last_key) = keys.last()
        && let Some(map) = current.as_object_mut()
    {
        return map.remove(*last_key).is_some();
    }
    false
}

// ---------------------------------------------------------------------------
// Value coercion (string -> typed value)
// ---------------------------------------------------------------------------

/// Coerce a string value to its appropriate JSON type.
///
/// - `"true"` / `"false"` -> boolean
/// - Numeric strings -> number
/// - JSON arrays/objects -> parsed containers
/// - Everything else -> string
pub fn coerce_value(value: &str, force_string: bool) -> serde_json::Value {
    if force_string {
        return serde_json::Value::String(value.to_string());
    }

    // Boolean coercion.
    if value == "true" {
        return serde_json::Value::Bool(true);
    }
    if value == "false" {
        return serde_json::Value::Bool(false);
    }

    // Number coercion -- must be a valid finite number.
    // Try integer first so that "42" becomes 42 (not 42.0).
    if let Ok(num) = value.parse::<i64>()
        && !value.trim().is_empty()
    {
        return serde_json::Value::Number(num.into());
    }
    if let Ok(num) = value.parse::<f64>()
        && num.is_finite()
        && !value.trim().is_empty()
        && let Some(n) = serde_json::Number::from_f64(num)
    {
        return serde_json::Value::Number(n);
    }

    // JSON container coercion.
    if let Some(container) = parse_json_container(value) {
        return container;
    }

    serde_json::Value::String(value.to_string())
}

fn parse_json_container(value: &str) -> Option<serde_json::Value> {
    let trimmed = value.trim();
    let looks_like_container = (trimmed.starts_with('[') && trimmed.ends_with(']'))
        || (trimmed.starts_with('{') && trimmed.ends_with('}'));

    if !looks_like_container {
        return None;
    }

    let parsed: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    match &parsed {
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => Some(parsed),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// YAML-like display formatter
// ---------------------------------------------------------------------------

/// Format a JSON value for YAML-like display.
pub fn format_value_yaml(value: &serde_json::Value, indent: usize) -> String {
    let indent_str = "  ".repeat(indent);

    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                return "[]".to_string();
            }
            arr.iter()
                .map(|item| format!("{indent_str}- {}", format_value_yaml(item, indent + 1)))
                .collect::<Vec<_>>()
                .join("\n")
        }
        serde_json::Value::Object(map) => {
            if map.is_empty() {
                return "{}".to_string();
            }
            map.iter()
                .map(|(key, val)| {
                    let formatted = format_value_yaml(val, indent + 1);
                    if val.is_object() && !val.as_object().unwrap().is_empty() {
                        format!("{indent_str}{key}:\n{formatted}")
                    } else {
                        format!("{indent_str}{key}: {formatted}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

// ---------------------------------------------------------------------------
// Config validation
// ---------------------------------------------------------------------------

/// Validate a configuration value against the global config schema by
/// round-tripping through serde.
pub fn validate_config(config: &serde_json::Value) -> Result<(), String> {
    serde_json::from_value::<GlobalConfigSchema>(config.clone())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_key_segments_rejected() {
        assert!(has_unsafe_key_segment("featureFlags.__proto__"));
        assert!(has_unsafe_key_segment("constructor.foo"));
        assert!(!has_unsafe_key_segment("featureFlags.safeFlag"));
    }

    #[test]
    fn validate_known_paths() {
        let r = validate_config_key_path("profile");
        assert!(r.valid);
        let r = validate_config_key_path("feature_flags.myFlag");
        assert!(r.valid);
    }

    #[test]
    fn validate_unknown_root_key() {
        let r = validate_config_key_path("unknownKey");
        assert!(!r.valid);
        assert!(r.reason.as_ref().unwrap().contains("Unknown top-level key"));
    }

    #[test]
    fn validate_nested_on_scalar_root() {
        let r = validate_config_key_path("profile.nested");
        assert!(!r.valid);
        assert!(
            r.reason
                .as_ref()
                .unwrap()
                .contains("does not support nested keys")
        );
    }

    #[test]
    fn nested_get_set_delete() {
        let mut obj = serde_json::json!({"a": {"b": 1}});
        assert_eq!(get_nested_value(&obj, "a.b"), Some(&serde_json::json!(1)));

        set_nested_value(&mut obj, "a.c", serde_json::json!("hello"));
        assert_eq!(
            get_nested_value(&obj, "a.c"),
            Some(&serde_json::json!("hello"))
        );

        assert!(delete_nested_value(&mut obj, "a.b"));
        assert_eq!(get_nested_value(&obj, "a.b"), None);
    }

    #[test]
    fn coerce_booleans() {
        assert_eq!(coerce_value("true", false), serde_json::Value::Bool(true));
        assert_eq!(coerce_value("false", false), serde_json::Value::Bool(false));
    }

    #[test]
    fn coerce_numbers() {
        assert_eq!(
            coerce_value("42", false),
            serde_json::Value::Number(42.into())
        );
    }

    #[test]
    fn coerce_force_string() {
        assert_eq!(
            coerce_value("42", true),
            serde_json::Value::String("42".into())
        );
    }

    #[test]
    fn coerce_json_container() {
        let v = coerce_value("[1,2,3]", false);
        assert!(v.is_array());
        let v = coerce_value("{\"a\":1}", false);
        assert!(v.is_object());
    }
}
