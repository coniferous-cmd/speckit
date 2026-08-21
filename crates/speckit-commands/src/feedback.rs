//! Feedback Command
//!
//! Submit feedback about Speckit via GitHub CLI.

use crate::shared_output::StoreDiagnostic;

/// Execute the feedback command.
pub async fn feedback_command(message: &str, body: Option<&str>) -> anyhow::Result<()> {
    let title = format!("Feedback: {message}");
    let body_text = format_body(body);

    // Check if gh CLI is installed
    if !is_gh_installed() {
        handle_fallback(&title, &body_text, "missing");
        return Ok(());
    }

    // Check if gh CLI is authenticated
    if !is_gh_authenticated() {
        handle_fallback(&title, &body_text, "unauthenticated");
        return Ok(());
    }

    // Submit via gh CLI
    submit_via_gh_cli(&title, &body_text)?;
    Ok(())
}

/// Check if gh CLI is installed.
fn is_gh_installed() -> bool {
    std::process::Command::new("which")
        .arg("gh")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Check if gh CLI is authenticated.
fn is_gh_authenticated() -> bool {
    std::process::Command::new("gh")
        .arg("auth")
        .arg("status")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Get the Speckit version.
fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get the platform name.
fn get_platform() -> String {
    std::env::consts::OS.to_string()
}

/// Generate metadata footer for feedback.
fn generate_metadata() -> String {
    let version = get_version();
    let platform = get_platform();
    let timestamp = chrono::Utc::now().to_rfc3339();

    format!(
        "---\nSubmitted via Speckit CLI\n- Version: {version}\n- Platform: {platform}\n- Timestamp: {timestamp}"
    )
}

/// Format the full feedback body.
fn format_body(body_text: Option<&str>) -> String {
    let mut parts = Vec::new();

    if let Some(body) = body_text {
        parts.push(body.to_string());
        parts.push(String::new());
    }

    parts.push(generate_metadata());
    parts.join("\n")
}

/// Generate a pre-filled GitHub issue URL for manual submission.
fn generate_manual_submission_url(title: &str, body: &str) -> String {
    let repo = "Fission-AI/Speckit";
    let encoded_title = urlencoding::encode(title);
    let encoded_body = urlencoding::encode(body);
    let encoded_labels = urlencoding::encode("feedback");

    format!(
        "https://github.com/{repo}/issues/new?title={encoded_title}&body={encoded_body}&labels={encoded_labels}"
    )
}

/// Display formatted feedback content for manual submission.
fn display_formatted_feedback(title: &str, body: &str) {
    println!();
    println!("--- FORMATTED FEEDBACK ---");
    println!("Title: {title}");
    println!("Labels: feedback");
    println!();
    println!("Body:");
    println!("{body}");
    println!("--- END FEEDBACK ---");
    println!();
}

/// Handle fallback when gh CLI is not available or not authenticated.
fn handle_fallback(title: &str, body: &str, reason: &str) {
    if reason == "missing" {
        println!("GitHub CLI not found. Manual submission required.");
    } else {
        println!("GitHub authentication required. Manual submission required.");
    }

    display_formatted_feedback(title, body);

    let manual_url = generate_manual_submission_url(title, body);
    println!("Please submit your feedback manually:");
    println!("{manual_url}");

    if reason == "unauthenticated" {
        println!();
        println!("To auto-submit in the future: gh auth login");
    }
}

/// Create the feedback issue via gh CLI.
fn create_issue(title: &str, body: &str, labels: &[&str]) -> anyhow::Result<String> {
    let mut cmd = std::process::Command::new("gh");
    cmd.arg("issue")
        .arg("create")
        .arg("--repo")
        .arg("Fission-AI/Speckit")
        .arg("--title")
        .arg(title)
        .arg("--body")
        .arg(body);

    for label in labels {
        cmd.arg("--label").arg(label);
    }

    let output = cmd.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh issue create failed: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Submit feedback via gh CLI.
fn submit_via_gh_cli(title: &str, body: &str) -> anyhow::Result<()> {
    // Try with label first
    match create_issue(title, body, &["feedback"]) {
        Ok(issue_url) => {
            println!();
            println!("\u{2713} Feedback submitted successfully!");
            println!("Issue URL: {issue_url}");
            println!();
            Ok(())
        }
        Err(e) => {
            let stderr = e.to_string();
            if stderr.contains("could not add label") {
                // Label doesn't exist, retry without it
                match create_issue(title, body, &[]) {
                    Ok(issue_url) => {
                        println!();
                        println!("\u{2713} Feedback submitted successfully!");
                        println!("Issue URL: {issue_url}");
                        println!();
                        println!(
                            "Note: created without the 'feedback' label because the repository does not define it."
                        );
                        println!();
                        Ok(())
                    }
                    Err(retry_err) => {
                        report_gh_failure(&retry_err.to_string(), title, body);
                        std::process::exit(1);
                    }
                }
            } else {
                report_gh_failure(&stderr, title, body);
                std::process::exit(1);
            }
        }
    }
}

/// Report a gh CLI failure.
fn report_gh_failure(stderr: &str, title: &str, body: &str) {
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }

    display_formatted_feedback(title, body);

    let manual_url = generate_manual_submission_url(title, body);
    println!("Please submit your feedback manually:");
    println!("{manual_url}");
}

/// URL encoding module (minimal implementation to avoid external dependency).
mod urlencoding {
    pub fn encode(input: &str) -> String {
        let mut result = String::new();
        for byte in input.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    result.push(byte as char);
                }
                b' ' => result.push('+'),
                _ => {
                    result.push('%');
                    result.push_str(&format!("{byte:02X}"));
                }
            }
        }
        result
    }
}
