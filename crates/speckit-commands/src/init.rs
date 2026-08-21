//! Init command implementation.

use anyhow::Result;
use std::path::Path;

use speckit_core::init::{InitCommand, InitCommandOptions};

/// Execute the init command.
pub fn execute(
    target_path: &Path,
    tools: Option<String>,
    force: bool,
    profile: Option<String>,
    animation: bool,
    copilot_cloud: Option<bool>,
) -> Result<()> {
    let options = InitCommandOptions {
        tools,
        force,
        interactive: None,
        profile,
        animation,
        copilot_cloud,
    };

    let command = InitCommand::new(options);
    command.execute(target_path)
}
