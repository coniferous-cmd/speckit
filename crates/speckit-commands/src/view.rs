//! View command implementation.

use anyhow::Result;
use std::path::Path;

use speckit_core::view::ViewCommand;

/// Execute the view command.
///
/// `store_id`, when provided, selects a registered store via the unified
/// root resolver instead of the working directory.
pub fn execute(target_path: &Path, store_id: Option<&str>) -> Result<()> {
    let command = ViewCommand::new();
    command.execute(target_path, store_id)
}
