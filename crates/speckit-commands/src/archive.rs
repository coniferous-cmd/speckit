//! Archive command implementation.

use anyhow::Result;
use std::path::Path;

use speckit_core::archive::{ArchiveCommand, ArchiveOptions};
use speckit_core::root_selection::{ResolveSpeckitRootOptions, resolve_speckit_root};

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
    let root = resolve_speckit_root(&ResolveSpeckitRootOptions {
        store: store.clone(),
        store_path: None,
        start_path: Some(project_path.to_path_buf()),
        allow_implicit_root: Some(true),
        global_data_dir: None,
    })?;
    let options = ArchiveOptions {
        yes,
        skip_specs,
        no_validate,
        json,
        store,
        ..Default::default()
    };
    ArchiveCommand::execute(change_name, &options, &root.path)
}
