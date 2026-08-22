//! Relationship health composition.
//!
//! One read-only answer to "are the roots this work relates to available
//! on this machine?" -- pure composition over inputs the doctor command gathers.

use serde::{Deserialize, Serialize};

use crate::references::ReferenceIndexEntry;
use crate::store::errors::{StoreDiagnostic, StoreDiagnosticSeverity, make_store_diagnostic};

/// Health status of the root, store, and references.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipHealth {
    pub root: RootHealth,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<StoreHealth>,
    pub references: Vec<ReferenceIndexEntry>,
    pub status: Vec<StoreDiagnostic>,
}

/// Root health information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootHealth {
    pub path: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_id: Option<String>,
    pub healthy: bool,
    pub status: Vec<StoreDiagnostic>,
}

/// Store health information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreHealth {
    pub id: String,
    pub metadata: StoreMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drift: Option<StoreDrift>,
    pub status: Vec<StoreDiagnostic>,
}

/// Store metadata facts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreMetadata {
    pub present: bool,
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
}

/// Store drift (ahead/behind counts).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreDrift {
    pub ahead: usize,
    pub behind: usize,
}

/// Input for inspecting relationships.
#[derive(Debug, Clone)]
pub struct InspectRelationshipsInput {
    pub root_path: String,
    pub root_source: String,
    pub root_store_id: Option<String>,
    pub root_healthy: bool,
    pub root_status: Option<Vec<StoreDiagnostic>>,
    pub store_facts: Option<StoreFacts>,
    pub reference_entries: Vec<ReferenceIndexEntry>,
    pub registry_unreadable: bool,
    pub both_shapes_pointer: Option<PointerWarning>,
    pub malformed_pointer: Option<MalformedPointerWarning>,
    pub inert_pointer_declarations: Option<InertDeclarations>,
}

/// Store facts for store-backed roots.
#[derive(Debug, Clone)]
pub struct StoreFacts {
    pub id: String,
    pub metadata_present: bool,
    pub metadata_valid: bool,
    pub canonical_remote: Option<String>,
    pub origin_url: Option<String>,
    pub drift: Option<StoreDrift>,
}

/// A pointer warning.
#[derive(Debug, Clone)]
pub struct PointerWarning {
    pub value: String,
    pub file_path: String,
}

/// A malformed pointer warning.
#[derive(Debug, Clone)]
pub struct MalformedPointerWarning {
    pub file_path: String,
    pub reason: String,
}

/// Inert declarations in a pointer directory.
#[derive(Debug, Clone)]
pub struct InertDeclarations {
    pub file_path: String,
    pub fields: Vec<String>,
}

/// Build a warning diagnostic.
fn warning(code: &str, message: &str, fix: &str) -> StoreDiagnostic {
    make_store_diagnostic(
        StoreDiagnosticSeverity::Warning,
        code,
        message,
        Some("relationships".to_string()),
        Some(fix.to_string()),
    )
}

/// Build an info diagnostic.
fn info(code: &str, message: &str) -> StoreDiagnostic {
    make_store_diagnostic(
        StoreDiagnosticSeverity::Info,
        code,
        message,
        Some("store.metadata".to_string()),
        None,
    )
}

/// Inspect the relationship health of a root.
pub fn inspect_relationships(input: &InspectRelationshipsInput) -> RelationshipHealth {
    let mut status = Vec::new();

    if input.registry_unreadable {
        status.push(warning(
            "relationship_registry_unreadable",
            "The store registry is unreadable; reference health cannot be checked.",
            "Run: speckit store doctor",
        ));
    }

    if let Some(ref pointer) = input.both_shapes_pointer {
        status.push(warning(
            "root_pointer_ignored",
            &format!(
                "{} declares store '{}', but this directory is a real Speckit root; the declaration is ignored.",
                pointer.file_path, pointer.value
            ),
            &format!(
                "Remove the store: line from {}, or move the planning files into the store.",
                pointer.file_path
            ),
        ));
    }

    if let Some(ref malformed) = input.malformed_pointer {
        status.push(warning(
            "root_pointer_invalid",
            &format!(
                "{} declares a store: pointer that cannot be used ({}).",
                malformed.file_path, malformed.reason
            ),
            &format!("Fix or remove the store: line in {}.", malformed.file_path),
        ));
    }

    if let Some(ref inert) = input.inert_pointer_declarations
        && !inert.fields.is_empty()
    {
        status.push(warning(
                "pointer_declarations_inert",
                &format!(
                    "{} declares {}, but commands read the resolved store's config -- these declarations are inert.",
                    inert.file_path,
                    inert.fields.join(" and ")
                ),
                &format!(
                    "Move the {} declarations into the store's speckit/config.yaml.",
                    inert.fields.join("/")
                ),
            ));
    }

    // Store section
    let store = input.store_facts.as_ref().map(|facts| {
        let mut store_status = Vec::new();

        if let (Some(canonical), Some(origin)) = (&facts.canonical_remote, &facts.origin_url)
            && canonical != origin
        {
            store_status.push(info(
                "store_remote_divergence",
                &format!(
                    "The store.yaml remote ({}) differs from the checkout's origin ({}).",
                    canonical, origin
                ),
            ));
        }

        if let Some(ref drift) = facts.drift
            && drift.behind > 0
        {
            let behind_commits = format!("commit{}", if drift.behind == 1 { "" } else { "s" });
            store_status.push(info(
                "store_checkout_drift",
                &format!(
                    "This store checkout is {} behind its upstream tracking branch; \
                         teammates on newer commits may resolve different specs.",
                    behind_commits
                ),
            ));
        }

        StoreHealth {
            id: facts.id.clone(),
            metadata: StoreMetadata {
                present: facts.metadata_present,
                valid: facts.metadata_valid,
                remote: facts.canonical_remote.clone(),
            },
            origin_url: facts.origin_url.clone(),
            drift: facts.drift.clone(),
            status: store_status,
        }
    });

    RelationshipHealth {
        root: RootHealth {
            path: input.root_path.clone(),
            source: input.root_source.clone(),
            store_id: input.root_store_id.clone(),
            healthy: input.root_healthy,
            status: input.root_status.clone().unwrap_or_default(),
        },
        store,
        references: input.reference_entries.clone(),
        status,
    }
}
