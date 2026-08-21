//! Version check: detects available CLI updates and offers upgrades.

use std::path::{Path, PathBuf};

/// Package name for version checks.
const PACKAGE_NAME: &str = "@fission-ai/speckit";

/// Version of this CLI.
const OPENSPEC_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Safe version pattern: SemVer only.
fn is_safe_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('-').next().unwrap_or("").split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

/// Compare two versions. Returns positive if a > b, negative if a < b, 0 if equal.
pub fn compare_versions(a: &str, b: &str) -> i32 {
    let parse = |v: &str| -> (Vec<u32>, String) {
        let without_build = v.trim().trim_start_matches('v');
        let separator = without_build.find('-');
        let core = match separator {
            Some(idx) => &without_build[..idx],
            None => without_build,
        };
        let prerelease = match separator {
            Some(idx) => without_build[idx + 1..].to_string(),
            None => String::new(),
        };
        let numbers: Vec<u32> = core.split('.').map(|n| n.parse().unwrap_or(0)).collect();
        (numbers, prerelease)
    };

    let (left_nums, left_pre) = parse(a);
    let (right_nums, right_pre) = parse(b);

    for i in 0..3 {
        let l = left_nums.get(i).copied().unwrap_or(0);
        let r = right_nums.get(i).copied().unwrap_or(0);
        if l > r {
            return 1;
        }
        if l < r {
            return -1;
        }
    }

    compare_prerelease(&left_pre, &right_pre)
}

/// Compare prerelease tags per SemVer.
fn compare_prerelease(a: &str, b: &str) -> i32 {
    if a == b {
        return 0;
    }
    if a.is_empty() {
        return 1; // Release > prerelease
    }
    if b.is_empty() {
        return -1;
    }

    let left: Vec<&str> = a.split('.').collect();
    let right: Vec<&str> = b.split('.').collect();

    for i in 0..std::cmp::max(left.len(), right.len()) {
        let l = left.get(i);
        let r = right.get(i);
        match (l, r) {
            (None, None) => break,
            (None, Some(_)) => return -1,
            (Some(_), None) => return 1,
            (Some(l_val), Some(r_val)) => {
                let l_num = l_val.parse::<u32>();
                let r_num = r_val.parse::<u32>();
                match (l_num, r_num) {
                    (Ok(ln), Ok(rn)) => {
                        if ln != rn {
                            return if ln > rn { 1 } else { -1 };
                        }
                    }
                    (Ok(_), Err(_)) => return -1,
                    (Err(_), Ok(_)) => return 1,
                    (Err(_), Err(_)) => {
                        if l_val != r_val {
                            return if l_val > r_val { 1 } else { -1 };
                        }
                    }
                }
            }
        }
    }

    0
}

/// Check if the update check is enabled.
fn is_check_enabled() -> bool {
    if std::env::var("OPENSPEC_NO_UPDATE_CHECK").is_ok() {
        return false;
    }
    if std::env::var("DO_NOT_TRACK").as_deref() == Ok("1") {
        return false;
    }
    if std::env::var("SPECKIT_TELEMETRY").as_deref() == Ok("0") {
        return false;
    }
    if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
        return false;
    }
    if std::env::var("NODE_ENV").as_deref() == Ok("test") {
        return false;
    }
    true
}

/// Returns the published version when the installed CLI is behind it.
pub fn get_available_cli_update() -> Option<String> {
    if !is_check_enabled() {
        return None;
    }

    let latest = fetch_latest_version()?;
    if compare_versions(&latest, OPENSPEC_VERSION) > 0 {
        Some(latest)
    } else {
        None
    }
}

/// Fetch the latest version from the npm registry.
///
/// Uses `npm view` to avoid adding an HTTP client dependency.
fn fetch_latest_version() -> Option<String> {
    let package = PACKAGE_NAME;
    let output = std::process::Command::new("npm")
        .args(["view", package, "version", "--silent"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if is_safe_version(&version) {
        Some(version)
    } else {
        None
    }
}

/// Directory the running CLI was loaded from.
pub fn get_install_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

/// Whether the CLI is running from a source checkout.
pub fn is_source_checkout(install_dir: Option<&Path>) -> bool {
    match install_dir {
        Some(dir) => dir.join(".git").exists(),
        None => false,
    }
}

/// Whether to offer the upgrade.
pub fn should_offer_upgrade(
    install_dir: Option<&Path>,
    interactive: bool,
    stdout_is_tty: bool,
) -> bool {
    if !interactive || !stdout_is_tty {
        return false;
    }
    // Only offer for non-source checkouts
    !is_source_checkout(install_dir)
}

/// Build CLI update notification lines.
pub fn build_cli_update_lines(latest_version: &str, install_dir: Option<&Path>) -> Vec<String> {
    let mut lines = vec![format!(
        "A newer Speckit CLI is available (v{} -> v{}).",
        OPENSPEC_VERSION, latest_version
    )];

    if let Some(dir) = install_dir {
        lines.push(format!("  Running from: {}", dir.display()));
    }

    lines
}

/// Build upgrade command lines.
pub fn build_upgrade_command_lines(install_dir: Option<&Path>) -> Vec<String> {
    let mut lines = Vec::new();

    if is_source_checkout(install_dir) {
        lines.push("  This is a source checkout. Pull the latest changes.".to_string());
    } else {
        lines.push(format!("  npm install -g {}@latest", PACKAGE_NAME));
    }

    lines.push("  Then run 'speckit update' again to pick up new workflows.".to_string());
    lines
}

/// Display the CLI update note.
pub fn display_cli_update_note(latest_version: &str, project_path: &Path) {
    let install_dir = get_install_dir();
    let lines = build_cli_update_lines(latest_version, install_dir.as_deref());

    println!();
    for line in &lines {
        println!("{}", line);
    }
}

/// Display just the upgrade command.
pub fn display_upgrade_command(project_path: &Path) {
    let install_dir = get_install_dir();
    let lines = build_upgrade_command_lines(install_dir.as_deref());
    for line in &lines {
        println!("{}", line);
    }
}

/// The outcome of an upgrade offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeOutcome {
    Upgraded,
    Declined,
    Failed,
    Cancelled,
    NotOnPath,
}

/// Offer to run the upgrade.
pub fn offer_cli_upgrade(latest_version: &str) -> UpgradeOutcome {
    println!("Upgrade to v{} now? [Y/n]", latest_version);

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return UpgradeOutcome::Cancelled;
    }

    if input.trim().to_lowercase() == "n" {
        return UpgradeOutcome::Declined;
    }

    // Attempt upgrade
    let output = std::process::Command::new("npm")
        .args(["install", "-g", &format!("{}@latest", PACKAGE_NAME)])
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                println!("Upgraded to v{}.", latest_version);
                UpgradeOutcome::Upgraded
            } else {
                println!("The upgrade did not complete.");
                UpgradeOutcome::Failed
            }
        }
        Err(_) => {
            println!("The upgrade did not complete.");
            UpgradeOutcome::Failed
        }
    }
}

/// Run the update command with the upgraded CLI.
pub fn rerun_update_with_upgraded_cli(project_path: &Path, force: bool) -> i32 {
    let mut args = vec!["update"];
    if force {
        args.push("--force");
    }
    args.push("--");
    let path_str = project_path.to_string_lossy();
    args.push(&path_str);

    match std::process::Command::new("speckit").args(&args).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(_) => {
            println!("Instruction files were not regenerated.");
            println!("  Run 'speckit update' to pick up the new workflows.");
            1
        }
    }
}
