use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::artifact_graph::resolve_artifact_outputs;
use crate::parsers::parse_delta_spec;

/// Finds Markdown files whose delta sections would be ignored by the spec readers.
pub fn find_misplaced_delta_specs(change_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let change_dir = dunce::canonicalize(change_dir)?;
    let specs_dir = change_dir.join("specs");
    let mut misplaced = Vec::new();
    for path in resolve_artifact_outputs(&change_dir, "specs/**/*.[mM][dD]") {
        if path.file_name().is_some_and(|name| name == "spec.md") {
            continue;
        }
        let Ok(relative) = path.strip_prefix(&specs_dir) else {
            continue;
        };
        if relative
            .iter()
            .any(|part| part.to_string_lossy().starts_with('.'))
        {
            continue;
        }
        let content = std::fs::read_to_string(&path)?;
        let sections = parse_delta_spec(&content).section_presence;
        if sections.added || sections.modified || sections.removed || sections.renamed {
            misplaced.push(relative.to_path_buf());
        }
    }
    Ok(misplaced)
}

/// Discover spec files in a directory.
pub fn discover_spec_files(specs_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut spec_files = Vec::new();

    if !specs_dir.exists() {
        return Ok(spec_files);
    }

    for entry in WalkDir::new(specs_dir)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
    {
        let entry = entry?;
        let path = entry.path();

        if path.is_file()
            && let Some(ext) = path.extension()
            && ext == "md"
        {
            spec_files.push(path.to_path_buf());
        }
    }

    Ok(spec_files)
}

/// Check if a directory has any files under it.
pub fn has_any_file_under(dir: &Path) -> bool {
    if !dir.exists() {
        return false;
    }

    WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
        .any(|entry| entry.map(|e| e.file_type().is_file()).unwrap_or(false))
}

/// Check if a directory entry is hidden (starts with .).
fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Find all capability directories under specs/.
pub fn find_capability_dirs(specs_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();

    if !specs_dir.exists() {
        return Ok(dirs);
    }

    for entry in std::fs::read_dir(specs_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() && !is_hidden_entry(&path) {
            dirs.push(path);
        }
    }

    Ok(dirs)
}

fn is_hidden_entry(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_spec_files_nonexistent() {
        let result = discover_spec_files(Path::new("/nonexistent/path")).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_has_any_file_under_nonexistent() {
        assert!(!has_any_file_under(Path::new("/nonexistent/path")));
    }
}
