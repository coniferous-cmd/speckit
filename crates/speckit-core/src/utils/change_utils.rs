use anyhow::Result;
use std::path::Path;

/// Check if a directory is a valid change directory.
pub fn is_change_directory(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }

    // A change directory should have a proposal.md or specs/ directory
    path.join("proposal.md").exists() || path.join("specs").exists()
}

/// Get the change name from a directory path.
pub fn get_change_name(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// List all changes in a changes directory.
pub fn list_changes(changes_dir: &Path) -> Result<Vec<String>> {
    let mut changes = Vec::new();

    if !changes_dir.exists() {
        return Ok(changes);
    }

    for entry in std::fs::read_dir(changes_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir()
            && is_change_directory(&path)
            && let Some(name) = get_change_name(&path)
        {
            changes.push(name);
        }
    }

    changes.sort();
    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_is_change_directory() {
        let dir = tempdir().unwrap();
        assert!(!is_change_directory(dir.path()));

        fs::write(dir.path().join("proposal.md"), "# Proposal").unwrap();
        assert!(is_change_directory(dir.path()));
    }

    #[test]
    fn test_get_change_name() {
        let path = Path::new("/changes/add-dark-mode");
        assert_eq!(get_change_name(path), Some("add-dark-mode".to_string()));
    }
}
