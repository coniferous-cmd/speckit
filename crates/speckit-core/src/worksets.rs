//! Personal worksets (slice 7.1): purely local, manually composed,
//! named working views. The whole feature's state lives under
//! `<globalDataDir>/worksets/` -- the saved-views file plus the generated
//! `.code-workspace` files -- so deleting that one directory removes
//! every trace. Nothing here is committed, shared, or derived from
//! declarations, and nothing is ever written into a member folder.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::global_config::get_global_data_dir;
use crate::id::{KEBAB_ID_DESCRIPTION, KEBAB_ID_FIX, folder_style_name_problem, is_kebab_id};
use crate::store::errors::{StoreError, StoreErrorOptions};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Subdirectory name under the global data dir.
pub const WORKSETS_DIR_NAME: &str = "worksets";

/// Filename for the YAML persistence file.
pub const WORKSETS_FILE_NAME: &str = "worksets.yaml";

/// File extension for generated VS Code workspace files.
const CODE_WORKSPACE_EXTENSION: &str = ".code-workspace";

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Options accepted by every function that resolves workset paths.
/// When `global_data_dir` is `None` the real global data dir is used.
#[derive(Debug, Clone, Default)]
pub struct WorksetPathOptions {
    pub global_data_dir: Option<PathBuf>,
}

/// A single member inside a workset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorksetMember {
    /// Display label; becomes the `.code-workspace` folder name.
    pub name: String,
    /// Absolute path to the member directory.
    pub path: String,
}

/// A fully-resolved workset (name carried alongside the entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workset {
    pub name: String,
    /// Preferred opener id; validated only at open time.
    pub tool: Option<String>,
    /// Ordered; the first member is the primary (session cwd).
    pub members: Vec<WorksetMember>,
}

/// The serialisable entry shape inside `worksets.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorksetEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    pub members: Vec<WorksetMember>,
}

/// Top-level persisted state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorksetsState {
    /// Schema version; always 1.
    pub version: u32,
    /// Map from workset kebab-name to entry.
    pub worksets: HashMap<String, WorksetEntry>,
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn resolve_global_data_dir(options: &WorksetPathOptions) -> PathBuf {
    options
        .global_data_dir
        .clone()
        .unwrap_or_else(get_global_data_dir)
}

/// Returns the directory that holds all workset artifacts.
pub fn get_worksets_dir(options: &WorksetPathOptions) -> PathBuf {
    resolve_global_data_dir(options).join(WORKSETS_DIR_NAME)
}

/// Returns the full path to `worksets.yaml`.
pub fn get_worksets_file_path(options: &WorksetPathOptions) -> PathBuf {
    get_worksets_dir(options).join(WORKSETS_FILE_NAME)
}

/// Returns the path to the generated `.code-workspace` file for a workset.
pub fn get_workset_code_workspace_path(name: &str, options: &WorksetPathOptions) -> PathBuf {
    get_worksets_dir(options).join(format!("{name}{CODE_WORKSPACE_EXTENSION}"))
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validates that `name` is a legal kebab-case workset id.
///
/// Returns the name on success; returns a [`StoreError`] on failure.
pub fn validate_workset_name(name: &str) -> Result<String, StoreError> {
    if !is_kebab_id(name) {
        return Err(StoreError::new(
            format!("Workset name '{}' {}.", name, KEBAB_ID_DESCRIPTION),
            "invalid_workset_name",
            StoreErrorOptions {
                target: Some("workset.name".into()),
                fix: Some(KEBAB_ID_FIX.into()),
            },
        ));
    }
    Ok(name.to_owned())
}

/// Returns a problem description for a member label, or `None` when valid.
pub fn member_label_problem(label: &str) -> Option<String> {
    folder_style_name_problem(label, "member name")
}

/// Validates a member list.
///
/// Returns `Ok(())` when valid; returns a problem description string on
/// failure.
pub fn member_list_problem(members: &[WorksetMember]) -> Option<String> {
    if members.is_empty() {
        return Some("members must not be empty".into());
    }

    let mut seen = HashSet::new();
    for member in members {
        if let Some(problem) = member_label_problem(&member.name) {
            return Some(problem);
        }

        if !seen.insert(&member.name) {
            return Some(format!(
                "duplicate member name '{}' (use the name=path form to label members distinctly)",
                member.name
            ));
        }

        if !Path::new(&member.path).is_absolute() {
            return Some(format!("member path '{}' must be absolute", member.path));
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Parse / serialize
// ---------------------------------------------------------------------------

fn invalid_worksets_file_error(message: &str, options: &WorksetPathOptions) -> StoreError {
    StoreError::new(
        format!("Invalid worksets file: {message}"),
        "invalid_workset_file",
        StoreErrorOptions {
            target: Some("workset.file".into()),
            fix: Some(format!(
                "Repair or remove {}.",
                get_worksets_file_path(options).display()
            )),
        },
    )
}

/// Parses a YAML string into a [`WorksetsState`], running structural and
/// semantic validation.
pub fn parse_worksets_state(
    content: &str,
    options: &WorksetPathOptions,
) -> Result<WorksetsState, StoreError> {
    let raw: serde_yaml::Value = serde_yaml::from_str(content)
        .map_err(|e| invalid_worksets_file_error(&e.to_string(), options))?;

    // Enforce the `version: 1` literal.
    let version = raw
        .get("version")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| invalid_worksets_file_error("missing or non-integer 'version'", options))?;
    if version != 1 {
        return Err(invalid_worksets_file_error(
            &format!("expected version 1, got {version}"),
            options,
        ));
    }

    let state: WorksetsState = serde_yaml::from_value(raw)
        .map_err(|e| invalid_worksets_file_error(&e.to_string(), options))?;

    // Validate every entry: name must be kebab, members must be well-formed.
    for (name, entry) in &state.worksets {
        if !is_kebab_id(name) {
            return Err(invalid_worksets_file_error(
                &format!("workset name '{}' {}", name, KEBAB_ID_DESCRIPTION),
                options,
            ));
        }
        if let Some(problem) = member_list_problem(&entry.members) {
            return Err(invalid_worksets_file_error(
                &format!("workset '{name}': {problem}"),
                options,
            ));
        }
    }

    Ok(state)
}

/// Serialises a [`WorksetsState`] to YAML with deterministic key ordering.
pub fn serialize_worksets_state(
    state: &WorksetsState,
    options: &WorksetPathOptions,
) -> Result<String, StoreError> {
    // Validate the state before writing: names must be kebab, members valid.
    for (name, entry) in &state.worksets {
        if !is_kebab_id(name) {
            return Err(invalid_worksets_file_error(
                &format!("workset name '{}' {}", name, KEBAB_ID_DESCRIPTION),
                options,
            ));
        }
        if let Some(problem) = member_list_problem(&entry.members) {
            return Err(invalid_worksets_file_error(
                &format!("workset '{name}': {problem}"),
                options,
            ));
        }
    }

    // Build an ordered representation so YAML keys are sorted.
    let sorted: BTreeMap<&String, &WorksetEntry> = state.worksets.iter().collect();

    let output = serde_yaml::to_string(&WorksetsStateOrdered {
        version: 1,
        worksets: sorted,
    })
    .map_err(|e| invalid_worksets_file_error(&e.to_string(), options))?;

    Ok(output)
}

/// Intermediate struct used only for sorted serialisation.
#[derive(Serialize)]
struct WorksetsStateOrdered<'a> {
    version: u32,
    worksets: BTreeMap<&'a String, &'a WorksetEntry>,
}

// ---------------------------------------------------------------------------
// File I/O
// ---------------------------------------------------------------------------

/// Reads worksets state from disk. Returns the empty state when the file
/// does not exist; returns an error for a corrupt file.
pub fn read_worksets_state(options: &WorksetPathOptions) -> Result<WorksetsState, StoreError> {
    let file_path = get_worksets_file_path(options);

    if !file_path.exists() {
        return Ok(WorksetsState {
            version: 1,
            worksets: HashMap::new(),
        });
    }

    let content = fs::read_to_string(&file_path)
        .map_err(|e| invalid_worksets_file_error(&e.to_string(), options))?;

    parse_worksets_state(&content, options)
}

/// Writes worksets state to disk atomically.
pub fn write_worksets_state(
    state: &WorksetsState,
    options: &WorksetPathOptions,
) -> Result<(), StoreError> {
    let content = serialize_worksets_state(state, options)?;
    let file_path = get_worksets_file_path(options);

    crate::file_state::write_file_atomically(&file_path, &content)
        .map_err(|e| invalid_worksets_file_error(&e.to_string(), options))
}

/// Acquires the worksets lock, reads state, passes it to `updater`, writes
/// the result back, and releases the lock.
pub fn update_worksets_state<F>(
    updater: F,
    options: &WorksetPathOptions,
) -> Result<WorksetsState, StoreError>
where
    F: FnOnce(&WorksetsState) -> Result<WorksetsState, StoreError>,
{
    with_worksets_lock(
        |state| {
            let next = updater(state)?;
            write_worksets_state(&next, options)?;
            Ok(next)
        },
        options,
    )
}

/// Lock-scoped read without a write-back. The lock is released before
/// control returns.
pub fn with_worksets_lock<F, T>(f: F, options: &WorksetPathOptions) -> Result<T, StoreError>
where
    F: FnOnce(&WorksetsState) -> Result<T, StoreError>,
{
    use crate::file_state::{acquire_file_lock, release_file_lock};

    let lock_path = {
        let mut p = get_worksets_file_path(options).into_os_string();
        p.push(".lock");
        PathBuf::from(p)
    };

    let lock_error_factory =
        crate::file_state::make_lock_error_factory(crate::file_state::LockErrorData {
            create_subject: "the worksets lock file".into(),
            busy_message: "The worksets file is busy.".into(),
            code: "workset_file_busy".into(),
            target: "workset.file".into(),
        });

    let lock = acquire_file_lock(&lock_path, &*lock_error_factory)?;

    let state = read_worksets_state(options)?;
    let result = f(&state)?;

    release_file_lock(lock);
    Ok(result)
}

// ---------------------------------------------------------------------------
// State mutation helpers (pure, no I/O)
// ---------------------------------------------------------------------------

/// Constructs a [`StoreError`] for a missing workset.
pub fn workset_not_found_error(name: &str, state: &WorksetsState) -> StoreError {
    let mut saved_names: Vec<&String> = state.worksets.keys().collect();
    saved_names.sort();

    let fix = if saved_names.is_empty() {
        format!("Create it first: speckit workset create {name}")
    } else {
        format!(
            "Saved worksets: {}. See them with: speckit workset list",
            saved_names
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    StoreError::new(
        format!("Workset '{name}' is not saved on this machine."),
        "workset_not_found",
        StoreErrorOptions {
            target: Some("workset.name".into()),
            fix: Some(fix),
        },
    )
}

/// Adds a workset to the state. Returns an error if the name already exists.
pub fn with_workset(state: &WorksetsState, workset: &Workset) -> Result<WorksetsState, StoreError> {
    if state.worksets.contains_key(&workset.name) {
        return Err(StoreError::new(
            format!("Workset '{}' already exists.", workset.name),
            "workset_exists",
            StoreErrorOptions {
                target: Some("workset.name".into()),
                fix: Some(format!(
                    "Choose another name, or remove it first: speckit workset remove {}",
                    workset.name
                )),
            },
        ));
    }

    let mut next_worksets = state.worksets.clone();
    next_worksets.insert(
        workset.name.clone(),
        WorksetEntry {
            tool: workset.tool.clone(),
            members: workset.members.clone(),
        },
    );

    Ok(WorksetsState {
        version: 1,
        worksets: next_worksets,
    })
}

/// Removes a workset from the state. Returns an error if the name is absent.
pub fn without_workset(state: &WorksetsState, name: &str) -> Result<WorksetsState, StoreError> {
    if !state.worksets.contains_key(name) {
        return Err(workset_not_found_error(name, state));
    }

    let mut remaining = state.worksets.clone();
    remaining.remove(name);

    Ok(WorksetsState {
        version: 1,
        worksets: remaining,
    })
}

/// Removes a saved workset and its derived `.code-workspace` under one
/// lock. The derived-file cleanup runs AFTER the durable write (a failed
/// write must not have already destroyed the artifact); a never-opened
/// workset has no file -- ENOENT is fine.
pub fn remove_workset(name: &str, options: &WorksetPathOptions) -> Result<(), StoreError> {
    update_worksets_state(
        |state| {
            let next = without_workset(state, name)?;
            Ok(next)
        },
        options,
    )?;

    // Best-effort removal of the derived .code-workspace file.
    let _ = fs::remove_file(get_workset_code_workspace_path(name, options));

    Ok(())
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

fn to_workset(name: &str, entry: &WorksetEntry) -> Workset {
    Workset {
        name: name.to_owned(),
        tool: entry.tool.clone(),
        members: entry.members.clone(),
    }
}

/// Lists all worksets in sorted order.
pub fn list_worksets(state: &WorksetsState) -> Vec<Workset> {
    let mut entries: Vec<Workset> = state
        .worksets
        .iter()
        .map(|(name, entry)| to_workset(name, entry))
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

/// Returns a single workset by name, or `None` if absent.
pub fn get_workset(state: &WorksetsState, name: &str) -> Option<Workset> {
    state
        .worksets
        .get(name)
        .map(|entry| to_workset(name, entry))
}

// ---------------------------------------------------------------------------
// Workspace file builder
// ---------------------------------------------------------------------------

/// The generated `.code-workspace` content: members in saved order with
/// their saved names, absolute paths, two-space JSON, trailing newline
/// (the working-set builder's conventions).
pub fn build_workset_code_workspace_json(members: &[WorksetMember]) -> String {
    #[derive(Serialize)]
    struct WorkspaceFolder<'a> {
        name: &'a str,
        path: &'a str,
    }

    #[derive(Serialize)]
    struct Workspace<'a> {
        folders: Vec<WorkspaceFolder<'a>>,
    }

    let workspace = Workspace {
        folders: members
            .iter()
            .map(|m| WorkspaceFolder {
                name: &m.name,
                path: &m.path,
            })
            .collect(),
    };

    serde_json::to_string_pretty(&workspace).expect("workspace JSON serialization is infallible")
        + "\n"
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn temp_options(tmp: &TempDir) -> WorksetPathOptions {
        WorksetPathOptions {
            global_data_dir: Some(tmp.path().join("data")),
        }
    }

    fn member_a(tmp: &TempDir) -> WorksetMember {
        WorksetMember {
            name: "team-context".into(),
            path: tmp.path().join("team-context").display().to_string(),
        }
    }

    fn member_b(tmp: &TempDir) -> WorksetMember {
        WorksetMember {
            name: "web-app".into(),
            path: tmp.path().join("web-app").display().to_string(),
        }
    }

    // -- path helpers -------------------------------------------------------

    #[test]
    fn paths_locate_everything_under_global_data_dir() {
        let tmp = TempDir::new().unwrap();
        let opts = temp_options(&tmp);
        let data_dir = tmp.path().join("data");

        assert_eq!(get_worksets_dir(&opts), data_dir.join(WORKSETS_DIR_NAME));
        assert_eq!(
            get_worksets_file_path(&opts),
            data_dir.join(WORKSETS_DIR_NAME).join(WORKSETS_FILE_NAME)
        );
        assert_eq!(
            get_workset_code_workspace_path("platform", &opts),
            data_dir
                .join(WORKSETS_DIR_NAME)
                .join("platform.code-workspace")
        );
    }

    // -- name and member validation -----------------------------------------

    #[test]
    fn validate_workset_name_accepts_kebab() {
        assert_eq!(validate_workset_name("platform-2").unwrap(), "platform-2");
    }

    #[test]
    fn validate_workset_name_rejects_non_kebab() {
        let err = validate_workset_name("My Stuff").unwrap_err();
        assert_eq!(err.code(), "invalid_workset_name");
        assert!(err.to_string().contains("must be kebab-case"));
    }

    #[test]
    fn member_label_accepts_plain_labels() {
        assert_eq!(member_label_problem("web-app"), None);
        assert_eq!(member_label_problem("Web App"), None);
    }

    #[test]
    fn member_label_rejects_empty_dot_separator() {
        assert!(member_label_problem("").is_some());
        assert!(member_label_problem(".").is_some());
        assert!(member_label_problem("a/b").is_some());
        assert!(member_label_problem("a\\b").is_some());
    }

    #[test]
    fn member_list_rejects_empty() {
        assert!(member_list_problem(&[]).is_some());
    }

    #[test]
    fn member_list_rejects_duplicates() {
        let tmp = TempDir::new().unwrap();
        let a = member_a(&tmp);
        let mut b = member_b(&tmp);
        b.name = a.name.clone();
        assert!(
            member_list_problem(&[a, b])
                .unwrap()
                .contains("duplicate member name")
        );
    }

    #[test]
    fn member_list_rejects_relative_paths() {
        assert!(
            member_list_problem(&[WorksetMember {
                name: "web".into(),
                path: "relative/web".into(),
            }])
            .unwrap()
            .contains("must be absolute")
        );
    }

    #[test]
    fn member_list_accepts_valid_members() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(member_list_problem(&[member_a(&tmp), member_b(&tmp)]), None);
    }

    // -- parse and serialize ------------------------------------------------

    #[test]
    fn parse_serialize_round_trip_with_sorted_names() {
        let tmp = TempDir::new().unwrap();
        let opts = temp_options(&tmp);

        let mut worksets = HashMap::new();
        worksets.insert(
            "zeta".into(),
            WorksetEntry {
                tool: None,
                members: vec![member_a(&tmp)],
            },
        );
        worksets.insert(
            "alpha".into(),
            WorksetEntry {
                tool: Some("claude".into()),
                members: vec![member_a(&tmp), member_b(&tmp)],
            },
        );

        let state = WorksetsState {
            version: 1,
            worksets,
        };

        let serialized = serialize_worksets_state(&state, &opts).unwrap();
        let parsed = parse_worksets_state(&serialized, &opts).unwrap();

        let keys: Vec<&str> = parsed.worksets.keys().map(|s| s.as_str()).collect();
        assert_eq!(keys, vec!["alpha", "zeta"]);
        assert_eq!(parsed.worksets["alpha"].tool.as_deref(), Some("claude"));
        assert!(parsed.worksets["zeta"].tool.is_none());
        assert!(!serialized.contains("tool: null"));
    }

    #[test]
    fn parse_rejects_bad_yaml() {
        let tmp = TempDir::new().unwrap();
        let opts = temp_options(&tmp);
        assert!(parse_worksets_state("{not yaml", &opts).is_err());
    }

    #[test]
    fn parse_rejects_wrong_version() {
        let tmp = TempDir::new().unwrap();
        let opts = temp_options(&tmp);
        assert!(parse_worksets_state("version: 2\nworksets: {}\n", &opts).is_err());
    }

    #[test]
    fn parse_rejects_non_kebab_name_in_file() {
        let tmp = TempDir::new().unwrap();
        let opts = temp_options(&tmp);
        let path = tmp.path().join("member");
        let content = format!(
            "version: 1\nworksets:\n  Bad Name:\n    members:\n      - name: a\n        path: {}\n",
            path.display()
        );
        let err = parse_worksets_state(&content, &opts).unwrap_err();
        assert_eq!(err.code(), "invalid_workset_file");
        assert!(err.to_string().contains("must be kebab-case"));
    }

    #[test]
    fn parse_rejects_empty_members_in_file() {
        let tmp = TempDir::new().unwrap();
        let opts = temp_options(&tmp);
        let content = "version: 1\nworksets:\n  empty:\n    members: []\n";
        let err = parse_worksets_state(content, &opts).unwrap_err();
        assert!(err.to_string().contains("members must not be empty"));
    }

    #[test]
    fn parse_rejects_relative_path_in_file() {
        let tmp = TempDir::new().unwrap();
        let opts = temp_options(&tmp);
        let content = "version: 1\nworksets:\n  rel:\n    members:\n      - name: a\n        path: relative/path\n";
        let err = parse_worksets_state(content, &opts).unwrap_err();
        assert!(err.to_string().contains("must be absolute"));
    }

    #[test]
    fn parse_rejects_duplicate_member_names_in_file() {
        let tmp = TempDir::new().unwrap();
        let opts = temp_options(&tmp);
        let path = tmp.path().join("x");
        let content = format!(
            "version: 1\nworksets:\n  dup:\n    members:\n      - name: a\n        path: {}\n      - name: a\n        path: {}\n",
            path.display(),
            path.display()
        );
        let err = parse_worksets_state(&content, &opts).unwrap_err();
        assert!(err.to_string().contains("duplicate member name"));
    }

    #[test]
    fn parse_accepts_unknown_tool_string() {
        let tmp = TempDir::new().unwrap();
        let opts = temp_options(&tmp);
        let path = tmp.path().join("x");
        let content = format!(
            "version: 1\nworksets:\n  alpha:\n    tool: deleted-tool\n    members:\n      - name: a\n        path: {}\n",
            path.display()
        );
        let parsed = parse_worksets_state(&content, &opts).unwrap();
        assert_eq!(
            parsed.worksets["alpha"].tool.as_deref(),
            Some("deleted-tool")
        );
    }

    // -- state mutations ----------------------------------------------------

    #[test]
    fn add_list_get_remove_worksets() {
        let tmp = TempDir::new().unwrap();
        let empty = WorksetsState {
            version: 1,
            worksets: HashMap::new(),
        };
        let workset = Workset {
            name: "platform".into(),
            tool: Some("claude".into()),
            members: vec![member_a(&tmp), member_b(&tmp)],
        };

        let with_one = with_workset(&empty, &workset).unwrap();
        assert_eq!(
            list_worksets(&with_one)
                .iter()
                .map(|w| w.name.as_str())
                .collect::<Vec<_>>(),
            vec!["platform"]
        );
        assert_eq!(
            get_workset(&with_one, "platform").unwrap().tool.as_deref(),
            Some("claude")
        );
        assert!(get_workset(&with_one, "absent").is_none());

        let removed = without_workset(&with_one, "platform").unwrap();
        assert!(list_worksets(&removed).is_empty());
    }

    #[test]
    fn with_workset_rejects_duplicate_name() {
        let tmp = TempDir::new().unwrap();
        let empty = WorksetsState {
            version: 1,
            worksets: HashMap::new(),
        };
        let ws = Workset {
            name: "platform".into(),
            tool: None,
            members: vec![member_a(&tmp)],
        };

        let state = with_workset(&empty, &ws).unwrap();
        let err = with_workset(&state, &ws).unwrap_err();
        assert_eq!(err.code(), "workset_exists");
        assert!(
            err.fix()
                .unwrap()
                .contains("Choose another name, or remove it first")
        );
    }

    #[test]
    fn without_workset_reports_unknown_name_with_saved_list() {
        let tmp = TempDir::new().unwrap();
        let empty = WorksetsState {
            version: 1,
            worksets: HashMap::new(),
        };
        let ws = Workset {
            name: "platform".into(),
            tool: None,
            members: vec![member_a(&tmp)],
        };
        let state = with_workset(&empty, &ws).unwrap();

        let err = without_workset(&state, "absent").unwrap_err();
        assert_eq!(err.code(), "workset_not_found");
        assert!(err.fix().unwrap().contains("Saved worksets: platform"));

        let err2 = without_workset(&empty, "absent").unwrap_err();
        assert!(
            err2.fix()
                .unwrap()
                .contains("Create it first: speckit workset create absent")
        );
    }

    // -- file I/O -----------------------------------------------------------

    #[test]
    fn read_returns_empty_state_when_no_file() {
        let tmp = TempDir::new().unwrap();
        let opts = temp_options(&tmp);
        let state = read_worksets_state(&opts).unwrap();
        assert_eq!(state.version, 1);
        assert!(state.worksets.is_empty());
    }

    #[test]
    fn write_and_read_back() {
        let tmp = TempDir::new().unwrap();
        let opts = temp_options(&tmp);

        let empty = WorksetsState {
            version: 1,
            worksets: HashMap::new(),
        };
        let ws = Workset {
            name: "platform".into(),
            tool: Some("code".into()),
            members: vec![member_a(&tmp)],
        };
        let state = with_workset(&empty, &ws).unwrap();

        write_worksets_state(&state, &opts).unwrap();
        let read_back = read_worksets_state(&opts).unwrap();
        assert_eq!(
            get_workset(&read_back, "platform").unwrap().members,
            vec![member_a(&tmp)]
        );
    }

    #[test]
    fn update_worksets_state_round_trip() {
        let tmp = TempDir::new().unwrap();
        let opts = temp_options(&tmp);

        update_worksets_state(
            |state| {
                let ws = Workset {
                    name: "platform".into(),
                    tool: Some("code".into()),
                    members: vec![member_a(&tmp)],
                };
                with_workset(state, &ws)
            },
            &opts,
        )
        .unwrap();

        let state = read_worksets_state(&opts).unwrap();
        assert_eq!(
            get_workset(&state, "platform").unwrap().members,
            vec![member_a(&tmp)]
        );
    }

    // -- code-workspace builder ---------------------------------------------

    #[test]
    fn build_workspace_json_emits_folders_in_order() {
        let tmp = TempDir::new().unwrap();
        let members = vec![member_a(&tmp), member_b(&tmp)];
        let json = build_workset_code_workspace_json(&members);

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let folders = parsed["folders"].as_array().unwrap();
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0]["name"].as_str().unwrap(), "team-context");
        assert_eq!(folders[1]["name"].as_str().unwrap(), "web-app");
        assert!(json.ends_with('\n'));
    }
}
