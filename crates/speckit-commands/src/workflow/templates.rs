//! Templates Command
//!
//! Shows resolved template paths for all artifacts in a schema.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::shared::{DEFAULT_SCHEMA, get_schema_dir, validate_schema_exists};

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

/// Options for the templates command.
#[derive(Debug, Clone)]
pub struct TemplatesOptions {
    pub schema: Option<String>,
    pub json: bool,
}

/// Information about a single template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub artifact_id: String,
    pub template_path: String,
    pub source: String,
}

// -----------------------------------------------------------------------------
// Command Implementation
// -----------------------------------------------------------------------------

/// Execute the templates command.
pub async fn templates_command(options: TemplatesOptions) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?.to_string_lossy().to_string();

    let schema_name = options
        .schema
        .clone()
        .unwrap_or_else(|| DEFAULT_SCHEMA.to_string());
    validate_schema_exists(&schema_name, &project_root)?;

    let schema_dir = get_schema_dir(&schema_name, &project_root)
        .ok_or_else(|| anyhow::anyhow!("Schema '{}' not found", schema_name))?;

    let source = determine_source(&schema_dir, &project_root);
    let templates_dir = Path::new(&schema_dir).join("templates");

    // Parse artifacts from schema.yaml
    let schema_path = Path::new(&schema_dir).join("schema.yaml");
    let content = std::fs::read_to_string(&schema_path)?;
    let artifact_ids = parse_artifact_ids(&content);

    let templates: Vec<TemplateInfo> = artifact_ids
        .iter()
        .map(|id| {
            // Each artifact's template is typically at templates/<artifact-id>.md
            // or as specified in the schema
            let template_path = templates_dir.join(format!("{id}.md"));
            let resolved = if template_path.exists() {
                template_path.to_string_lossy().to_string()
            } else {
                // Try nested paths
                let nested = templates_dir.join(id).join("spec.md");
                if nested.exists() {
                    nested.to_string_lossy().to_string()
                } else {
                    template_path.to_string_lossy().to_string()
                }
            };
            TemplateInfo {
                artifact_id: id.clone(),
                template_path: resolved,
                source: source.clone(),
            }
        })
        .collect();

    if options.json {
        let output: serde_json::Value = templates
            .iter()
            .map(|t| {
                (
                    t.artifact_id.clone(),
                    serde_json::json!({
                        "path": t.template_path,
                        "source": t.source,
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>()
            .into();
        crate::shared_output::print_json(&output);
        return Ok(());
    }

    println!("Schema: {schema_name}");
    println!("Source: {source}");
    println!();

    for t in &templates {
        println!("{}:", t.artifact_id);
        println!("  {}", t.template_path);
    }

    Ok(())
}

/// Determine the source of a schema directory.
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

/// Parse artifact IDs from schema YAML content.
fn parse_artifact_ids(content: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(id) = trimmed.strip_prefix("- id:") {
            let id = id.trim().trim_matches('"').to_string();
            if !id.is_empty() {
                ids.push(id);
            }
        }
    }
    if ids.is_empty() {
        ids.extend([
            "proposal".to_string(),
            "specs".to_string(),
            "design".to_string(),
            "tasks".to_string(),
        ]);
    }
    ids
}
