//! Instructions Command
//!
//! Generates enriched instructions for creating artifacts or applying tasks.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use speckit_core::project_config::{ProjectConfig, load_operation_inputs};

use super::shared::{
    ArchiveInstructions, ImplementInstructions, ImplementProgress, TaskItem, print_json,
    validate_change_exists,
};
use crate::shared_gather::{
    ReferenceIndexEntry, assemble_reference_index, read_project_config, read_registry_snapshot,
};
use speckit_core::artifact_graph::{
    ChangeContext, LoadChangeContextOptions, generate_instructions as generate_core_instructions,
    load_change_context, resolve_artifact_outputs,
};

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

/// Options for the instructions command.
#[derive(Debug, Clone)]
pub struct InstructionsOptions {
    pub change: Option<String>,
    pub schema: Option<String>,
    pub store: Option<String>,
    pub json: bool,
}

/// Options for the implement instructions command.
pub type ImplementInstructionsOptions = InstructionsOptions;

/// Options for the archive instructions command.
pub type ArchiveInstructionsOptions = InstructionsOptions;

/// Rich instructions for creating a single artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactInstructions {
    pub artifact_id: String,
    pub change_name: String,
    pub schema_name: String,
    pub change_dir: String,
    pub resolved_output_path: String,
    pub description: String,
    pub instruction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    pub rules: Vec<String>,
    pub template: String,
    pub dependencies: Vec<DependencyInfo>,
    pub unlocks: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<Vec<ReferenceIndexEntry>>,
    #[serde(default)]
    pub skipped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// Information about a dependency artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInfo {
    pub id: String,
    pub path: String,
    pub description: String,
    pub done: bool,
    #[serde(default)]
    pub skipped: bool,
}

// -----------------------------------------------------------------------------
// Artifact Instructions Command
// -----------------------------------------------------------------------------

/// Execute the instructions command for a specific artifact.
pub async fn instructions_command(
    artifact_id: Option<&str>,
    options: InstructionsOptions,
) -> anyhow::Result<()> {
    let project_root = super::resolve_project_root(options.store.as_deref()).await?;

    let change_name = validate_change_exists(
        options.change.as_deref(),
        &project_root,
        None,
        Some("speckit new change <name>"),
    )
    .await?;

    let id = match artifact_id {
        Some(id) => id.to_string(),
        None => {
            anyhow::bail!("Missing required argument <artifact>");
        }
    };

    let change_dir = Path::new(&project_root)
        .join("speckit")
        .join("changes")
        .join(&change_name);

    let context = load_change_context(
        Path::new(&project_root),
        &change_name,
        options.schema.as_deref(),
        LoadChangeContextOptions {
            change_dir: Some(change_dir),
        },
    )
    .map_err(|error| anyhow::anyhow!("Unable to load change instructions: {error}"))?;

    let instructions = build_artifact_instructions(&id, &context, &project_root).await?;

    if options.json {
        print_json(&instructions);
        return Ok(());
    }

    print_instructions_text(&instructions);
    Ok(())
}

/// Build artifact instructions from the filesystem state.
async fn build_artifact_instructions(
    artifact_id: &str,
    context: &ChangeContext,
    project_root: &str,
) -> anyhow::Result<ArtifactInstructions> {
    let core_instructions = generate_core_instructions(context, artifact_id)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let fallback_artifact_id = core_instructions.artifact_id.clone();
    let fallback_change_name = core_instructions.change_name.clone();
    let dependencies = core_instructions
        .dependencies
        .into_iter()
        .map(|dependency| DependencyInfo {
            id: dependency.id,
            path: dependency.path,
            description: dependency.description,
            done: dependency.done,
            skipped: dependency.skipped.unwrap_or(false),
        })
        .collect::<Vec<_>>();
    let is_blocked = dependencies.iter().any(|dependency| !dependency.done);

    // Load project context
    let project_config = read_project_config(project_root);
    let context = project_config
        .as_ref()
        .and_then(|_| Some(format!("Project root: {project_root}")));

    // Build references
    let registry = read_registry_snapshot().await;
    let references_decl = project_config
        .as_ref()
        .map(|c| c.references.clone())
        .unwrap_or_default();
    let references = if references_decl.is_empty() {
        None
    } else {
        let idx = assemble_reference_index(&references_decl, &registry.entries, project_root).await;
        if idx.is_empty() { None } else { Some(idx) }
    };

    Ok(ArtifactInstructions {
        artifact_id: core_instructions.artifact_id,
        change_name: core_instructions.change_name,
        schema_name: core_instructions.schema_name,
        change_dir: core_instructions
            .change_dir
            .to_string_lossy()
            .replace('\\', "/"),
        resolved_output_path: core_instructions
            .resolved_output_path
            .to_string_lossy()
            .replace('\\', "/"),
        description: core_instructions.description,
        instruction: if is_blocked {
            "Complete the dependencies above before creating this artifact.".to_string()
        } else {
            core_instructions.instruction.unwrap_or_else(|| {
                format!(
                    "Create the {} artifact for change \"{}\".",
                    fallback_artifact_id, fallback_change_name
                )
            })
        },
        context,
        rules: Vec::new(),
        template: prepend_create_time(&core_instructions.template),
        dependencies,
        unlocks: core_instructions.unlocks,
        references,
        skipped: core_instructions.skipped.unwrap_or(false),
        warning: core_instructions.warning,
    })
}

/// Prepend a YAML frontmatter block with a single `create-time` key to a
/// template body. The timestamp is captured at invocation time in local time
/// and formatted as `YYYY-MM-DD HH:MM:SS` with no timezone suffix.
fn prepend_create_time(template: &str) -> String {
    let header = format!(
        "---\ncreate-time: {}\n---\n\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
    );
    format!("{header}{template}")
}

/// Print artifact instructions in human-readable format.
fn print_instructions_text(instructions: &ArtifactInstructions) {
    println!(
        "<artifact id=\"{}\" change=\"{}\" schema=\"{}\">",
        instructions.artifact_id, instructions.change_name, instructions.schema_name
    );
    println!();

    if instructions.skipped {
        println!("<warning>");
        println!(
            "{}",
            instructions
                .warning
                .as_deref()
                .unwrap_or("This artifact is skipped.")
        );
        println!("</warning>");
        println!();
        println!("</artifact>");
        return;
    }

    let is_blocked = instructions.dependencies.iter().any(|d| !d.done);
    if is_blocked {
        let missing: Vec<_> = instructions
            .dependencies
            .iter()
            .filter(|d| !d.done)
            .map(|d| d.id.as_str())
            .collect();
        println!("<warning>");
        println!(
            "This artifact has unmet dependencies. Complete them first or proceed with caution."
        );
        println!("Missing: {}", missing.join(", "));
        println!("</warning>");
        println!();
    }

    println!("<task>");
    println!(
        "Create the {} artifact for change \"{}\".",
        instructions.artifact_id, instructions.change_name
    );
    println!("{}", instructions.description);
    println!("</task>");
    println!();

    if let Some(ref ctx) = instructions.context {
        println!("<project_context>");
        println!(
            "<!-- This is background information for you. Do NOT include this in your output. -->"
        );
        println!("{ctx}");
        println!("</project_context>");
        println!();
    }

    if let Some(ref refs) = instructions.references {
        if !refs.is_empty() {
            println!("<referenced_stores>");
            for entry in refs {
                let root_label = entry.root.as_deref().unwrap_or("unknown");
                println!("  - {} ({})", entry.store_id, root_label);
            }
            println!("</referenced_stores>");
            println!();
        }
    }

    if !instructions.rules.is_empty() {
        println!("<rules>");
        println!(
            "<!-- These are constraints for you to follow. Do NOT include this in your output. -->"
        );
        for rule in &instructions.rules {
            println!("- {rule}");
        }
        println!("</rules>");
        println!();
    }

    if !instructions.dependencies.is_empty() {
        println!("<dependencies>");
        println!("Read the current contents of these files before creating this artifact:");
        println!();
        for dep in &instructions.dependencies {
            let status = if dep.skipped {
                "skipped"
            } else if dep.done {
                "done"
            } else {
                "missing"
            };
            println!("<dependency id=\"{}\" status=\"{}\">", dep.id, status);
            if dep.skipped {
                println!("  <description>Skipped: the change declares skip_specs.</description>");
            } else {
                let full_path = Path::new(&instructions.change_dir).join(&dep.path);
                println!("  <path>{}</path>", full_path.display());
                println!("  <description>{}</description>", dep.description);
            }
            println!("</dependency>");
        }
        println!("</dependencies>");
        println!();
    }

    println!("<output>");
    println!("Write to: {}", instructions.resolved_output_path);
    println!("</output>");
    println!();

    if !instructions.instruction.is_empty() {
        println!("<instruction>");
        println!("{}", instructions.instruction.trim());
        println!("</instruction>");
        println!();
    }

    println!("<template>");
    println!("<!-- Use this as the structure for your output file. -->");
    println!("{}", instructions.template.trim());
    println!("</template>");
    println!();

    if !instructions.unlocks.is_empty() {
        println!("<unlocks>");
        println!(
            "Completing this artifact enables: {}",
            instructions.unlocks.join(", ")
        );
        println!("</unlocks>");
        println!();
    }

    println!("</artifact>");
}

// -----------------------------------------------------------------------------
// Implement Instructions Command
// -----------------------------------------------------------------------------

/// Execute the implement instructions command.
pub async fn implement_instructions_command(
    options: ImplementInstructionsOptions,
) -> anyhow::Result<()> {
    let project_root = super::resolve_project_root(options.store.as_deref()).await?;

    reject_removed_operation_guidance(&project_root)?;

    let change_name = validate_change_exists(
        options.change.as_deref(),
        &project_root,
        None,
        Some("speckit new change <name>"),
    )
    .await?;

    let instructions =
        generate_implement_instructions(&project_root, &change_name, options.schema.as_deref())
            .await?;

    if options.json {
        print_json(&instructions);
        return Ok(());
    }

    print_implement_instructions_text(&instructions);
    Ok(())
}

/// Reject the removed configuration spelling instead of silently dropping it.
fn reject_removed_operation_guidance(project_root: &str) -> anyhow::Result<()> {
    let config_path = Path::new(project_root).join("speckit/config.yaml");
    let Some(content) = std::fs::read_to_string(config_path).ok() else {
        return Ok(());
    };
    let Ok(raw) = serde_yaml::from_str::<serde_yaml::Value>(&content) else {
        return Ok(());
    };
    if raw
        .get("operations")
        .and_then(|operations| operations.get("apply"))
        .is_some()
    {
        anyhow::bail!(
            "The `operations.apply` key has been removed. Replace it with `operations.implement`."
        );
    }
    Ok(())
}

/// Generate implement instructions for implementing tasks from a change.
pub async fn generate_implement_instructions(
    project_root: &str,
    change_name: &str,
    schema_name: Option<&str>,
) -> anyhow::Result<ImplementInstructions> {
    let change_dir = Path::new(project_root)
        .join("speckit")
        .join("changes")
        .join(change_name);

    let change_context = load_change_context(
        Path::new(project_root),
        change_name,
        schema_name,
        LoadChangeContextOptions {
            change_dir: Some(change_dir.clone()),
        },
    )
    .map_err(|error| anyhow::anyhow!("Unable to load change context: {error}"))?;
    let schema = change_context.schema_name.as_str();
    let implement_phase = change_context.graph.schema().implement.as_ref();
    let tracking_file = implement_phase
        .and_then(|phase| phase.tracks.as_deref())
        .unwrap_or("tasks.md");
    let tasks_path = change_dir.join(tracking_file);
    let required_artifacts = implement_phase
        .map(|phase| phase.requires.clone())
        .unwrap_or_default();
    let missing_artifacts: Vec<String> = required_artifacts
        .into_iter()
        .filter(|artifact_id| !change_context.completed.contains(artifact_id))
        .collect();

    // Parse tasks
    let (tasks, total, complete, remaining) = if tasks_path.exists() {
        let content = tokio::fs::read_to_string(&tasks_path)
            .await
            .unwrap_or_default();
        let parsed = parse_task_lines(&content);
        let total = parsed.len();
        let complete = parsed.iter().filter(|t| t.1).count();
        let remaining = total - complete;
        let task_items: Vec<TaskItem> = parsed
            .into_iter()
            .filter(|(_, _, desc)| !desc.is_empty())
            .enumerate()
            .map(|(i, (_, done, desc))| TaskItem {
                id: format!("{}", i + 1),
                description: desc,
                done,
            })
            .collect();
        (task_items, total, complete, remaining)
    } else {
        (Vec::new(), 0, 0, 0)
    };

    // Build context files from every artifact in the active schema.
    let mut context_files: HashMap<String, Vec<String>> = HashMap::new();
    for artifact in change_context.graph.get_all_artifacts() {
        let files = resolve_artifact_outputs(&change_dir, &artifact.generates)
            .into_iter()
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect::<Vec<_>>();
        if !files.is_empty() {
            context_files.insert(artifact.id.clone(), files);
        }
    }

    // Determine state
    let (state, instruction) = if !missing_artifacts.is_empty() {
        (
            "blocked".to_string(),
            format!(
                "Create the required artifacts before implementing this change: {}.",
                missing_artifacts.join(", ")
            ),
        )
    } else if !tasks_path.exists() {
        (
            "blocked".to_string(),
            format!(
                "The {tracking_file} file is missing and must be created.\nUse speckit-continue-change to generate the tracking file."
            ),
        )
    } else if tasks.is_empty() {
        (
            "blocked".to_string(),
            format!(
                "The {tracking_file} file exists but contains no tasks to work on.\nAdd tasks to {tracking_file} or regenerate it with speckit-continue-change."
            ),
        )
    } else if remaining == 0 && total > 0 {
        (
            "all_done".to_string(),
            "All tasks are complete! This change is ready to be archived.\nConsider running tests and reviewing the changes before archiving.".to_string(),
        )
    } else {
        (
            "ready".to_string(),
            implement_phase
                .and_then(|phase| phase.instruction.clone())
                .unwrap_or_else(|| "Read context files, work through pending tasks, mark complete as you go.\nPause if you hit blockers or need clarification.".to_string()),
        )
    };

    // Load references
    let registry = read_registry_snapshot().await;
    let project_config = read_project_config(project_root);
    let core_project_config =
        speckit_core::project_config::read_project_config(Path::new(project_root));
    let operation_inputs = load_operation_inputs(core_project_config.as_ref(), "implement");
    let references_decl = project_config
        .as_ref()
        .map(|c| c.references.clone())
        .unwrap_or_default();
    let references = if references_decl.is_empty() {
        None
    } else {
        let idx = assemble_reference_index(&references_decl, &registry.entries, project_root).await;
        if idx.is_empty() { None } else { Some(idx) }
    };

    Ok(ImplementInstructions {
        change_name: change_name.to_string(),
        change_dir: change_dir.to_string_lossy().replace('\\', "/"),
        schema_name: schema.to_string(),
        context_files,
        progress: ImplementProgress {
            total,
            complete,
            remaining,
        },
        tasks,
        state,
        missing_artifacts: if missing_artifacts.is_empty() {
            None
        } else {
            Some(missing_artifacts)
        },
        instruction,
        references,
        context: operation_inputs.context,
        operation_guidance: operation_inputs.operation_guidance,
    })
}

/// Parse task lines from a tasks.md file content.
fn parse_task_lines(content: &str) -> Vec<(usize, bool, String)> {
    let mut tasks = Vec::new();
    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- [") {
            if rest.len() > 2 && rest.as_bytes()[1] == b']' {
                let done = rest.as_bytes()[0] == b'x' || rest.as_bytes()[0] == b'X';
                let desc = rest[2..].trim().to_string();
                tasks.push((line_no, done, desc));
            }
        }
    }
    tasks
}

/// Print implement instructions in human-readable format.
fn print_implement_instructions_text(instructions: &ImplementInstructions) {
    println!("## Implement: {}", instructions.change_name);
    println!("Schema: {}", instructions.schema_name);
    println!();

    if let Some(ref refs) = instructions.references {
        if !refs.is_empty() {
            println!("Referenced stores:");
            for entry in refs {
                let root_label = entry.root.as_deref().unwrap_or("unknown");
                println!("  - {} ({})", entry.store_id, root_label);
            }
            println!();
        }
    }

    if instructions.state == "blocked" {
        if let Some(ref missing) = instructions.missing_artifacts {
            println!("### Blocked");
            println!();
            println!("Missing artifacts: {}", missing.join(", "));
            println!("Use the speckit-continue-change skill to create these first.");
            println!();
        }
    }

    if !instructions.context_files.is_empty() {
        println!("### Context Files");
        for (artifact_id, file_paths) in &instructions.context_files {
            for file_path in file_paths {
                println!("- {artifact_id}: {file_path}");
            }
        }
        println!();
    }

    if instructions.progress.total > 0 || !instructions.tasks.is_empty() {
        println!("### Progress");
        if instructions.state == "all_done" {
            println!(
                "{}/{} complete \u{2713}",
                instructions.progress.complete, instructions.progress.total
            );
        } else {
            println!(
                "{}/{} complete",
                instructions.progress.complete, instructions.progress.total
            );
        }
        println!();
    }

    if !instructions.tasks.is_empty() {
        println!("### Tasks");
        for task in &instructions.tasks {
            let checkbox = if task.done { "[x]" } else { "[ ]" };
            println!("- {checkbox} {}", task.description);
        }
        println!();
    }

    println!("### Instruction");
    println!("{}", instructions.instruction);
    println!();
}

// -----------------------------------------------------------------------------
// Archive Instructions Command
// -----------------------------------------------------------------------------

/// Execute the archive instructions command.
pub async fn archive_instructions_command(
    options: ArchiveInstructionsOptions,
) -> anyhow::Result<()> {
    let project_root = super::resolve_project_root(options.store.as_deref()).await?;

    let change_name = validate_change_exists(
        options.change.as_deref(),
        &project_root,
        None,
        Some("speckit new change <name>"),
    )
    .await?;

    let project_config =
        speckit_core::project_config::read_project_config(Path::new(&project_root));
    let instructions = generate_archive_instructions(&change_name, project_config.as_ref());

    if options.json {
        print_json(&instructions);
        return Ok(());
    }

    print_archive_instructions_text(&instructions);
    Ok(())
}

/// Generate archive instructions, including configured project context and
/// archive-specific advisory guidance when present.
pub fn generate_archive_instructions(
    change_name: &str,
    project_config: Option<&ProjectConfig>,
) -> ArchiveInstructions {
    let operation_inputs = load_operation_inputs(project_config, "archive");
    ArchiveInstructions {
        change_name: change_name.to_string(),
        context: operation_inputs.context,
        operation_guidance: operation_inputs.operation_guidance,
    }
}

/// Print archive instructions in human-readable format.
fn print_archive_instructions_text(instructions: &ArchiveInstructions) {
    println!("## Archive Inputs: {}", instructions.change_name);
    println!();

    if let Some(ref ctx) = instructions.context {
        println!("### Project Context (required instruction input)");
        println!("{ctx}");
        println!();
    }

    if let Some(ref guidance) = instructions.operation_guidance {
        if !guidance.is_empty() {
            println!("### Operation Guidance (advisory)");
            for g in guidance {
                println!("- {g}");
            }
            println!();
        }
    }

    if instructions.context.is_none() && instructions.operation_guidance.is_none() {
        println!("No project context or operation guidance configured.");
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{generate_archive_instructions, prepend_create_time};
    use chrono::Local;
    use speckit_core::project_config::{OperationConfig, ProjectConfig};

    #[test]
    fn archive_instructions_omit_optional_inputs_without_config() {
        let instructions = generate_archive_instructions("release-notes", None);

        assert_eq!(instructions.change_name, "release-notes");
        assert!(instructions.context.is_none());
        assert!(instructions.operation_guidance.is_none());
    }

    #[test]
    fn archive_instructions_load_project_context_and_archive_guidance() {
        let mut operations = HashMap::new();
        operations.insert(
            "archive".to_string(),
            OperationConfig {
                guidance: Some(vec!["Confirm release notes are published.".to_string()]),
            },
        );
        let config = ProjectConfig {
            schema: "spec-driven".to_string(),
            context: Some("Production changes require a rollout record.".to_string()),
            rules: None,
            operations: Some(operations),
            store: None,
            github_copilot: None,
            references: None,
        };

        let instructions = generate_archive_instructions("release-notes", Some(&config));

        assert_eq!(
            instructions.context.as_deref(),
            Some("Production changes require a rollout record.")
        );
        assert_eq!(
            instructions.operation_guidance,
            Some(vec!["Confirm release notes are published.".to_string()])
        );
    }

    #[test]
    fn prepend_create_time_starts_with_frontmatter() {
        let body = "## Why\n\nbody\n";
        let stamped = prepend_create_time(body);

        let lines: Vec<&str> = stamped.split('\n').collect();
        assert_eq!(lines[0], "---", "first line must be the frontmatter open");
        assert!(
            lines[1].starts_with("create-time: "),
            "second line must hold the create-time key, got: {}",
            lines[1]
        );
        assert_eq!(lines[2], "---", "third line must close the frontmatter");
        assert_eq!(
            lines[3], "",
            "blank line must separate frontmatter from body"
        );
        assert_eq!(lines[4], "## Why", "body must follow unchanged");
    }

    #[test]
    fn prepend_create_time_value_matches_format() {
        let stamped = prepend_create_time("body");
        let line = stamped
            .lines()
            .find(|l| l.starts_with("create-time: "))
            .expect("create-time line must be present");
        let value = line.trim_start_matches("create-time: ");

        assert!(
            regex::Regex::new(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$")
                .unwrap()
                .is_match(value),
            "create-time value must match YYYY-MM-DD HH:MM:SS, got: {value}"
        );
        assert!(
            !value.contains('Z') && !value.contains('+') && !value.contains("UTC"),
            "create-time must carry no timezone marker, got: {value}"
        );
    }

    #[test]
    fn prepend_create_time_value_is_close_to_now() {
        let before = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let stamped = prepend_create_time("body");
        let after = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let captured = stamped
            .lines()
            .find(|l| l.starts_with("create-time: "))
            .unwrap()
            .trim_start_matches("create-time: ")
            .to_string();

        // Captured timestamp must lie within [before, after]; clock skew
        // outside that window would indicate the helper is reading from a
        // different clock than Local::now().
        assert!(
            captured >= before && captured <= after,
            "create-time {captured} not within [{before}, {after}]"
        );
    }

    #[test]
    fn prepend_create_time_preserves_body_verbatim() {
        let body = "line one\nline two\n\nline four\n";
        let stamped = prepend_create_time(body);
        let (_, tail) = stamped
            .split_once("\n\n")
            .expect("frontmatter must end with blank line");
        // tail begins after the blank line; the original body should follow.
        assert!(
            stamped.ends_with(body),
            "stamped output must end with the original body, got tail: {tail:?}"
        );
    }
}
