use std::path::Path;
use std::process::Command;

use crate::store::errors::{StoreError, StoreErrorOptions};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Runs a git command in the given working directory. Returns `Ok(stdout)`
/// on success, `Err(exit_code)` on failure.
fn git_exec(cwd: &Path, args: &[&str]) -> Result<String, i32> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                -1
            } else {
                -2
            }
        })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(output.status.code().unwrap_or(1))
    }
}

/// Like `git_exec` but returns `None` on any failure (including git not found).
fn git_probe(store_root: &Path, args: &[&str]) -> Option<String> {
    git_exec(store_root, args).ok().map(|s| {
        let trimmed = s.trim().to_string();
        if trimmed.is_empty() {
            String::new()
        } else {
            trimmed
        }
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns `true` when `store_root` is (or is inside) a git repository.
///
/// Detection: a `.git` directory or file (submodule) at the root.
pub fn is_git_repository_at_root(store_root: &Path) -> bool {
    let dot_git = store_root.join(".git");
    dot_git.is_dir() || dot_git.is_file()
}

/// Initializes a git repository at `store_root`.
///
/// Returns `true` if a new repository was created, `false` if one already
/// existed.
pub fn init_git_repository(store_root: &Path) -> Result<bool, StoreError> {
    if is_git_repository_at_root(store_root) {
        return Ok(false);
    }

    git_exec(store_root, &["init"]).map_err(|_| {
        StoreError::new(
            "Failed to initialize Git repository.",
            "store_git_init_failed",
            StoreErrorOptions {
                target: Some("store.git".into()),
                fix: Some("Install Git or rerun setup with --no-init-git.".into()),
            },
        )
    })?;

    Ok(true)
}

/// Asserts that a usable Git commit identity is configured.
///
/// Uses `git var GIT_AUTHOR_IDENT` and `git var GIT_COMMITTER_IDENT` which
/// resolve identity exactly as `git commit` would (config, env vars,
/// auto-detection).
pub fn assert_git_commit_identity(probe_cwd: &Path) -> Result<(), StoreError> {
    for ident_var in &["GIT_COMMITTER_IDENT", "GIT_AUTHOR_IDENT"] {
        match git_exec(probe_cwd, &["var", ident_var]) {
            Ok(_) => continue,
            Err(-1) => {
                return Err(StoreError::new(
                    "Git is not available, so setup cannot create the initial store commit.",
                    "store_git_init_failed",
                    StoreErrorOptions {
                        target: Some("store.git".into()),
                        fix: Some("Install Git or rerun setup with --no-init-git.".into()),
                    },
                ));
            }
            Err(_) => {
                return Err(StoreError::new(
                    "No usable Git commit identity is configured, so setup cannot create the initial store commit.",
                    "store_git_identity_missing",
                    StoreErrorOptions {
                        target: Some("store.git".into()),
                        fix: Some(
                            "Run git config --global user.name \"Your Name\" and git config --global user.email \"you@example.com\", or rerun setup with --no-init-git.".into(),
                        ),
                    },
                ));
            }
        }
    }
    Ok(())
}

/// Index-preserving initial commit. Adds and commits only the given
/// pathspecs so that any files the user had already staged are left alone.
///
/// Returns `true` when a commit was created, `false` when `pathspecs` was
/// empty.
pub fn commit_store_files(
    store_root: &Path,
    id: &str,
    pathspecs: &[&str],
) -> Result<bool, StoreError> {
    if pathspecs.is_empty() {
        return Ok(false);
    }

    let add_result = git_exec(store_root, &[&["add", "--"], pathspecs].concat());

    if add_result.is_err() {
        return Err(StoreError::new(
            "Failed to stage store files for initial commit.",
            "store_git_commit_failed",
            StoreErrorOptions {
                target: Some("store.git".into()),
                fix: Some(
                    "Commit the created files manually, or rerun setup with --no-init-git.".into(),
                ),
            },
        ));
    }

    let commit_message = format!("Initialize Speckit store {id}");
    let commit_args: Vec<&str> = ["commit", "-m", &commit_message, "--"]
        .into_iter()
        .chain(pathspecs.iter().copied())
        .collect();

    match git_exec(store_root, &commit_args) {
        Ok(_) => Ok(true),
        Err(_) => {
            // Best-effort unstage so a failed commit does not leave
            // setup's files in the user's index after rollback.
            let rm_args: Vec<&str> = ["rm", "--cached", "-r", "-f", "-q", "--"]
                .into_iter()
                .chain(pathspecs.iter().copied())
                .collect();
            let _ = git_exec(store_root, &rm_args);

            Err(StoreError::new(
                "Failed to create the initial store commit.",
                "store_git_commit_failed",
                StoreErrorOptions {
                    target: Some("store.git".into()),
                    fix: Some(
                        "Commit the created files manually, or rerun setup with --no-init-git."
                            .into(),
                    ),
                },
            ))
        }
    }
}

/// Returns `true` if the repository has at least one commit, `false` if it
/// exists but is empty, `None` when git is unavailable or the repo is
/// corrupt.
pub fn git_has_commits(store_root: &Path) -> Option<bool> {
    match git_exec(store_root, &["rev-parse", "--verify", "--quiet", "HEAD"]) {
        Ok(_) => Some(true),
        Err(-1) => None,       // git not found
        Err(1) => Some(false), // repo exists but no commits
        Err(_) => None,        // corrupt / fake .git
    }
}

/// Returns `true` when the working tree has uncommitted changes.
pub fn git_has_uncommitted_changes(store_root: &Path) -> Option<bool> {
    let stdout = git_probe(store_root, &["status", "--porcelain"])?;
    Some(!stdout.is_empty())
}

/// Returns `true` when at least one remote is configured.
pub fn git_has_remote(store_root: &Path) -> Option<bool> {
    let stdout = git_probe(store_root, &["remote"])?;
    Some(!stdout.is_empty())
}

/// The configured origin URL, read from local config only. `None` when
/// there is no repository or no origin.
pub fn git_origin_url(store_root: &Path) -> Option<String> {
    let url = git_probe(store_root, &["remote", "get-url", "origin"])?;
    if url.is_empty() { None } else { Some(url) }
}

/// Ahead/behind counts of HEAD against its upstream tracking ref.
///
/// The comparison is against the local upstream ref, not the live remote.
/// `None` when there is no repository, no upstream, a detached HEAD, or
/// git is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GitTrackingDrift {
    pub ahead: u64,
    pub behind: u64,
}

pub fn git_tracking_drift(store_root: &Path) -> Option<GitTrackingDrift> {
    let stdout = git_probe(
        store_root,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    )?;
    let parts: Vec<&str> = stdout.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }
    let behind = parts[0].parse::<u64>().ok()?;
    let ahead = parts[1].parse::<u64>().ok()?;
    Some(GitTrackingDrift { ahead, behind })
}

/// Returns `true` when `relative_dir` contains tracked files.
pub fn git_directory_has_tracked_files(store_root: &Path, relative_dir: &str) -> Option<bool> {
    let stdout = git_probe(store_root, &["ls-files", "--", relative_dir])?;
    Some(!stdout.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn is_git_repository_at_root_detects_absence() {
        let tmp = TempDir::new().unwrap();
        assert!(!is_git_repository_at_root(tmp.path()));
    }

    #[test]
    fn is_git_repository_at_root_detects_presence() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        assert!(is_git_repository_at_root(tmp.path()));
    }

    #[test]
    fn init_and_detect_repository() {
        let tmp = TempDir::new().unwrap();
        if init_git_repository(tmp.path()).is_ok() {
            // git is available
            assert!(is_git_repository_at_root(tmp.path()));
            // Second call returns false (already initialized).
            assert!(!init_git_repository(tmp.path()).unwrap());
        }
    }
}
