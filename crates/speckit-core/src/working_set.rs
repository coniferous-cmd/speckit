//! Working-set assembly: the full set a root's declarations describe -- the
//! Speckit root and its referenced stores -- as an agent-consumable brief.
//!
//! A local convenience computed from declared relationships, never a planning
//! system; no clone/sync/launch machinery. Unresolvable members are reported,
//! not guessed.

use serde::{Deserialize, Serialize};

use crate::references::{self, ReferenceIndexEntry, StoreDiagnostic};

/// Role assigned to every working-set member that originates from a
/// referenced store.
const ROLE_REFERENCED_STORE: &str = "referenced_store";

/// Role assigned to the root entry of a working set.
const ROLE_OPENSPEC_ROOT: &str = "speckit_root";

/// A single member of a working set -- one referenced store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingSetMember {
    pub role: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch: Option<String>,
    #[serde(default)]
    pub status: Vec<StoreDiagnostic>,
}

/// Root descriptor carried inside a [`WorkingSet`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingSetRoot {
    pub path: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_id: Option<String>,
    pub role: String,
}

/// The assembled working set: root plus referenced-store members and
/// top-level status diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingSet {
    pub root: WorkingSetRoot,
    pub members: Vec<WorkingSetMember>,
    #[serde(default)]
    pub status: Vec<StoreDiagnostic>,
}

/// AVAILABLE = path present AND per-entry status empty.
pub fn is_available_member(member: &WorkingSetMember) -> bool {
    member.path.is_some() && member.status.is_empty()
}

/// Build a [`WorkingSet`] from a root descriptor and reference index entries.
///
/// * `root_path`   -- resolved filesystem path of the Speckit root.
/// * `root_source` -- how the root was resolved (e.g. "store", "local").
/// * `store_id`    -- optional store id when the root is store-backed.
/// * `reference_entries` -- entries from the referenced-store index.
/// * `top_level_status`  -- the composition's top-level diagnostics; only
///   entries whose code is `relationship_registry_unreadable` are kept.
pub fn assemble_working_set(
    root_path: &str,
    root_source: &str,
    store_id: Option<&str>,
    reference_entries: &[ReferenceIndexEntry],
    top_level_status: &[StoreDiagnostic],
) -> WorkingSet {
    let members: Vec<WorkingSetMember> = reference_entries
        .iter()
        .map(|entry| {
            let has_path = entry.root.is_some();
            let has_errors = !entry.status.is_empty();

            WorkingSetMember {
                role: ROLE_REFERENCED_STORE.to_string(),
                id: entry.store_id.clone(),
                path: entry.root.clone(),
                remote: None,
                fetch: if has_path && !has_errors {
                    Some(references::fetch_recipe(&entry.store_id))
                } else {
                    None
                },
                status: entry.status.clone(),
            }
        })
        .collect();

    let status: Vec<StoreDiagnostic> = top_level_status
        .iter()
        .filter(|d| d.code == "relationship_registry_unreadable")
        .cloned()
        .collect();

    WorkingSet {
        root: WorkingSetRoot {
            path: root_path.to_string(),
            source: root_source.to_string(),
            store_id: store_id.map(|s| s.to_string()),
            role: ROLE_OPENSPEC_ROOT.to_string(),
        },
        members,
        status,
    }
}

/// Pure builder for the `.code-workspace` editor view -- one consumer of
/// assembly, not the feature. Only available members are included.
pub fn build_code_workspace_json(working_set: &WorkingSet, root_name: &str) -> String {
    #[derive(Serialize)]
    struct FolderEntry {
        name: String,
        path: String,
    }

    #[derive(Serialize)]
    struct CodeWorkspace {
        folders: Vec<FolderEntry>,
    }

    let mut folders = vec![FolderEntry {
        name: root_name.to_string(),
        path: working_set.root.path.clone(),
    }];

    for member in &working_set.members {
        if !is_available_member(member) {
            continue;
        }
        if let Some(ref path) = member.path {
            folders.push(FolderEntry {
                name: format!("ref:{}", member.id),
                path: path.clone(),
            });
        }
    }

    let workspace = CodeWorkspace { folders };
    let mut json = serde_json::to_string_pretty(&workspace).unwrap();
    json.push('\n');
    json
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warn(code: &str) -> StoreDiagnostic {
        StoreDiagnostic {
            severity: "warning".to_string(),
            code: code.to_string(),
            message: "x".to_string(),
            target: Some("relationships".to_string()),
            fix: Some("y".to_string()),
        }
    }

    fn make_entry(
        store_id: &str,
        root: Option<&str>,
        status: Vec<StoreDiagnostic>,
    ) -> ReferenceIndexEntry {
        ReferenceIndexEntry {
            store_id: store_id.to_string(),
            root: root.map(|s| s.to_string()),
            specs: None,
            fetch: None,
            status,
        }
    }

    #[test]
    fn maps_referenced_stores_into_available_and_unavailable_members() {
        let entries = vec![
            make_entry("up", Some("/up"), vec![]),
            make_entry("ghost", None, vec![warn("reference_unresolved")]),
        ];
        let top_status = vec![warn("relationship_registry_unreadable")];

        let ws = assemble_working_set(
            "/team/store",
            "store",
            Some("team-context"),
            &entries,
            &top_status,
        );

        assert_eq!(ws.root.path, "/team/store");
        assert_eq!(ws.root.source, "store");
        assert_eq!(ws.root.store_id.as_deref(), Some("team-context"));
        assert_eq!(ws.root.role, "speckit_root");

        let ids: Vec<&str> = ws.members.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["up", "ghost"]);

        // Fetch recipe only on available references.
        assert_eq!(
            ws.members[0].fetch.as_deref(),
            Some("speckit show <spec-id> --type spec --store up")
        );
        assert!(ws.members[1].fetch.is_none());

        // Availability rule: path AND empty status.
        let available: Vec<&str> = ws
            .members
            .iter()
            .filter(|m| is_available_member(m))
            .map(|m| m.id.as_str())
            .collect();
        assert_eq!(available, vec!["up"]);

        // Registry degradation selected by code, never position.
        let codes: Vec<&str> = ws.status.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, vec!["relationship_registry_unreadable"]);
    }

    #[test]
    fn selects_registry_diagnostic_by_code_among_other_status_entries() {
        let top_status = vec![
            warn("root_pointer_ignored"),
            warn("relationship_registry_unreadable"),
        ];

        let ws = assemble_working_set("/r", "local", None, &[], &top_status);

        let codes: Vec<&str> = ws.status.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, vec!["relationship_registry_unreadable"]);
    }

    #[test]
    fn builds_code_workspace_view_from_available_members_only() {
        let entries = vec![
            make_entry("up", Some("/up"), vec![]),
            make_entry("ghost", None, vec![warn("reference_unresolved")]),
        ];

        let ws = assemble_working_set("/team/store", "store", Some("team-context"), &entries, &[]);
        let json = build_code_workspace_json(&ws, "team-context");

        assert!(json.ends_with('\n'));

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let folders = parsed["folders"].as_array().unwrap();
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0]["name"], "team-context");
        assert_eq!(folders[0]["path"], "/team/store");
        assert_eq!(folders[1]["name"], "ref:up");
        assert_eq!(folders[1]["path"], "/up");
    }

    #[test]
    fn available_member_requires_path_and_empty_status() {
        let with_path = WorkingSetMember {
            role: ROLE_REFERENCED_STORE.to_string(),
            id: "a".to_string(),
            path: Some("/a".to_string()),
            remote: None,
            fetch: None,
            status: vec![],
        };
        assert!(is_available_member(&with_path));

        let no_path = WorkingSetMember {
            role: ROLE_REFERENCED_STORE.to_string(),
            id: "b".to_string(),
            path: None,
            remote: None,
            fetch: None,
            status: vec![],
        };
        assert!(!is_available_member(&no_path));

        let with_status = WorkingSetMember {
            role: ROLE_REFERENCED_STORE.to_string(),
            id: "c".to_string(),
            path: Some("/c".to_string()),
            remote: None,
            fetch: None,
            status: vec![StoreDiagnostic {
                severity: "warning".to_string(),
                code: "test".to_string(),
                message: "m".to_string(),
                target: None,
                fix: None,
            }],
        };
        assert!(!is_available_member(&with_status));
    }
}
