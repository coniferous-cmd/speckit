//! Instructions Command
//!
//! Generates enriched instructions for creating artifacts or applying tasks.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::shared::{
    ApplyInstructions, ApplyProgress, ArchiveInstructions, DEFAULT_SCHEMA, TaskItem,
    get_available_changes, print_json, validate_change_exists, validate_schema_exists,
};
use crate::shared_gather::{
    ReferenceIndexEntry, assemble_reference_index, read_project_config, read_registry_snapshot,
};
use crate::shared_output::StoreDiagnostic;

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

/// Options for the apply instructions command.
pub type ApplyInstructionsOptions = InstructionsOptions;

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

    let schema_name = options
        .schema
        .clone()
        .unwrap_or_else(|| DEFAULT_SCHEMA.to_string());
    validate_schema_exists(&schema_name, &project_root)?;

    let id = match artifact_id {
        Some(id) => id.to_string(),
        None => {
            anyhow::bail!(
                "Missing required argument <artifact>. Valid artifacts:\n  proposal\n  specs\n  design\n  tasks"
            );
        }
    };

    let change_dir = Path::new(&project_root)
        .join("speckit")
        .join("changes")
        .join(&change_name);

    // Build instructions based on the artifact id
    let instructions = build_artifact_instructions(
        &id,
        &change_name,
        &schema_name,
        &change_dir.to_string_lossy(),
        &project_root,
    )
    .await?;

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
    change_name: &str,
    schema_name: &str,
    change_dir: &str,
    project_root: &str,
) -> anyhow::Result<ArtifactInstructions> {
    let change_path = Path::new(change_dir);

    let (description, output_path, template, rules, dependencies, unlocks) = match artifact_id {
        "proposal" => (
            "High-level description of the change, its motivation, and scope.",
            "proposal.md",
            include_str!("../../templates/proposal.md"),
            vec![
                "Include a clear 'Why' section explaining motivation".to_string(),
                "List new and modified capabilities".to_string(),
            ],
            vec![],
            vec![
                "specs".to_string(),
                "design".to_string(),
                "tasks".to_string(),
            ],
        ),
        "specs" => (
            "Detailed specifications with requirements and scenarios.",
            "specs/**/*.md",
            include_str!("../../templates/specs.md"),
            vec![
                "Use ## ADDED/MODIFIED/REMOVED Requirements headers".to_string(),
                "Each requirement MUST include at least one #### Scenario: block".to_string(),
            ],
            vec![DependencyInfo {
                id: "proposal".to_string(),
                path: "proposal.md".to_string(),
                description: "The proposal defining what this change is about".to_string(),
                done: change_path.join("proposal.md").exists(),
                skipped: false,
            }],
            vec!["design".to_string(), "tasks".to_string()],
        ),
        "design" => (
            "Technical design decisions and implementation approach.",
            "design.md",
            include_str!("../../templates/design.md"),
            vec![
                "Document goals and non-goals".to_string(),
                "Record key decisions and alternatives considered".to_string(),
            ],
            vec![DependencyInfo {
                id: "specs".to_string(),
                path: "specs/".to_string(),
                description: "The detailed specifications".to_string(),
                done: change_path.join("specs").is_dir(),
                skipped: false,
            }],
            vec!["tasks".to_string()],
        ),
        "tasks" => (
            "Implementation checklist with trackable tasks.",
            "tasks.md",
            include_str!("../../templates/tasks.md"),
            vec![
                "Use - [ ] and - [x] checkbox syntax".to_string(),
                "Each task should be specific and actionable".to_string(),
            ],
            vec![DependencyInfo {
                id: "design".to_string(),
                path: "design.md".to_string(),
                description: "The technical design to implement".to_string(),
                done: change_path.join("design.md").exists(),
                skipped: false,
            }],
            vec![],
        ),
        _ => {
            anyhow::bail!(
                "Artifact '{artifact_id}' not found in schema '{schema_name}'. Valid artifacts:\n  proposal\n  specs\n  design\n  tasks"
            );
        }
    };

    let resolved_output = change_path.join(output_path.trim_end_matches("/**/*"));

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

    let is_blocked = dependencies.iter().any(|d| !d.done);

    Ok(ArtifactInstructions {
        artifact_id: artifact_id.to_string(),
        change_name: change_name.to_string(),
        schema_name: schema_name.to_string(),
        change_dir: change_dir.to_string(),
        resolved_output_path: resolved_output.to_string_lossy().to_string(),
        description: description.to_string(),
        instruction: if is_blocked {
            "Complete the dependencies above before creating this artifact.".to_string()
        } else {
            format!("Create the {artifact_id} artifact for change \"{change_name}\".")
        },
        context,
        rules,
        template: prepend_create_time(template),
        dependencies,
        unlocks,
        references,
        skipped: false,
        warning: None,
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
// Apply Instructions Command
// -----------------------------------------------------------------------------

/// Execute the apply instructions command.
pub async fn apply_instructions_command(options: ApplyInstructionsOptions) -> anyhow::Result<()> {
    let project_root = super::resolve_project_root(options.store.as_deref()).await?;

    let change_name = validate_change_exists(
        options.change.as_deref(),
        &project_root,
        None,
        Some("speckit new change <name>"),
    )
    .await?;

    let schema_name = options
        .schema
        .clone()
        .unwrap_or_else(|| DEFAULT_SCHEMA.to_string());
    validate_schema_exists(&schema_name, &project_root)?;

    let instructions =
        generate_apply_instructions(&project_root, &change_name, Some(&schema_name)).await?;

    if options.json {
        print_json(&instructions);
        return Ok(());
    }

    print_apply_instructions_text(&instructions);
    Ok(())
}

/// Generate apply instructions for implementing tasks from a change.
pub async fn generate_apply_instructions(
    project_root: &str,
    change_name: &str,
    schema_name: Option<&str>,
) -> anyhow::Result<ApplyInstructions> {
    let schema = schema_name.unwrap_or(DEFAULT_SCHEMA);
    let change_dir = Path::new(project_root)
        .join("speckit")
        .join("changes")
        .join(change_name);

    let tasks_path = change_dir.join("tasks.md");

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

    // Build context files
    let mut context_files: HashMap<String, Vec<String>> = HashMap::new();
    let proposal = change_dir.join("proposal.md");
    if proposal.exists() {
        context_files.insert(
            "proposal".to_string(),
            vec![proposal.to_string_lossy().to_string()],
        );
    }
    let specs_dir = change_dir.join("specs");
    if specs_dir.is_dir() {
        if let Ok(mut rd) = std::fs::read_dir(&specs_dir) {
            let specs: Vec<String> = rd
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
                .map(|e| e.path().to_string_lossy().to_string())
                .collect();
            if !specs.is_empty() {
                context_files.insert("specs".to_string(), specs);
            }
        }
    }
    let design = change_dir.join("design.md");
    if design.exists() {
        context_files.insert(
            "design".to_string(),
            vec![design.to_string_lossy().to_string()],
        );
    }

    // Determine state
    let (state, instruction) = if !tasks_path.exists() {
        (
            "blocked".to_string(),
            "The tasks.md file is missing and must be created.\nUse speckit-continue-change to generate the tracking file.".to_string(),
        )
    } else if tasks.is_empty() {
        (
            "blocked".to_string(),
            "The tasks.md file exists but contains no tasks to work on.\nAdd tasks to tasks.md or regenerate it with speckit-continue-change.".to_string(),
        )
    } else if remaining == 0 && total > 0 {
        (
            "all_done".to_string(),
            "All tasks are complete! This change is ready to be archived.\nConsider running tests and reviewing the changes before archiving.".to_string(),
        )
    } else {
        (
            "ready".to_string(),
            "Read context files, work through pending tasks, mark complete as you go.\nPause if you hit blockers or need clarification.".to_string(),
        )
    };

    // Load references
    let registry = read_registry_snapshot().await;
    let project_config = read_project_config(project_root);
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

    Ok(ApplyInstructions {
        change_name: change_name.to_string(),
        change_dir: change_dir.to_string_lossy().to_string(),
        schema_name: schema.to_string(),
        context_files,
        progress: ApplyProgress {
            total,
            complete,
            remaining,
        },
        tasks,
        state,
        missing_artifacts: None,
        instruction,
        references,
        context: None,
        operation_guidance: None,
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

/// Print apply instructions in human-readable format.
fn print_apply_instructions_text(instructions: &ApplyInstructions) {
    println!("## Apply: {}", instructions.change_name);
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

    let instructions = generate_archive_instructions(&change_name);

    if options.json {
        print_json(&instructions);
        return Ok(());
    }

    print_archive_instructions_text(&instructions);
    Ok(())
}

/// Generate archive instructions.
pub fn generate_archive_instructions(change_name: &str) -> ArchiveInstructions {
    ArchiveInstructions {
        change_name: change_name.to_string(),
        context: None,
        operation_guidance: None,
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
    use super::prepend_create_time;
    use chrono::Local;

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
