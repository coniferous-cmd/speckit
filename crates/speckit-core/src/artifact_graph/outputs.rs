use globset::{Glob, GlobMatcher};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

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
    let output_path = change_dir.join(generates);

    if !is_glob_pattern(generates) {
        // Simple path: check if it's a file
        return match fs::metadata(&output_path) {
            Ok(meta) if meta.is_file() => {
                let canon = dunce::canonicalize(&output_path).unwrap_or(output_path);
                vec![canon]
            }
            _ => vec![],
        };
    }

    // Glob pattern: use globset for matching
    let normalized_pattern = generates.replace('\\', "/");
    let glob = match Glob::new(&normalized_pattern) {
        Ok(g) => g.compile_matcher(),
        Err(_) => return vec![],
    };

    // Walk the change directory to find matches
    let mut matches = BTreeSet::new();
    walkdir_matches(change_dir, change_dir, &glob, &mut matches);

    matches.into_iter().collect()
}

/// Recursively walks a directory collecting files that match the glob matcher.
fn walkdir_matches(
    root: &Path,
    dir: &Path,
    glob: &GlobMatcher,
    results: &mut BTreeSet<PathBuf>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let relative = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Convert to posix-style for matching
        let relative_str = relative.to_string_lossy().replace('\\', "/");

        if path.is_dir() {
            walkdir_matches(root, &path, glob, results);
        } else if path.is_file() && glob.is_match(&relative_str) {
            let canon = dunce::canonicalize(&path).unwrap_or(path);
            results.insert(canon);
        }
    }
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
