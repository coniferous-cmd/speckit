//! View command implementation.

use anyhow::Result;
use std::path::Path;

use speckit_core::view::ViewCommand;

/// Execute the view command.
pub fn execute(target_path: &Path) -> Result<()> {
    let command = ViewCommand::new();
    command.execute(target_path)
}
