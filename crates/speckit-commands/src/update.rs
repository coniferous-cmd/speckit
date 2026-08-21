//! Update command implementation.

use anyhow::Result;
use std::path::Path;

use speckit_core::update::{UpdateCommand, UpdateCommandOptions};

/// Execute the update command.
pub fn execute(target_path: &Path, force: bool) -> Result<()> {
    let options = UpdateCommandOptions { force };
    let command = UpdateCommand::new(options);
    command.execute(target_path)
}
