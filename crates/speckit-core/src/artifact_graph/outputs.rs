use globset::GlobBuilder;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

/// Checks if a path contains glob pattern characters.
pub fn is_glob_pattern(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains('[')
}

/// Resolves an artifact's output path pattern under the change directory.
///
/// Returns the absolute path (or glob pattern) after joining with `change_dir`.
/// The path is canonicalized if it exists; otherwise the raw join is returned.
pub fn resolve_artifact_output_path(change_dir: &Path, generates: &str) -> PathBuf {
    let output_path = change_dir.join(generates);
    // Canonicalize if the path exists, otherwise return the joined path as-is.
    dunce::canonicalize(&output_path).unwrap_or(output_path)
}

/// Resolves an artifact's output path(s) to concrete files that currently exist.
///
/// Returns absolute file paths. Glob matches are sorted for deterministic output.
pub fn resolve_artifact_outputs(change_dir: &Path, generates: &str) -> Vec<PathBuf> {
    let normalized_pattern = generates.replace('\\', "/");
    let pattern = Path::new(&normalized_pattern);
    if pattern
        .components()
        .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Vec::new();
    }
    let Ok(root) = dunce::canonicalize(change_dir) else {
        return Vec::new();
    };

    if !is_glob_pattern(generates) {
        return dunce::canonicalize(root.join(pattern))
            .ok()
            .filter(|path| path.starts_with(&root) && path.is_file())
            .into_iter()
            .collect();
    }

    let normalized_pattern = normalized_pattern.trim_start_matches("./");
    let glob = match GlobBuilder::new(normalized_pattern)
        .literal_separator(true)
        .build()
    {
        Ok(g) => g.compile_matcher(),
        Err(_) => return vec![],
    };

    WalkDir::new(&root)
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| {
            !entry.path_is_symlink()
                || dunce::canonicalize(entry.path()).is_ok_and(|path| path.starts_with(&root))
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .strip_prefix(&root)
                .is_ok_and(|path| glob.is_match(path))
        })
        .filter_map(|entry| dunce::canonicalize(entry.path()).ok())
        .filter(|path| path.starts_with(&root))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Checks if an artifact has at least one resolved output file.
pub fn artifact_output_exists(change_dir: &Path, generates: &str) -> bool {
    !resolve_artifact_outputs(change_dir, generates).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn is_glob_pattern_detects_wildcards() {
        assert!(is_glob_pattern("specs/**/*.md"));
        assert!(is_glob_pattern("file?.txt"));
        assert!(is_glob_pattern("file[0-9].txt"));
        assert!(!is_glob_pattern("simple-file.md"));
        assert!(!is_glob_pattern("path/to/file.txt"));
    }

    #[test]
    fn resolve_artifact_output_path_joins() {
        let change_dir = Path::new("/tmp/change");
        let result = resolve_artifact_output_path(change_dir, "proposal.md");
        assert_eq!(result, PathBuf::from("/tmp/change/proposal.md"));
    }

    #[test]
    fn artifact_output_exists_false_for_missing() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!artifact_output_exists(tmp.path(), "nonexistent.md"));
    }

    #[test]
    fn artifact_output_exists_true_for_present() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("proposal.md"), "content").unwrap();
        assert!(artifact_output_exists(tmp.path(), "proposal.md"));
    }

    #[test]
    fn resolve_artifact_outputs_glob_finds_files() {
        let tmp = tempfile::tempdir().unwrap();
        let specs_dir = tmp.path().join("specs").join("auth");
        fs::create_dir_all(&specs_dir).unwrap();
        fs::write(specs_dir.join("spec.md"), "content").unwrap();

        let results = resolve_artifact_outputs(tmp.path(), "specs/**/*.md");
        assert_eq!(results.len(), 1);
        assert!(results[0].to_string_lossy().contains("spec.md"));
    }

    #[test]
    fn resolve_artifact_outputs_empty_for_no_matches() {
        let tmp = tempfile::tempdir().unwrap();
        let results = resolve_artifact_outputs(tmp.path(), "*.md");
        assert!(results.is_empty());
    }
}
