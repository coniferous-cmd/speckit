//! Workflow CLI Commands
//!
//! Commands for the artifact-driven workflow: status, instructions,
//! templates, schemas, new change.

pub mod instructions;
pub mod new_change;
pub mod schemas;
pub mod shared;
pub mod status;
pub mod templates;

// Re-export commonly used items
pub use instructions::{
    InstructionsOptions, apply_instructions_command, archive_instructions_command,
    instructions_command,
};
pub use new_change::{NewChangeOptions, new_change_command};
pub use schemas::{SchemasOptions, schemas_command};
pub use shared::DEFAULT_SCHEMA;
pub use status::{StatusOptions, status_command};
pub use templates::{TemplatesOptions, templates_command};
