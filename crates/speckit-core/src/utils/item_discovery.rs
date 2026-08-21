use anyhow::Result;
use std::path::Path;

/// Discover items (changes or specs) in a directory.
pub fn discover_items(base_dir: &Path, item_type: &str) -> Result<Vec<String>> {
    let mut items = Vec::new();

    if !base_dir.exists() {
        return Ok(items);
    }

    let target_dir = match item_type {
        "change" | "changes" => base_dir.join("changes"),
        "spec" | "specs" => base_dir.join("specs"),
        _ => base_dir.to_path_buf(),
    };

    if !target_dir.exists() {
        return Ok(items);
    }

    for entry in std::fs::read_dir(&target_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                items.push(name.to_string());
            }
        }
    }

    items.sort();
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_discover_items_empty() {
        let dir = tempdir().unwrap();
        let items = discover_items(dir.path(), "changes").unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_discover_items_with_changes() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("changes").join("change-1")).unwrap();
        fs::create_dir_all(dir.path().join("changes").join("change-2")).unwrap();

        let items = discover_items(dir.path(), "changes").unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.contains(&"change-1".to_string()));
        assert!(items.contains(&"change-2".to_string()));
    }
}
