//! Artifact graph system for Speckit.
//!
//! This module provides the core types and operations for managing artifact
//! dependency graphs: schema parsing, graph construction, state tracking,
//! instruction generation, and output resolution.

pub mod graph;
pub mod instruction_loader;
pub mod outputs;
pub mod resolver;
pub mod schema;
pub mod state;
pub mod types;

// Re-export core types
pub use graph::ArtifactGraph;
pub use instruction_loader::{
    ArtifactInstructions, ArtifactPathSummary, ArtifactStatus, ArtifactStatusKind, ChangeContext,
    ChangeStatus, DependencyInfo, LoadChangeContextOptions, SKIP_SPECS_INSTRUCTIONS_WARNING,
    TemplateLoadError, format_change_status, generate_instructions, load_change_context,
    load_template,
};
pub use outputs::{
    artifact_output_exists, is_glob_pattern, resolve_artifact_output_path, resolve_artifact_outputs,
};
pub use resolver::{
    SchemaInfo, SchemaLoadError, SchemaSource, get_package_schemas_dir, get_project_schemas_dir,
    get_schema_dir, get_user_schemas_dir, list_schemas, list_schemas_with_info, resolve_schema,
};
pub use schema::{SchemaValidationError, load_schema, parse_schema};
pub use state::detect_completed;
pub use types::{Artifact, BlockedArtifacts, CompletedSet, ImplementPhase, SchemaYaml};
