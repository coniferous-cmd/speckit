//! Schema Command
//!
//! Manage workflow schemas [experimental].

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::shared_output::StoreDiagnostic;
use crate::workflow::shared::{get_schema_dir, list_schemas};

/// A schema location for output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaResolution {
    pub name: String,
    pub source: String,
    pub path: String,
    #[serde(default)]
    pub shadows: Vec<SchemaShadow>,
}

/// A shadowed schema location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaShadow {
    pub source: String,
    pub path: String,
}

/// Validation issue for schemas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaValidationIssue {
    pub level: String,
    pub path: String,
    pub message: String,
}

/// Execute the schema which command.
pub async fn schema_which(name: Option<&str>, json: bool, all: bool) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?.to_string_lossy().to_string();

    if all {
        let schemas = get_all_schemas_with_resolution(&project_root);
        if json {
            crate::shared_output::print_json(&schemas);
            return Ok(());
        }
        if schemas.is_empty() {
            println!("No schemas found.");
            return Ok(());
        }

        println!("Project schemas:");
        for s in schemas.iter().filter(|s| s.source == "project") {
            let shadow_info = if s.shadows.is_empty() {
                String::new()
            } else {
                let sources: Vec<&str> = s.shadows.iter().map(|s| s.source.as_str()).collect();
                format!(" (shadows: {})", sources.join(", "))
            };
            println!("  {}{shadow_info}", s.name);
        }

        println!("User schemas:");
        for s in schemas.iter().filter(|s| s.source == "user") {
            let shadow_info = if s.shadows.is_empty() {
                String::new()
            } else {
                let sources: Vec<&str> = s.shadows.iter().map(|s| s.source.as_str()).collect();
                format!(" (shadows: {})", sources.join(", "))
            };
            println!("  {}{shadow_info}", s.name);
        }

        println!("Package schemas:");
        for s in schemas.iter().filter(|s| s.source == "package") {
            println!("  {}", s.name);
        }

        return Ok(());
    }

    let schema_name = match name {
        Some(n) => n.to_string(),
        None => {
            eprintln!("Error: Schema name is required (or use --all to list all schemas)");
            std::process::exit(1);
        }
    };

    let resolution = get_schema_resolution(&schema_name, &project_root);
    match resolution {
        Some(res) => {
            if json {
                crate::shared_output::print_json(&res);
            } else {
                println!("Schema: {}", res.name);
                println!("Source: {}", res.source);
                println!("Path: {}", res.path);
                if !res.shadows.is_empty() {
                    println!();
                    println!("Shadows:");
                    for shadow in &res.shadows {
                        println!("  {}: {}", shadow.source, shadow.path);
                    }
                }
            }
            Ok(())
        }
        None => {
            let available = list_schemas(&project_root);
            if json {
                crate::shared_output::print_json(&serde_json::json!({
                    "error": format!("Schema '{}' not found", schema_name),
                    "available": available,
                }));
            } else {
                eprintln!("Error: Schema '{}' not found", schema_name);
                eprintln!("Available schemas: {}", available.join(", "));
            }
            std::process::exit(1);
        }
    }
}

/// Execute the schema validate command.
pub async fn schema_validate(name: Option<&str>, json: bool, verbose: bool) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?.to_string_lossy().to_string();

    match name {
        Some(schema_name) => {
            let schema_dir = get_schema_dir(schema_name, &project_root);
            match schema_dir {
                Some(dir) => {
                    let result = validate_schema_dir(&dir, verbose);
                    if json {
                        crate::shared_output::print_json(&serde_json::json!({
                            "name": schema_name,
                            "path": dir,
                            "valid": result.0,
                            "issues": result.1,
                        }));
                    } else if result.0 {
                        println!("\u{2713} Schema '{schema_name}' is valid");
                    } else {
                        println!("\u{2717} Schema '{schema_name}' has errors:");
                        for issue in &result.1 {
                            println!("  {}: {}", issue.level, issue.message);
                        }
                    }
                    if !result.0 {
                        std::process::exit(1);
                    }
                    Ok(())
                }
                None => {
                    let available = list_schemas(&project_root);
                    if json {
                        crate::shared_output::print_json(&serde_json::json!({
                            "valid": false,
                            "error": format!("Schema '{}' not found", schema_name),
                            "available": available,
                        }));
                    } else {
                        eprintln!("Error: Schema '{}' not found", schema_name);
                        eprintln!("Available schemas: {}", available.join(", "));
                    }
                    std::process::exit(1);
                }
            }
        }
        None => {
            // Validate all project schemas
            let project_schemas_dir = Path::new(&project_root).join("speckit").join("schemas");
            if !project_schemas_dir.is_dir() {
                if json {
                    crate::shared_output::print_json(&serde_json::json!({
                        "valid": true,
                        "message": "No project schemas directory found",
                        "schemas": [],
                    }));
                } else {
                    println!("No project schemas directory found.");
                }
                return Ok(());
            }

            let mut results = Vec::new();
            let mut any_invalid = false;

            if let Ok(entries) = std::fs::read_dir(&project_schemas_dir) {
                for entry in entries.flatten() {
                    if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                        continue;
                    }
                    let schema_dir = entry.path();
                    let schema_path = schema_dir.join("schema.yaml");
                    if !schema_path.exists() {
                        continue;
                    }

                    let name = entry.file_name().to_string_lossy().to_string();
                    if verbose && !json {
                        println!();
                        println!("Validating {name}...");
                    }

                    let result =
                        validate_schema_dir(&schema_dir.to_string_lossy(), verbose && !json);
                    if !result.0 {
                        any_invalid = true;
                    }

                    results.push(serde_json::json!({
                        "name": name,
                        "path": schema_dir.to_string_lossy(),
                        "valid": result.0,
                        "issues": result.1,
                    }));
                }
            }

            if json {
                crate::shared_output::print_json(&serde_json::json!({
                    "valid": !any_invalid,
                    "schemas": results,
                }));
            } else {
                if results.is_empty() {
                    println!("No schemas found in project.");
                    return Ok(());
                }
                println!();
                println!("Validation Results:");
                for result in &results {
                    let valid = result["valid"].as_bool().unwrap_or(false);
                    let status = if valid { "\u{2713}" } else { "\u{2717}" };
                    let name = result["name"].as_str().unwrap_or("unknown");
                    println!("  {status} {name}");
                    if let Some(issues) = result["issues"].as_array() {
                        for issue in issues {
                            let level = issue["level"].as_str().unwrap_or("unknown");
                            let msg = issue["message"].as_str().unwrap_or("");
                            println!("    {level}: {msg}");
                        }
                    }
                }
            }

            if any_invalid {
                std::process::exit(1);
            }
            Ok(())
        }
    }
}

/// Execute the schema fork command.
pub async fn schema_fork(
    source: &str,
    name: Option<&str>,
    json: bool,
    force: bool,
) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?.to_string_lossy().to_string();

    let destination_name = name
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{source}-custom"));

    // Validate destination name
    if let Err(e) = validate_schema_name(&destination_name) {
        if json {
            crate::shared_output::print_json(&serde_json::json!({
                "forked": false,
                "error": e.to_string(),
            }));
        } else {
            eprintln!("Error: {e}");
        }
        std::process::exit(1);
    }

    // Find source schema
    let source_dir = get_schema_dir(source, &project_root);
    let source_dir = match source_dir {
        Some(d) => d,
        None => {
            let available = list_schemas(&project_root);
            if json {
                crate::shared_output::print_json(&serde_json::json!({
                    "forked": false,
                    "error": format!("Schema '{}' not found", source),
                    "available": available,
                }));
            } else {
                eprintln!("Error: Schema '{}' not found", source);
                eprintln!("Available schemas: {}", available.join(", "));
            }
            std::process::exit(1);
        }
    };

    let schemas_dir = Path::new(&project_root).join("speckit").join("schemas");
    let destination_dir = schemas_dir.join(&destination_name);

    // Check destination exists
    if destination_dir.exists() && !force {
        if json {
            crate::shared_output::print_json(&serde_json::json!({
                "forked": false,
                "error": format!("Schema '{}' already exists", destination_name),
                "suggestion": "Use --force to overwrite",
            }));
        } else {
            eprintln!(
                "Error: Schema '{}' already exists at {}",
                destination_name,
                destination_dir.display()
            );
            eprintln!("Use --force to overwrite");
        }
        std::process::exit(1);
    }

    // Copy schema directory
    if destination_dir.exists() && force {
        std::fs::remove_dir_all(&destination_dir)?;
    }

    copy_dir_recursive(Path::new(&source_dir), &destination_dir)?;

    // Update name in schema.yaml
    let schema_path = destination_dir.join("schema.yaml");
    if schema_path.exists() {
        let content = std::fs::read_to_string(&schema_path)?;
        let new_content = content.replace(
            &format!("name: {source}"),
            &format!("name: {destination_name}"),
        );
        std::fs::write(&schema_path, new_content)?;
    }

    if json {
        crate::shared_output::print_json(&serde_json::json!({
            "forked": true,
            "source": source,
            "sourcePath": source_dir,
            "destination": destination_name,
            "destinationPath": destination_dir.to_string_lossy(),
        }));
    } else {
        println!();
        println!("Source: {source_dir}");
        println!("Destination: {}", destination_dir.display());
        println!();
        println!("You can now customize the schema at:");
        println!("  {}/schema.yaml", destination_dir.display());
    }

    Ok(())
}

/// Execute the schema init command.
pub async fn schema_init(
    name: &str,
    json: bool,
    description: Option<&str>,
    artifacts: Option<&str>,
    set_default: bool,
    force: bool,
) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?.to_string_lossy().to_string();

    // Validate name
    if let Err(e) = validate_schema_name(name) {
        if json {
            crate::shared_output::print_json(&serde_json::json!({
                "created": false,
                "error": e.to_string(),
            }));
        } else {
            eprintln!("Error: {e}");
        }
        std::process::exit(1);
    }

    let schema_dir = Path::new(&project_root)
        .join("speckit")
        .join("schemas")
        .join(name);

    if schema_dir.exists() && !force {
        if json {
            crate::shared_output::print_json(&serde_json::json!({
                "created": false,
                "error": format!("Schema '{}' already exists", name),
                "suggestion": "Use --force to overwrite",
            }));
        } else {
            eprintln!(
                "Error: Schema '{}' already exists at {}",
                name,
                schema_dir.display()
            );
            eprintln!("Use --force to overwrite or \"speckit schema fork\" to copy");
        }
        std::process::exit(1);
    }

    let desc = description
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Custom workflow schema for {name}"));

    let artifact_ids: Vec<String> = match artifacts {
        Some(list) => list.split(',').map(|s| s.trim().to_string()).collect(),
        None => vec![
            "proposal".to_string(),
            "specs".to_string(),
            "design".to_string(),
            "tasks".to_string(),
        ],
    };

    // Remove existing if force
    if schema_dir.exists() && force {
        std::fs::remove_dir_all(&schema_dir)?;
    }

    // Create schema directory
    std::fs::create_dir_all(&schema_dir)?;

    // Write schema.yaml
    let schema_yaml = build_schema_yaml(name, &desc, &artifact_ids);
    std::fs::write(schema_dir.join("schema.yaml"), schema_yaml)?;

    // Create templates
    let templates_dir = schema_dir.join("templates");
    std::fs::create_dir_all(&templates_dir)?;
    for artifact_id in &artifact_ids {
        let template_content = create_default_template(artifact_id);
        let template_path = templates_dir.join(format!("{artifact_id}.md"));
        std::fs::write(&template_path, template_content)?;
    }

    // Set as default if requested
    if set_default {
        let config_path = Path::new(&project_root).join("speckit").join("config.yaml");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let mut config: serde_yaml::Value = serde_yaml::from_str(&content).unwrap_or_default();
            if let Some(mapping) = config.as_mapping_mut() {
                mapping.insert(
                    serde_yaml::Value::String("defaultSchema".to_string()),
                    serde_yaml::Value::String(name.to_string()),
                );
            }
            std::fs::write(&config_path, serde_yaml::to_string(&config)?)?;
        } else {
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let config = serde_json::json!({
                "defaultSchema": name
            });
            std::fs::write(&config_path, serde_yaml::to_string(&config)?)?;
        }
    }

    if json {
        crate::shared_output::print_json(&serde_json::json!({
            "created": true,
            "path": schema_dir.to_string_lossy(),
            "schema": name,
            "artifacts": artifact_ids,
            "setAsDefault": set_default,
        }));
    } else {
        println!();
        println!("Schema created at: {}", schema_dir.display());
        println!();
        println!("Artifacts: {}", artifact_ids.join(", "));
        if set_default {
            println!();
            println!("Set as project default schema.");
        }
        println!();
        println!("Next steps:");
        println!(
            "  1. Edit {}/schema.yaml to customize artifacts",
            schema_dir.display()
        );
        println!("  2. Modify templates in the schema directory");
        println!("  3. Use with: speckit new --schema {name}");
    }

    Ok(())
}

/// Get all schemas with resolution info.
fn get_all_schemas_with_resolution(project_root: &str) -> Vec<SchemaResolution> {
    let names = list_schemas(project_root);
    names
        .iter()
        .filter_map(|name| get_schema_resolution(name, project_root))
        .collect()
}

/// Get resolution info for a schema.
fn get_schema_resolution(name: &str, project_root: &str) -> Option<SchemaResolution> {
    let schema_dir = get_schema_dir(name, project_root)?;
    let source = determine_source(&schema_dir, project_root);
    Some(SchemaResolution {
        name: name.to_string(),
        source,
        path: schema_dir,
        shadows: Vec::new(),
    })
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

/// Validate a schema directory.
fn validate_schema_dir(schema_dir: &str, verbose: bool) -> (bool, Vec<SchemaValidationIssue>) {
    let mut issues = Vec::new();
    let schema_path = Path::new(schema_dir).join("schema.yaml");

    if verbose {
        println!("  Checking schema.yaml exists...");
    }

    if !schema_path.exists() {
        issues.push(SchemaValidationIssue {
            level: "error".to_string(),
            path: "schema.yaml".to_string(),
            message: "schema.yaml not found".to_string(),
        });
        return (false, issues);
    }

    if verbose {
        println!("  Parsing YAML...");
    }

    let content = match std::fs::read_to_string(&schema_path) {
        Ok(c) => c,
        Err(e) => {
            issues.push(SchemaValidationIssue {
                level: "error".to_string(),
                path: "schema.yaml".to_string(),
                message: format!("Failed to read file: {e}"),
            });
            return (false, issues);
        }
    };

    if verbose {
        println!("  Validating schema structure...");
    }

    // Validate YAML structure
    let schema: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            issues.push(SchemaValidationIssue {
                level: "error".to_string(),
                path: "schema.yaml".to_string(),
                message: format!("Parse error: {e}"),
            });
            return (false, issues);
        }
    };

    // Check for required fields
    if schema.get("name").is_none() {
        issues.push(SchemaValidationIssue {
            level: "error".to_string(),
            path: "schema.yaml".to_string(),
            message: "Missing required field: name".to_string(),
        });
    }

    if schema.get("artifacts").is_none() {
        issues.push(SchemaValidationIssue {
            level: "error".to_string(),
            path: "schema.yaml".to_string(),
            message: "Missing required field: artifacts".to_string(),
        });
    }

    // Check template files
    if verbose {
        println!("  Checking template files...");
    }

    if let Some(artifacts) = schema.get("artifacts").and_then(|a| a.as_sequence()) {
        let templates_dir = Path::new(schema_dir).join("templates");
        for artifact in artifacts {
            if let Some(template) = artifact.get("template").and_then(|t| t.as_str()) {
                let template_path = templates_dir.join(template);
                if !template_path.exists() {
                    let artifact_id = artifact
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("unknown");
                    issues.push(SchemaValidationIssue {
                        level: "error".to_string(),
                        path: format!("artifacts.{artifact_id}.template"),
                        message: format!(
                            "Template file '{template}' not found for artifact '{artifact_id}'"
                        ),
                    });
                }
            }
        }
    }

    (issues.is_empty(), issues)
}

/// Build schema YAML content.
fn build_schema_yaml(name: &str, description: &str, artifact_ids: &[String]) -> String {
    let mut yaml =
        format!("name: {name}\nversion: 1\ndescription: \"{description}\"\nartifacts:\n");

    for id in artifact_ids {
        let (generates, template) = match id.as_str() {
            "proposal" => ("proposal.md", "proposal.md"),
            "specs" => ("specs/**/*.md", "specs/spec.md"),
            "design" => ("design.md", "design.md"),
            "tasks" => ("tasks.md", "tasks.md"),
            _ => ("unknown", "unknown"),
        };
        yaml.push_str(&format!(
            "  - id: {id}\n    generates: {generates}\n    template: {template}\n    requires: []\n"
        ));
    }

    if artifact_ids.contains(&"tasks".to_string()) {
        yaml.push_str("apply:\n  requires:\n    - tasks\n  tracks: tasks.md\n");
    }

    yaml
}

/// Create default template content for an artifact.
fn create_default_template(artifact_id: &str) -> &'static str {
    match artifact_id {
        "proposal" => include_str!("../templates/proposal.md"),
        "specs" => include_str!("../templates/specs.md"),
        "design" => include_str!("../templates/design.md"),
        "tasks" => include_str!("../templates/tasks.md"),
        _ => "## {{artifact_id}}\n\n<!-- Add content here -->\n",
    }
}

/// Copy a directory recursively.
fn copy_dir_recursive(src: &Path, dest: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest_path = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

/// Validate a schema name (kebab-case).
pub fn validate_schema_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        anyhow::bail!("Schema name cannot be empty");
    }
    let re = regex::Regex::new(r"^[a-z][a-z0-9]*(-[a-z0-9]+)*$").unwrap();
    if !re.is_match(name) {
        anyhow::bail!(
            "Invalid schema name '{}'. Use kebab-case (e.g., my-workflow)",
            name
        );
    }
    Ok(())
}
