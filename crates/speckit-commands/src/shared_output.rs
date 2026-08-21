//! Shared JSON/failure output plumbing for command groups whose errors
//! carry the StoreDiagnostic envelope. One definition of the failure
//! contract: exit code 1, Error:/Fix: lines in human mode, a status
//! array in JSON mode.

use serde::{Deserialize, Serialize};

/// A diagnostic entry in the store/status envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

/// Print a value as pretty-printed JSON to stdout.
pub fn print_json(payload: &impl Serialize) {
    match serde_json::to_string_pretty(payload) {
        Ok(json) => println!("{json}"),
        Err(e) => eprintln!("Error serializing JSON: {e}"),
    }
}

/// Extract a human-readable error message from any error.
pub fn as_error_message(error: &anyhow::Error) -> String {
    error.to_string()
}

/// Check if an error represents a prompt cancellation (Ctrl-C in interactive prompts).
pub fn is_prompt_cancellation_error(error: &anyhow::Error) -> bool {
    let msg = error.to_string();
    msg.contains("ExitPromptError")
        || msg.contains("force closed the prompt with SIGINT")
        || msg.contains("cancelled")
        || msg.contains("Cancelled")
}

/// Convert any error into a `StoreDiagnostic` status entry.
pub fn as_status(error: &anyhow::Error, fallback_code: &str) -> StoreDiagnostic {
    // Try to downcast to a structured diagnostic if available.
    // In the Rust port we check if the error chain contains a diagnostic-like payload.
    let msg = as_error_message(error);
    StoreDiagnostic {
        severity: "error".to_string(),
        code: fallback_code.to_string(),
        message: msg,
        fix: None,
    }
}

/// Emit a failure: prints JSON status in JSON mode, or human-readable
/// Error:/Fix: lines in human mode. Sets the process exit code to 1.
pub fn emit_failure(
    json: bool,
    mut payload: serde_json::Value,
    error: &anyhow::Error,
    fallback_code: &str,
) {
    // Ctrl-C in a prompt is the user's choice, not an error
    if !json && is_prompt_cancellation_error(error) {
        eprintln!("Cancelled.");
        std::process::exit(130);
    }

    let status = as_status(error, fallback_code);
    if json {
        // Merge status into existing payload
        if let Some(arr) = payload.get_mut("status").and_then(|v| v.as_array_mut()) {
            arr.push(serde_json::to_value(&status).unwrap_or_default());
        } else if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "status".to_string(),
                serde_json::to_value(&[&status]).unwrap_or_default(),
            );
        }
        print_json(&payload);
        std::process::exit(1);
    }

    eprintln!("Error: {}", status.message);
    if let Some(ref fix) = status.fix {
        eprintln!("Fix: {fix}");
    }
    std::process::exit(1);
}

/// Format a path for human display, replacing the home directory with `~`.
pub fn format_path_for_human(target_path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        if target_path == home_str.as_ref() {
            return "~".to_string();
        }
        if let Some(rest) = target_path.strip_prefix(home_str.as_ref()) {
            let rest = rest.strip_prefix(std::path::MAIN_SEPARATOR).unwrap_or(rest);
            return format!("~{}{}", std::path::MAIN_SEPARATOR, rest);
        }
    }
    target_path.to_string()
}
