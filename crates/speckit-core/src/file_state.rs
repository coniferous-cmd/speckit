use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use crate::store::errors::{StoreError, StoreErrorOptions};

const LOCK_DEADLINE: Duration = Duration::from_secs(5);
const LOCK_POLL: Duration = Duration::from_millis(25);

/// Error kind for lock operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileLockErrorKind {
    CreateFailed,
    Timeout,
}

/// Information about a lock error.
#[derive(Debug, Clone)]
pub struct FileLockErrorInfo {
    pub lock_path: PathBuf,
    pub cause: Option<String>,
}

/// Data used to construct lock error messages.
#[derive(Debug, Clone)]
pub struct LockErrorData {
    /// Noun phrase for the create-failed message, e.g. "the registry lock file".
    pub create_subject: String,
    /// The full timeout message, e.g. "Store registry is busy."
    pub busy_message: String,
    pub code: String,
    pub target: String,
}

/// Creates a factory closure that produces [`StoreError`]s for lock failures.
pub fn make_lock_error_factory(
    data: LockErrorData,
) -> Box<dyn Fn(FileLockErrorKind, &FileLockErrorInfo) -> StoreError> {
    Box::new(move |kind, info| match kind {
        FileLockErrorKind::CreateFailed => {
            let cause_display = info.cause.as_deref().unwrap_or("unknown filesystem error");
            StoreError::new(
                format!(
                    "Cannot create {} {} ({}).",
                    data.create_subject,
                    info.lock_path.display(),
                    cause_display
                ),
                &data.code,
                StoreErrorOptions {
                    target: Some(data.target.clone()),
                    fix: Some(format!(
                        "Check permissions on {}.",
                        info.lock_path
                            .parent()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| ".".into())
                    )),
                },
            )
        }
        FileLockErrorKind::Timeout => StoreError::new(
            &data.busy_message,
            &data.code,
            StoreErrorOptions {
                target: Some(data.target.clone()),
                fix: Some(format!(
                    "Retry shortly; if this persists, delete the stale lock file {}.",
                    info.lock_path.display()
                )),
            },
        ),
    })
}

/// Returns `true` if the path points to a file (not a directory, not missing).
pub fn path_is_file(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
}

/// Returns `true` if the path points to a directory.
pub fn path_is_directory(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

/// Atomically writes content to a file by writing to a temp file first,
/// then renaming. The temp file sits in the same directory as the target
/// so the rename is guaranteed to be atomic on the same filesystem.
pub fn write_file_atomically(path: &Path, content: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "tmp".into());

    let pid = std::process::id();
    let unique = Uuid::new_v4().to_string();
    let temp_name = format!(".{}.{}.{}.{}.tmp", file_name, pid, unique, "tmp");
    let temp_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(&temp_name);

    let result = (|| -> Result<(), std::io::Error> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp_path, path)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    result
}

/// Handle to an acquired file lock. Dropping this handle does NOT release
/// the lock — call [`release_file_lock`] explicitly.
#[derive(Debug)]
pub struct FileLock {
    file: File,
    lock_path: PathBuf,
    ownership_token: String,
}

/// Acquires an exclusive file lock at `lock_path`.
///
/// The lock is created as an exclusive file (`O_CREAT | O_EXCL`). If the
/// file already exists the function polls until the deadline, then returns
/// a timeout error via the `error_for` factory.
pub fn acquire_file_lock(
    lock_path: &Path,
    error_for: &dyn Fn(FileLockErrorKind, &FileLockErrorInfo) -> StoreError,
) -> Result<FileLock, StoreError> {
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            error_for(
                FileLockErrorKind::CreateFailed,
                &FileLockErrorInfo {
                    lock_path: lock_path.to_path_buf(),
                    cause: Some(e.to_string()),
                },
            )
        })?;
    }

    let deadline = Instant::now() + LOCK_DEADLINE;
    let pid = std::process::id();
    let ownership_token = format!("{}:{}", pid, Uuid::new_v4());

    loop {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(lock_path)
        {
            Ok(mut file) => {
                file.write_all(ownership_token.as_bytes()).map_err(|e| {
                    let _ = file;
                    let _ = fs::remove_file(lock_path);
                    error_for(
                        FileLockErrorKind::CreateFailed,
                        &FileLockErrorInfo {
                            lock_path: lock_path.to_path_buf(),
                            cause: Some(e.to_string()),
                        },
                    )
                })?;

                // Best-effort fsync. Some FUSE / network filesystems do not
                // implement fsync on lock files; the token is still visible
                // to cooperating processes.
                let _ = file.sync_all();

                return Ok(FileLock {
                    file,
                    lock_path: lock_path.to_path_buf(),
                    ownership_token,
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if Instant::now() >= deadline {
                    return Err(error_for(
                        FileLockErrorKind::Timeout,
                        &FileLockErrorInfo {
                            lock_path: lock_path.to_path_buf(),
                            cause: None,
                        },
                    ));
                }
                std::thread::sleep(LOCK_POLL);
            }
            Err(e) => {
                return Err(error_for(
                    FileLockErrorKind::CreateFailed,
                    &FileLockErrorInfo {
                        lock_path: lock_path.to_path_buf(),
                        cause: Some(e.to_string()),
                    },
                ));
            }
        }
    }
}

/// Releases a file lock acquired with [`acquire_file_lock`].
///
/// The implementation only removes the lock file if the on-disk token
/// still matches this owner's token — a concurrent replacement is never
/// clobbered.
pub fn release_file_lock(lock: FileLock) {
    // Close the file handle first.
    drop(lock.file);

    // Read the current token; only delete if we still own it.
    if let Ok(current_token) = fs::read_to_string(&lock.lock_path)
        && current_token == lock.ownership_token {
            let _ = fs::remove_file(&lock.lock_path);
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn path_is_file_returns_true_for_file() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("test.txt");
        fs::write(&file, "hello").unwrap();
        assert!(path_is_file(&file));
        assert!(!path_is_directory(&file));
    }

    #[test]
    fn path_is_directory_returns_true_for_dir() {
        let tmp = TempDir::new().unwrap();
        assert!(path_is_directory(tmp.path()));
        assert!(!path_is_file(tmp.path()));
    }

    #[test]
    fn path_checks_return_false_for_missing() {
        let missing = PathBuf::from("/nonexistent/path/that/does/not/exist");
        assert!(!path_is_file(&missing));
        assert!(!path_is_directory(&missing));
    }

    #[test]
    fn write_file_atomically_creates_file() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("out.txt");
        write_file_atomically(&file, "test content").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "test content");
    }

    #[test]
    fn write_file_atomically_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("nested").join("dir").join("out.txt");
        write_file_atomically(&file, "deep").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "deep");
    }

    #[test]
    fn file_lock_acquire_and_release() {
        let tmp = TempDir::new().unwrap();
        let lock_path = tmp.path().join("test.lock");

        let factory = make_lock_error_factory(LockErrorData {
            create_subject: "test lock".into(),
            busy_message: "Test lock is busy.".into(),
            code: "test_lock".into(),
            target: "test".into(),
        });

        let lock = acquire_file_lock(&lock_path, &*factory).unwrap();
        assert!(lock_path.exists());
        release_file_lock(lock);
        assert!(!lock_path.exists());
    }

    #[test]
    fn file_lock_timeout_on_contention() {
        let tmp = TempDir::new().unwrap();
        let lock_path = tmp.path().join("contended.lock");

        // Create a fake lock file that will not be cleaned up.
        fs::write(&lock_path, "other-owner").unwrap();

        let factory = make_lock_error_factory(LockErrorData {
            create_subject: "test lock".into(),
            busy_message: "Test lock is busy.".into(),
            code: "test_lock_busy".into(),
            target: "test".into(),
        });

        let result = acquire_file_lock(&lock_path, &*factory);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "test_lock_busy");
    }
}
