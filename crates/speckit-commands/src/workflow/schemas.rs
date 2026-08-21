//! Schemas Command
//!
//! Lists available workflow schemas with descriptions.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::shared::{get_schema_dir, list_schemas, print_json};

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

/// Options for the schemas command.
#[derive(Debug, Clone)]
pub struct SchemasOptions {
    pub store: Option<String>,
    pub json: bool,
}

/// Information about a single schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaInfo {
    pub name: String,
    pub description: String,
    pub artifacts: Vec<String>,
    pub source: String,
}

// -----------------------------------------------------------------------------
// Command Implementation
// -----------------------------------------------------------------------------

/// Execute the schemas command.
pub async fn schemas_command(options: SchemasOptions) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?.to_string_lossy().to_string();

    let schemas = list_schemas_with_info(&project_root);

    if options.json {
        print_json(&schemas);
        return Ok(());
    }

    if schemas.is_empty() {
        println!("No schemas found.");
        return Ok(());
    }

    println!("Available schemas:");
    println!();

    for schema in &schemas {
        let source_label = match schema.source.as_str() {
            "project" => " (project)",
            "user" => " (user override)",
            _ => "",
        };
        println!("  {}{}", schema.name, source_label);
        println!("    {}", schema.description);
        println!("    Artifacts: {}", schema.artifacts.join(" -> "));
        println!();
    }

    Ok(())
}

/// List all schemas with their metadata.
pub fn list_schemas_with_info(project_root: &str) -> Vec<SchemaInfo> {
    let names = list_schemas(project_root);
    let mut schemas = Vec::new();

    for name in names {
        let schema_dir = get_schema_dir(&name, project_root);
        let (description, artifacts, source) = match schema_dir {
            Some(ref dir) => {
                let schema_path = Path::new(dir).join("schema.yaml");
                let source = determine_source(dir, project_root);
                match std::fs::read_to_string(&schema_path) {
                    Ok(content) => {
                        let (desc, arts) = parse_schema_info(&content);
                        (desc, arts, source)
                    }
                    Err(_) => (
                        format!("Schema: {name}"),
                        vec![
                            "proposal".to_string(),
                            "specs".to_string(),
                            "design".to_string(),
                            "tasks".to_string(),
                        ],
                        source,
                    ),
                }
            }
            None => (
                format!("Schema: {name}"),
                vec![
                    "proposal".to_string(),
                    "specs".to_string(),
                    "design".to_string(),
                    "tasks".to_string(),
                ],
                "package".to_string(),
            ),
        };

        schemas.push(SchemaInfo {
            name,
            description,
            artifacts,
            source,
        });
    }

    schemas
}

/// Determine the source of a schema (project, user, or package).
fn determine_source(schema_dir: &str, project_root: &str) -> String {
    let dir = Path::new(schema_dir);
    let project_schemas = Path::new(project_root).join("speckit").join("schemas");

    if let Ok(relative) = dir.strip_prefix(&project_schemas) {
        if !relative.starts_with("..") {
            return "project".to_string();
        }
    }

    if let Some(config_dir) = dirs::config_dir() {
        let user_schemas = config_dir.join("speckit").join("schemas");
        if let Ok(relative) = dir.strip_prefix(&user_schemas) {
            if !relative.starts_with("..") {
                return "user".to_string();
            }
        }
    }

    "package".to_string()
}

/// Parse description and artifact IDs from schema YAML content.
fn parse_schema_info(content: &str) -> (String, Vec<String>) {
    let mut description = String::new();
    let mut artifacts = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(desc) = trimmed.strip_prefix("description:") {
            description = desc.trim().trim_matches('"').to_string();
        }
        if let Some(id) = trimmed.strip_prefix("- id:") {
            let id = id.trim().trim_matches('"').to_string();
            if !id.is_empty() {
                artifacts.push(id);
            }
        }
    }

    if description.is_empty() {
        description = "Workflow schema".to_string();
    }

    (description, artifacts)
}
