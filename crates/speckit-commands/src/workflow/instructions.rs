//! Instructions Command
//!
//! Generates enriched instructions for creating artifacts or applying tasks.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use speckit_core::project_config::{ProjectConfig, load_operation_inputs, validate_config_rules};

use super::shared::{
    ArchiveInstructions, ImplementInstructions, ImplementProgress, TaskItem,
    UnavailableTrackingFile, print_json, validate_change_exists,
};
use crate::shared_gather::{
    ReferenceIndexEntry, assemble_reference_index, read_project_config, read_registry_snapshot,
};
use speckit_core::artifact_graph::{
    ChangeContext, LoadChangeContextOptions, generate_instructions as generate_core_instructions,
    load_change_context, resolve_artifact_outputs,
};
use speckit_core::change_metadata::METADATA_FILENAME;
use speckit_core::references::{escape_envelope_attribute, escape_envelope_tags};
use speckit_core::root_selection::{ResolvedSpeckitRoot, to_root_output, with_store_flag};
use speckit_core::utils::spec_discovery::find_misplaced_delta_specs;
use speckit_core::utils::task_progress::parse_task_lines;

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
    /// Per-artifact guidance from the schema. Absent when the schema declares
    /// none: the field is deliberately not filled with boilerplate, because a
    /// synthetic sentence would tell the agent "the schema has said everything"
    /// when it has said nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    /// Project background from `context:` in the project config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Artifact-specific rules from config (constraints for AI, not to be included in output).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<String>>,
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
    // Resolved here rather than through `resolve_project_root` so the store id
    // survives: the JSON payload carries `planningHome`, whose `root` is what
    // the schema instructions tell the agent to use instead of a repo-relative
    // path. Same options and same implicit-root policy as before.
    let root = crate::change::resolve_command_root(options.store.as_deref(), true)?;
    let project_root = root.path.to_string_lossy().into_owned();

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
            // Listing the valid ids turns a guessed artifact into a one-shot
            // correction instead of a retry loop.
            let context = load_change_context(
                Path::new(&project_root),
                &change_name,
                options.schema.as_deref(),
                LoadChangeContextOptions { change_dir: None },
            )
            .map_err(|error| anyhow::anyhow!("Unable to load change instructions: {error}"))?;
            let valid: Vec<String> = context
                .graph
                .get_all_artifacts()
                .into_iter()
                .map(|artifact| artifact.id.clone())
                .collect();
            anyhow::bail!(
                "Missing required argument <artifact>. Valid artifacts:\n  {}",
                valid.join("\n  ")
            );
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
        // `root` and `planningHome` join the payload because the artifact
        // instructions point the agent at `planningHome.root` for store-aware
        // reads of the main specs; emitting that instruction without the field
        // would send the agent looking for a value only `status` carried.
        let mut payload = serde_json::to_value(&instructions)?;
        let object = payload
            .as_object_mut()
            .expect("artifact instructions serialize to an object");
        let root_path = project_root.replace('\\', "/");
        let default_schema = instructions.schema_name.clone();
        object.insert(
            "planningHome".to_string(),
            serde_json::json!({
                "kind": if root.store_id.is_some() { "store" } else { "repo" },
                "root": root_path,
                "changesDir": format!("{root_path}/speckit/changes"),
                "defaultSchema": default_schema,
            }),
        );
        object.insert(
            "root".to_string(),
            serde_json::to_value(to_root_output(&root))?,
        );
        print_json(&payload);
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
    // Load project context
    let project_config = read_project_config(project_root);
    let core_project_config =
        speckit_core::project_config::read_project_config(Path::new(project_root));
    let core_artifact_id = core_instructions.artifact_id.clone();
    // Mirrors the OpenSpec side: the project's own `context:` text is the
    // project context, and an empty or whitespace-only value is absence rather
    // than an empty block. Synthesizing "Project root: <path>" here would put a
    // path the payload already carries into the slot reserved for the user's
    // background, and would silently displace whatever they actually wrote.
    let context = core_project_config
        .as_ref()
        .and_then(|config| config.context.as_deref())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);

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
        artifact_id: core_artifact_id.clone(),
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
        // A blocked artifact keeps its real guidance: the `<warning>` above
        // already says what is missing, and replacing the instruction would
        // cost the agent the schema's own rules for the artifact it is about to
        // write anyway.
        instruction: core_instructions.instruction,
        context,
        rules: extract_rules_for_artifact(
            core_project_config.as_ref(),
            &core_artifact_id,
            project_root,
        ),
        template: prepend_create_time(&core_instructions.template),
        dependencies,
        unlocks: core_instructions.unlocks,
        references,
        skipped: core_instructions.skipped.unwrap_or(false),
        warning: core_instructions.warning,
    })
}

/// Session-level cache for `validate_config_rules` warnings, so repeated calls
/// within one invocation do not spam the user with the same unknown-artifact-id
/// notice. Mirrors `shownWarnings` in the OpenSpec implementation.
fn shown_rules_warnings() -> &'static Mutex<HashSet<String>> {
    static CACHE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Collect the artifact-specific rules from the project config, mirroring the
/// OpenSpec `generateInstructions` filter: only rules whose key matches the
/// requested artifact ID are returned. Any artifact ID listed under `rules:`
/// that matches no artifact in any available schema triggers a session-deduped
/// warning so a typo is visible without flooding output when many artifacts are
/// queried in sequence.
///
/// Returns `None` when the config has no rules for this artifact or the
/// resolved list is empty — `None` keeps the field out of the JSON output so
/// consumers can treat absence and "no rules" identically.
fn extract_rules_for_artifact(
    project_config: Option<&ProjectConfig>,
    artifact_id: &str,
    project_root: &str,
) -> Option<Vec<String>> {
    let rules_map = project_config.and_then(|c| c.rules.as_ref())?;

    // Validate artifact IDs once per session. The rules map is global while
    // each change can use a different schema, so a key is "unknown" only when
    // it matches no artifact in any available schema — matching the OpenSpec
    // behavior at instruction-loader.ts:364-376.
    let valid_artifact_ids: HashSet<String> = super::schemas::list_schemas_with_info(project_root)
        .into_iter()
        .flat_map(|schema| schema.artifacts.into_iter())
        .collect();
    let warnings = validate_config_rules(rules_map, &valid_artifact_ids);
    if !warnings.is_empty() {
        let mut cache = shown_rules_warnings()
            .lock()
            .expect("rules-warnings cache poisoned");
        for warning in warnings {
            if cache.insert(warning.clone()) {
                eprintln!("{warning}");
            }
        }
    }

    let for_artifact = rules_map.get(artifact_id)?;
    if for_artifact.is_empty() {
        None
    } else {
        Some(for_artifact.clone())
    }
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
        escape_envelope_attribute(&instructions.artifact_id),
        escape_envelope_attribute(&instructions.change_name),
        escape_envelope_attribute(&instructions.schema_name)
    );
    println!();

    if instructions.skipped {
        let default_warning =
            format!("This artifact is skipped (skip_specs is set in {METADATA_FILENAME}).");
        println!("<warning>");
        println!(
            "{}",
            escape_envelope_tags(instructions.warning.as_deref().unwrap_or(&default_warning))
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
        escape_envelope_tags(&instructions.artifact_id),
        escape_envelope_tags(&instructions.change_name)
    );
    println!("{}", escape_envelope_tags(&instructions.description));
    println!("</task>");
    println!();

    if let Some(ref ctx) = instructions.context {
        println!("<project_context>");
        println!(
            "<!-- This is background information for you. Do NOT include this in your output. -->"
        );
        println!("{}", escape_envelope_tags(ctx));
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

    if let Some(ref rules) = instructions.rules {
        if !rules.is_empty() {
            println!("<rules>");
            println!(
                "<!-- These are constraints for you to follow. Do NOT include this in your output. -->"
            );
            for rule in rules {
                // Flattened so a newline cannot forge a sibling bullet, but
                // never truncated: these are instructions an agent has to
                // follow in full.
                println!(
                    "- {}",
                    escape_envelope_tags(&speckit_core::references::sanitize_inline(
                        rule,
                        usize::MAX
                    ))
                );
            }
            println!("</rules>");
            println!();
        }
    }

    if !instructions.dependencies.is_empty() {
        println!("<dependencies>");
        println!(
            "Read the current contents of these files before creating this artifact \
             (re-read them from disk even if you saw them earlier - they may have been edited):"
        );
        println!();
        for dep in &instructions.dependencies {
            // A dependency satisfied via skip_specs has no files by design:
            // telling the agent to read them (or calling them "done") would
            // send it hunting for spec files that must not exist.
            if dep.skipped {
                println!(
                    "<dependency id=\"{}\" status=\"skipped\">",
                    escape_envelope_attribute(&dep.id)
                );
                println!(
                    "  <description>Skipped: the change declares skip_specs, so this artifact has no files to read.</description>"
                );
                println!("</dependency>");
                continue;
            }
            let status = if dep.done { "done" } else { "missing" };
            let full_path = Path::new(&instructions.change_dir).join(&dep.path);
            println!(
                "<dependency id=\"{}\" status=\"{}\">",
                escape_envelope_attribute(&dep.id),
                status
            );
            println!(
                "  <path>{}</path>",
                escape_envelope_attribute(&full_path.display().to_string())
            );
            println!(
                "  <description>{}</description>",
                escape_envelope_attribute(&dep.description)
            );
            println!("</dependency>");
        }
        println!("</dependencies>");
        println!();
    }

    println!("<output>");
    println!(
        "Write to: {}",
        escape_envelope_attribute(&instructions.resolved_output_path)
    );
    println!("</output>");
    println!();

    if let Some(ref instruction) = instructions.instruction {
        if !instruction.trim().is_empty() {
            println!("<instruction>");
            println!("{}", escape_envelope_tags(instruction.trim()));
            println!("</instruction>");
            println!();
        }
    }

    println!("<template>");
    println!("<!-- Use this as the structure for your output file. Fill in the sections. -->");
    // Copied verbatim into the artifact file, so its `<!-- ... -->` comments and
    // `<placeholder>` markers must survive - only the envelope's own closing
    // tags are neutralized.
    println!("{}", escape_envelope_tags(instructions.template.trim()));
    println!("</template>");
    println!();

    println!("<success_criteria>");
    println!("<!-- To be defined in schema validation rules -->");
    println!("</success_criteria>");
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
    let root = crate::change::resolve_command_root(options.store.as_deref(), false)?;
    let project_root = root.path.to_string_lossy();
    let new_change_hint = with_store_flag(&root, "speckit new change <name>");

    reject_removed_operation_guidance(&project_root)?;

    let change_name = validate_change_exists(
        options.change.as_deref(),
        &project_root,
        Some(&root.changes_dir.to_string_lossy()),
        Some(&new_change_hint),
    )
    .await?;

    let instructions =
        generate_implement_instructions(&root, &change_name, options.schema.as_deref()).await?;

    if options.json {
        let mut payload = serde_json::to_value(&instructions)?;
        payload["root"] = serde_json::to_value(to_root_output(&root))?;
        print_json(&payload);
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
    root: &ResolvedSpeckitRoot,
    change_name: &str,
    schema_name: Option<&str>,
) -> anyhow::Result<ImplementInstructions> {
    let change_dir = root.changes_dir.join(change_name);
    let project_root = root.path.to_string_lossy();

    let change_context = load_change_context(
        &root.path,
        change_name,
        schema_name,
        LoadChangeContextOptions {
            change_dir: Some(change_dir.clone()),
        },
    )
    .map_err(|error| anyhow::anyhow!("Unable to load change context: {error}"))?;
    let schema = change_context.graph.schema();
    let implement_phase = schema.implement.as_ref();
    let tracking = implement_phase.and_then(|phase| phase.tracks.as_deref());
    let tracking_files = tracking
        .map(|pattern| resolve_artifact_outputs(&change_dir, pattern))
        .unwrap_or_default();
    let required_artifacts = schema.implement_requires();
    let missing_artifacts: Vec<String> = required_artifacts
        .iter()
        .filter(|artifact_id| !change_context.completed.contains(*artifact_id))
        .cloned()
        .collect();
    let missing_prerequisites = collect_missing_prerequisites(&change_context, &required_artifacts);

    let mut tasks = Vec::new();
    let mut total = 0;
    let mut complete = 0;
    let mut unavailable_tracking_files = Vec::new();
    for file in &tracking_files {
        let source_path = file.to_string_lossy().replace('\\', "/");
        match tokio::fs::read_to_string(file).await {
            Ok(content) => {
                for task in parse_task_lines(&content) {
                    total += 1;
                    complete += usize::from(task.done);
                    if !task.description.is_empty() {
                        tasks.push(TaskItem {
                            id: (tasks.len() + 1).to_string(),
                            description: task.description,
                            done: task.done,
                            source_path: source_path.clone(),
                            line: task.line,
                        });
                    }
                }
            }
            Err(error) => unavailable_tracking_files.push(UnavailableTrackingFile {
                path: source_path,
                reason: error.to_string(),
            }),
        }
    }
    let remaining = total - complete;

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
    let remedy = |artifact: &str| {
        let instructions = with_store_flag(
            root,
            &format!("speckit instructions {artifact} --change {change_name}"),
        );
        let status = with_store_flag(root, &format!("speckit status --change {change_name}"));
        format!(
            "Use `{instructions}` to create or repair the artifact. `{status}` shows what's left."
        )
    };
    let (state, mut instruction) = if !missing_artifacts.is_empty() {
        let mut message = format!(
            "Cannot implement this change yet. Missing artifacts: {}.",
            missing_artifacts.join(", ")
        );
        if missing_prerequisites.len() > missing_artifacts.len() {
            message.push_str(&format!(
                "\nNot created yet, in build order: {}. Build the ones this change needs and check the schema for conditional artifacts.",
                missing_prerequisites.join(", ")
            ));
        }
        let artifact = match missing_prerequisites.as_slice() {
            [only] => only,
            _ => "<artifact>",
        };
        ("blocked", format!("{message}\n{}", remedy(artifact)))
    } else if let Some(tracking) = tracking
        && tasks.is_empty()
    {
        if !unavailable_tracking_files.is_empty() {
            (
                "blocked",
                "No readable task descriptions are available.".to_owned(),
            )
        } else {
            let message = if tracking_files.is_empty() {
                format!(
                    "The tracking file or pattern {tracking} has no matching file. Create a tracking file before implementing."
                )
            } else {
                format!(
                    "The tracking file or pattern {tracking} contains no tasks to work on. Add tasks before implementing."
                )
            };
            let recovery = schema.artifacts.iter().find(|artifact| artifact.generates == tracking)
                .map(|artifact| remedy(&artifact.id))
                .unwrap_or_else(|| format!("Use the existing planning artifacts to create or repair the tracking file at {tracking}."));
            ("blocked", format!("{message}\n{recovery}"))
        }
    } else if tracking.is_some() && unavailable_tracking_files.is_empty() && remaining == 0 {
        (
            "all_done",
            "All tracked tasks are complete.\nReview or verify the change before archiving."
                .to_owned(),
        )
    } else {
        (
            "ready",
            implement_phase
                .and_then(|phase| phase.instruction.as_deref())
                .map(str::trim)
                .unwrap_or(if tracking.is_some() {
                    "Read context files, work through pending tasks, mark complete as you go.\nPause if you hit blockers or need clarification."
                } else {
                    "All required artifacts complete. Proceed with implementation."
                })
                .to_owned(),
        )
    };
    if !unavailable_tracking_files.is_empty() {
        instruction.push_str("\nTask completion could not be verified for these tracking files:");
        for file in &unavailable_tracking_files {
            instruction.push_str(&format!("\n- {}: {}", file.path, file.reason));
        }
    }
    let warnings = if state == "blocked" {
        Vec::new()
    } else {
        implement_spec_warnings(&change_context, &context_files, root)?
    };

    // Load references
    let registry = read_registry_snapshot().await;
    let project_config = read_project_config(&project_root);
    let core_project_config = speckit_core::project_config::read_project_config(&root.path);
    let operation_inputs = load_operation_inputs(core_project_config.as_ref(), "implement");
    let references_decl = project_config
        .as_ref()
        .map(|c| c.references.clone())
        .unwrap_or_default();
    let references = if references_decl.is_empty() {
        None
    } else {
        let idx =
            assemble_reference_index(&references_decl, &registry.entries, &project_root).await;
        if idx.is_empty() { None } else { Some(idx) }
    };

    Ok(ImplementInstructions {
        change_name: change_name.to_string(),
        change_dir: change_dir.to_string_lossy().replace('\\', "/"),
        schema_name: change_context.schema_name,
        context_files,
        progress: ImplementProgress {
            total,
            complete,
            remaining,
        },
        tasks,
        task_tracking_configured: tracking.is_some(),
        unavailable_tracking_files,
        state: state.to_owned(),
        missing_artifacts: if missing_artifacts.is_empty() {
            None
        } else {
            Some(missing_artifacts)
        },
        missing_prerequisites,
        warnings,
        instruction,
        references,
        context: operation_inputs.context,
        operation_guidance: operation_inputs.operation_guidance,
    })
}

fn collect_missing_prerequisites(context: &ChangeContext, required: &[String]) -> Vec<String> {
    let mut pending: Vec<&str> = required.iter().map(String::as_str).collect();
    let mut visited = HashSet::new();
    while let Some(id) = pending.pop() {
        if visited.insert(id)
            && let Some(artifact) = context.graph.get_artifact(id)
        {
            pending.extend(artifact.requires.iter().map(String::as_str));
        }
    }
    context
        .graph
        .get_build_order()
        .into_iter()
        .filter(|id| visited.contains(id.as_str()) && !context.completed.contains(id))
        .collect()
}

fn implement_spec_warnings(
    context: &ChangeContext,
    context_files: &HashMap<String, Vec<String>>,
    root: &ResolvedSpeckitRoot,
) -> anyhow::Result<Vec<String>> {
    let spec_artifacts = context
        .graph
        .schema()
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact
                .generates
                .trim_start_matches("./")
                .starts_with("specs/")
        })
        .collect::<Vec<_>>();
    if spec_artifacts.is_empty()
        || context.skipped_artifacts.as_ref().is_some_and(|skipped| {
            spec_artifacts
                .iter()
                .any(|artifact| skipped.contains(&artifact.id))
        })
    {
        return Ok(Vec::new());
    }

    let mut warnings = Vec::new();
    let change_name = &context.change_name;
    let validate = with_store_flag(root, &format!("speckit validate {change_name}"));
    for relative in find_misplaced_delta_specs(&context.change_dir)? {
        let capability = relative
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| relative.with_extension(""));
        warnings.push(format!(
            "`specs/{}` contains delta requirements outside a capability's spec.md. Move them into `specs/{}/spec.md` so {validate} and archive can read them.",
            relative.display(),
            capability.display(),
        ));
    }

    if !spec_artifacts
        .iter()
        .any(|artifact| context_files.contains_key(&artifact.id))
    {
        let target = match spec_artifacts.as_slice() {
            [artifact] => artifact.id.as_str(),
            _ => "<artifact-id>",
        };
        let instructions = with_store_flag(
            root,
            &format!("speckit instructions {target} --change {change_name}"),
        );
        warnings.push(format!(
            "This change has no delta specs and does not declare `skip_specs: true`, so `{validate}` fails on it. Write the delta specs (`{instructions}`), or set `skip_specs: true` in {METADATA_FILENAME} if no specified behavior changes."
        ));
    }
    Ok(warnings)
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
        println!("### Blocked");
        if let Some(missing) = &instructions.missing_artifacts {
            println!("Missing artifacts: {}", missing.join(", "));
        }
        println!();
    }

    if !instructions.warnings.is_empty() {
        println!("### Warnings");
        for warning in &instructions.warnings {
            println!("- {warning}");
        }
        println!();
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

    use super::{extract_rules_for_artifact, generate_archive_instructions, prepend_create_time};
    use chrono::Local;
    use speckit_core::project_config::{OperationConfig, ProjectConfig};

    /// Build a minimal `ProjectConfig` with only the rules field populated, for
    /// exercising `extract_rules_for_artifact` in isolation.
    fn config_with_rules(rules: HashMap<String, Vec<String>>) -> ProjectConfig {
        ProjectConfig {
            schema: "spec-driven".to_string(),
            context: None,
            rules: Some(rules),
            operations: None,
            store: None,
            github_copilot: None,
            references: None,
        }
    }

    #[test]
    fn extract_rules_returns_matching_artifact_rules() {
        let mut rules = HashMap::new();
        rules.insert(
            "proposal".to_string(),
            vec!["Every section starts with Why".to_string()],
        );
        rules.insert("specs".to_string(), vec!["Include scenarios".to_string()]);

        let config = config_with_rules(rules);

        let result =
            extract_rules_for_artifact(Some(&config), "proposal", "/nonexistent-speckit-root");

        assert_eq!(
            result,
            Some(vec!["Every section starts with Why".to_string()])
        );
    }

    #[test]
    fn extract_rules_returns_none_when_artifact_id_not_listed() {
        let mut rules = HashMap::new();
        rules.insert(
            "proposal".to_string(),
            vec!["Every section starts with Why".to_string()],
        );

        let config = config_with_rules(rules);

        let result =
            extract_rules_for_artifact(Some(&config), "design", "/nonexistent-speckit-root");

        assert!(result.is_none());
    }

    #[test]
    fn extract_rules_returns_none_without_project_config() {
        let result = extract_rules_for_artifact(None, "proposal", "/nonexistent-speckit-root");
        assert!(result.is_none());
    }

    #[test]
    fn extract_rules_returns_none_when_config_has_no_rules_block() {
        let config = ProjectConfig {
            schema: "spec-driven".to_string(),
            context: None,
            rules: None,
            operations: None,
            store: None,
            github_copilot: None,
            references: None,
        };

        let result =
            extract_rules_for_artifact(Some(&config), "proposal", "/nonexistent-speckit-root");

        assert!(result.is_none());
    }

    #[test]
    fn extract_rules_returns_none_when_matching_key_has_empty_vec() {
        let mut rules = HashMap::new();
        rules.insert("proposal".to_string(), Vec::new());

        let config = config_with_rules(rules);

        let result =
            extract_rules_for_artifact(Some(&config), "proposal", "/nonexistent-speckit-root");

        assert!(
            result.is_none(),
            "empty rules vec must collapse to None so the JSON field is omitted"
        );
    }

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
