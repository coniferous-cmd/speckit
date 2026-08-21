//! Archive command implementation.

use anyhow::Result;
use std::path::Path;

use speckit_core::archive::{ArchiveCommand, ArchiveOptions};

/// Execute the archive command.
pub fn execute(
    change_name: Option<&str>,
    yes: bool,
    skip_specs: bool,
    no_validate: bool,
    json: bool,
    store: Option<String>,
    project_path: &Path,
) -> Result<Option<speckit_core::archive::ArchiveResult>> {
    let options = ArchiveOptions {
        yes,
        skip_specs,
        no_validate,
        json,
        store,
        ..Default::default()
    };
    ArchiveCommand::execute(change_name, &options, project_path)
}
