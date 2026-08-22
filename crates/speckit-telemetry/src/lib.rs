pub mod config;

use std::sync::{Arc, LazyLock, Mutex};

use config::{TelemetryConfig, get_telemetry_config, update_telemetry_config};

/// PostHog API key -- public key for client-side analytics.
/// This is safe to embed as it only allows sending events, not reading data.
#[allow(dead_code)]
const POSTHOG_API_KEY: &str = "phc_Hthu8YvaIJ9QaFKyTG4TbVwkbd5ktcAFzVTKeMmoW2g";

/// Using reverse proxy to avoid ad blockers and keep traffic on our domain.
#[allow(dead_code)]
const POSTHOG_HOST: &str = "https://edge.speckit.dev";

/// Timeout for telemetry HTTP requests.
#[allow(dead_code)]
const TELEMETRY_REQUEST_TIMEOUT_MS: u64 = 1000;

/// Values that mean "CI is not enabled" when stored in the `CI` env var.
static CI_DISABLED_VALUES: &[&str] = &["", "false", "0", "no", "off"];

/// Check if telemetry is enabled.
///
/// Precedence (first match wins):
/// 1. `SPECKIT_TELEMETRY=0` -> disabled
/// 2. `DO_NOT_TRACK=1` -> disabled
/// 3. CI set to a truthy/on value -> disabled
/// 4. global config `telemetry.enabled === false` -> disabled
/// 5. otherwise enabled (unset config means on; opt-out model)
pub fn is_telemetry_enabled() -> bool {
    // Check explicit opt-out.
    if let Ok(val) = std::env::var("SPECKIT_TELEMETRY")
        && val == "0" {
            return false;
        }

    // Respect DO_NOT_TRACK standard.
    if let Ok(val) = std::env::var("DO_NOT_TRACK")
        && val == "1" {
            return false;
        }

    // Auto-disable in CI environments.
    if is_ci_environment() {
        return false;
    }

    // Global config opt-out.
    let config = get_telemetry_config();
    if config.enabled == Some(false) {
        return false;
    }

    true
}

/// Returns `true` when the `CI` environment variable is set to a truthy value.
fn is_ci_environment() -> bool {
    match std::env::var("CI") {
        Ok(val) => !CI_DISABLED_VALUES.contains(&val.trim().to_lowercase().as_str()),
        Err(_) => false,
    }
}

/// Get or create the anonymous user ID.
///
/// Lazily generates a UUID on first call and persists it.
pub fn get_or_create_anonymous_id() -> String {
    // Try to load from config.
    let config = get_telemetry_config();
    if let Some(id) = config.anonymous_id {
        return id;
    }

    // Generate new UUID and persist.
    let id = uuid::Uuid::new_v4().to_string();
    let _ = update_telemetry_config(&TelemetryConfig {
        anonymous_id: Some(id.clone()),
        ..Default::default()
    });
    id
}

/// Track a command execution.
///
/// Fire-and-forget: bounded by the request timeout, never throws, never retries.
#[cfg(feature = "telemetry")]
pub async fn track_command(command_name: &str, version: &str) {
    if !is_telemetry_enabled() {
        return;
    }

    let user_id = get_or_create_anonymous_id();

    let body = serde_json::json!({
        "api_key": POSTHOG_API_KEY,
        "batch": [{
            "type": "capture",
            "event": "command_executed",
            "distinct_id": user_id,
            "properties": {
                "command": command_name,
                "version": version,
                "surface": "cli",
                "$ip": null,
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }]
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(
            TELEMETRY_REQUEST_TIMEOUT_MS,
        ))
        .build()
        .unwrap_or_default();

    let _ = client
        .post(format!("{POSTHOG_HOST}/batch/"))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await;
}

/// Track a command execution (no-op when telemetry feature is disabled).
#[cfg(not(feature = "telemetry"))]
pub async fn track_command(_command_name: &str, _version: &str) {
    // Telemetry feature not enabled; no-op.
}

/// Show first-run telemetry notice if not already seen.
pub async fn maybe_show_telemetry_notice(options: TelemetryNoticeOptions) {
    if !is_telemetry_enabled() {
        return;
    }

    let config = get_telemetry_config();
    if config.notice_seen == Some(true) {
        return;
    }

    // In --json mode, skip the notice but leave noticeSeen unset.
    if options.silent {
        return;
    }

    println!(
        "Note: Speckit collects anonymous usage stats. Opt out: \
         SPECKIT_TELEMETRY=0 or speckit config set telemetry.enabled false"
    );

    let _ = update_telemetry_config(&TelemetryConfig {
        notice_seen: Some(true),
        ..Default::default()
    });
}

/// Options for the telemetry notice.
#[derive(Debug, Clone, Default)]
pub struct TelemetryNoticeOptions {
    /// When true, suppress the notice (e.g., in --json mode).
    pub silent: bool,
}

/// Pending events tracker for graceful shutdown.
static PENDING_EVENTS: LazyLock<Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));

/// Flush pending telemetry events. Call this before CLI exit.
pub async fn shutdown() {
    let handles: Vec<_> = {
        let mut pending = PENDING_EVENTS.lock().unwrap();
        std::mem::take(&mut *pending)
    };

    for handle in handles {
        let _ = handle.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ci_detection() {
        // Save original value.
        let original = std::env::var("CI").ok();

        // SAFETY: tests run single-threaded per process and restore the original value.
        unsafe {
            std::env::set_var("CI", "true");
        }
        assert!(is_ci_environment());

        unsafe {
            std::env::set_var("CI", "false");
        }
        assert!(!is_ci_environment());

        unsafe {
            std::env::set_var("CI", "0");
        }
        assert!(!is_ci_environment());

        unsafe {
            std::env::set_var("CI", "1");
        }
        assert!(is_ci_environment());

        // Restore original.
        match original {
            Some(val) => unsafe { std::env::set_var("CI", val) },
            None => unsafe { std::env::remove_var("CI") },
        }
    }

    #[test]
    fn telemetry_disabled_by_env() {
        let original_speckit = std::env::var("SPECKIT_TELEMETRY").ok();
        let original_dnt = std::env::var("DO_NOT_TRACK").ok();

        // SAFETY: tests run single-threaded per process and restore the original values.
        unsafe {
            std::env::set_var("SPECKIT_TELEMETRY", "0");
        }
        assert!(!is_telemetry_enabled());

        unsafe {
            std::env::remove_var("SPECKIT_TELEMETRY");
            std::env::set_var("DO_NOT_TRACK", "1");
        }
        assert!(!is_telemetry_enabled());

        // Restore.
        match original_speckit {
            Some(val) => unsafe { std::env::set_var("SPECKIT_TELEMETRY", val) },
            None => unsafe { std::env::remove_var("SPECKIT_TELEMETRY") },
        }
        match original_dnt {
            Some(val) => unsafe { std::env::set_var("DO_NOT_TRACK", val) },
            None => unsafe { std::env::remove_var("DO_NOT_TRACK") },
        }
    }

    #[test]
    fn anonymous_id_generation() {
        let id = get_or_create_anonymous_id();
        assert!(!id.is_empty());
        // Should be a valid UUID.
        assert!(uuid::Uuid::parse_str(&id).is_ok());
    }
}
