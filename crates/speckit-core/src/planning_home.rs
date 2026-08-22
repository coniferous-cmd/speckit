//! Planning home resolution: locates the nearest Speckit root by walking
//! up the directory tree looking for an `speckit/` directory.

use std::path::{Path, PathBuf};

/// The kind of planning home discovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanningHomeKind {
    Repo,
}

/// A resolved planning home: the nearest ancestor directory containing
/// an `speckit/` subdirectory.
#[derive(Debug, Clone)]
pub struct PlanningHome {
    pub kind: PlanningHomeKind,
    pub root: PathBuf,
    pub changes_dir: PathBuf,
    pub default_schema: String,
}

const REPO_DEFAULT_SCHEMA: &str = "spec-driven";

/// Options for resolving a planning home.
#[derive(Debug, Clone, Default)]
pub struct ResolvePlanningHomeOptions {
    pub start_path: Option<PathBuf>,
    pub allow_implicit_repo_root: Option<bool>,
}

/// Returns true if `candidate_path` is an existing directory.
fn path_exists_as_directory(candidate_path: &Path) -> bool {
    candidate_path.is_dir()
}

/// Canonicalize `start_path`, falling back to the parent if it is not a directory.
fn get_search_start_directory(start_path: &Path) -> PathBuf {
    let resolved = dunce::canonicalize(start_path).unwrap_or_else(|_| start_path.to_path_buf());
    if resolved.is_dir() {
        resolved
    } else {
        resolved.parent().unwrap_or(&resolved).to_path_buf()
    }
}

/// Walk up from `start_path` returning the first directory for which
/// `predicate` returns true, or `None` at the filesystem root.
fn find_nearest_ancestor<F>(start_path: &Path, predicate: F) -> Option<PathBuf>
where
    F: Fn(&Path) -> bool,
{
    let mut current_dir = get_search_start_directory(start_path);

    loop {
        if predicate(&current_dir) {
            return Some(dunce::canonicalize(&current_dir).unwrap_or_else(|_| current_dir.clone()));
        }

        let parent_dir = match current_dir.parent() {
            Some(p) => p.to_path_buf(),
            None => return None,
        };
        if parent_dir == current_dir {
            return None;
        }
        current_dir = parent_dir;
    }
}

/// Find the nearest repo planning root by looking for an `speckit/` directory.
///
/// Returns the canonicalized path of the directory containing the `speckit/`
/// subdirectory, or `None` if no such ancestor exists.
pub fn find_repo_planning_root_sync(start_path: Option<&Path>) -> Option<PathBuf> {
    let resolved_start = start_path.unwrap_or_else(|| Path::new("."));
    find_nearest_ancestor(resolved_start, |dir_path| {
        path_exists_as_directory(&dir_path.join("speckit"))
    })
}

/// Build a `PlanningHome` from a repo root.
fn repo_planning_home(repo_root: PathBuf) -> PlanningHome {
    let changes_dir = repo_root.join("speckit").join("changes");
    PlanningHome {
        kind: PlanningHomeKind::Repo,
        root: repo_root,
        changes_dir,
        default_schema: REPO_DEFAULT_SCHEMA.to_string(),
    }
}

/// Resolve the current planning home from `start_path`.
///
/// If `allow_implicit_repo_root` is `Some(false)` and no root is found,
/// an error is returned. Otherwise the search start is treated as the root.
pub fn resolve_current_planning_home_sync(
    options: &ResolvePlanningHomeOptions,
) -> Result<PlanningHome, String> {
    let start_path = options
        .start_path
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let search_start = get_search_start_directory(&start_path);
    let repo_root = find_repo_planning_root_sync(Some(&search_start));

    match repo_root {
        Some(root) => Ok(repo_planning_home(root)),
        None => {
            if options.allow_implicit_repo_root == Some(false) {
                Err("No Speckit planning home found from the current directory.".to_string())
            } else {
                let canonical =
                    dunce::canonicalize(&search_start).unwrap_or_else(|_| search_start.clone());
                Ok(repo_planning_home(canonical))
            }
        }
    }
}

/// Get the change directory within a planning home.
pub fn get_change_dir(planning_home: &PlanningHome, change_name: &str) -> PathBuf {
    planning_home.changes_dir.join(change_name)
}

/// Format the change location relative to the planning home root.
pub fn format_change_location(planning_home: &PlanningHome, change_name: &str) -> String {
    let change_dir = get_change_dir(planning_home, change_name);
    change_dir
        .strip_prefix(&planning_home.root)
        .unwrap_or(&change_dir)
        .to_string_lossy()
        // Locations are user-facing/logical paths, so keep their separator
        // stable across platforms instead of leaking Windows `\\` separators.
        .replace('\\', "/")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_repo_planning_root_sync_none() {
        let tmp = TempDir::new().unwrap();
        assert!(find_repo_planning_root_sync(Some(tmp.path())).is_none());
    }

    #[test]
    fn test_find_repo_planning_root_sync_found() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("speckit")).unwrap();
        let result = find_repo_planning_root_sync(Some(tmp.path()));
        assert!(result.is_some());
    }

    #[test]
    fn test_format_change_location() {
        let home = PlanningHome {
            kind: PlanningHomeKind::Repo,
            root: PathBuf::from("/project"),
            changes_dir: PathBuf::from("/project/speckit/changes"),
            default_schema: REPO_DEFAULT_SCHEMA.to_string(),
        };
        assert_eq!(
            format_change_location(&home, "my-change"),
            "speckit/changes/my-change"
        );
    }
}
