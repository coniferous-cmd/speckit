use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use super::graph::ArtifactGraph;
use super::outputs::{resolve_artifact_output_path, resolve_artifact_outputs};
use super::resolver::{get_schema_dir, resolve_schema};
use super::state::detect_completed;
use super::types::{Artifact, CompletedSet};

/// Error thrown when loading a template file fails.
#[derive(Debug, Error)]
pub enum TemplateLoadError {
    #[error("Schema '{schema}' not found")]
    SchemaNotFound { schema: String },

    #[error("Template not found: {path}")]
    NotFound { path: String },

    #[error("Failed to read template: {message}")]
    ReadError { message: String },
}

/// Warning attached to instructions for an artifact skipped via skip_specs.
pub const SKIP_SPECS_INSTRUCTIONS_WARNING: &str =
    "This change declares skip_specs: true in .speckit.yaml (no spec-level behavior changes), \
     so this artifact is skipped.\n\
     Do not create spec files - they will conflict with that marker. If requirements now change, \
     remove skip_specs from .speckit.yaml and rerun this command.";

/// Change context combining graph, completion state, and metadata.
#[derive(Debug, Clone)]
pub struct ChangeContext {
    /// The artifact dependency graph.
    pub graph: ArtifactGraph,
    /// Set of completed artifact IDs.
    pub completed: CompletedSet,
    /// Schema name being used.
    pub schema_name: String,
    /// Change name.
    pub change_name: String,
    /// Path to the change directory.
    pub change_dir: PathBuf,
    /// Project root directory.
    pub project_root: PathBuf,
    /// Artifact IDs counted as complete only because the change declares
    /// skip_specs, not because their files exist.
    pub skipped_artifacts: Option<CompletedSet>,
}

/// Options for loading a change context.
#[derive(Debug, Default)]
pub struct LoadChangeContextOptions {
    pub change_dir: Option<PathBuf>,
}

/// Dependency information including path and description.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyInfo {
    /// Artifact ID.
    pub id: String,
    /// Whether the dependency is completed.
    pub done: bool,
    /// Relative output path of the dependency.
    pub path: String,
    /// Description of the dependency artifact.
    pub description: String,
    /// True when the dependency is satisfied via skip_specs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<bool>,
}

/// Enriched instructions for creating an artifact.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactInstructions {
    /// Change name.
    pub change_name: String,
    /// Artifact ID.
    pub artifact_id: String,
    /// Schema name.
    pub schema_name: String,
    /// Full path to change directory.
    pub change_dir: PathBuf,
    /// Output path pattern (e.g., "proposal.md").
    pub output_path: String,
    /// Absolute output path or glob pattern resolved under the change directory.
    pub resolved_output_path: PathBuf,
    /// Existing concrete output files for this artifact.
    pub existing_output_paths: Vec<PathBuf>,
    /// Artifact description.
    pub description: String,
    /// Guidance on how to create this artifact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    /// Template content (structure to follow).
    pub template: String,
    /// Dependencies with completion status and paths.
    pub dependencies: Vec<DependencyInfo>,
    /// Artifacts that become available after completing this one.
    pub unlocks: Vec<String>,
    /// True when skipped via skip_specs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<bool>,
    /// Present only when skipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// Status of a single artifact in the workflow.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactStatus {
    /// Artifact ID.
    pub id: String,
    /// Output path pattern.
    pub output_path: String,
    /// Status: done, skipped, ready, or blocked.
    pub status: ArtifactStatusKind,
    /// Artifact IDs this artifact directly requires.
    pub requires: Vec<String>,
    /// Missing dependencies (only for blocked).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_deps: Option<Vec<String>>,
}

/// The status kind of an artifact.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactStatusKind {
    Done,
    Skipped,
    Ready,
    Blocked,
}

/// Absolute artifact path details.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactPathSummary {
    pub output_path: String,
    pub resolved_output_path: PathBuf,
    pub existing_output_paths: Vec<PathBuf>,
}

/// Formatted change status.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeStatus {
    /// Change name.
    pub change_name: String,
    /// Schema name.
    pub schema_name: String,
    /// Full path to the change root.
    pub change_root: PathBuf,
    /// Absolute artifact path details keyed by artifact ID.
    pub artifact_paths: HashMap<String, ArtifactPathSummary>,
    /// Whether all planning artifacts are complete.
    pub is_planning_complete: bool,
    /// Compatibility alias for is_planning_complete.
    pub is_complete: bool,
    /// Artifact IDs required before apply phase.
    pub apply_requires: Vec<String>,
    /// Status of each artifact.
    pub artifacts: Vec<ArtifactStatus>,
}

/// Loads a template from a schema's templates directory.
pub fn load_template(
    schema_name: &str,
    template_path: &str,
    project_root: Option<&Path>,
) -> Result<String, TemplateLoadError> {
    let schema_dir =
        get_schema_dir(schema_name, project_root).ok_or_else(|| TemplateLoadError::SchemaNotFound {
            schema: schema_name.to_string(),
        })?;

    let templates_dir = schema_dir.join("templates");
    let template_full_path = templates_dir.join(template_path);

    if !template_full_path.exists() {
        return Err(TemplateLoadError::NotFound {
            path: template_full_path.display().to_string(),
        });
    }

    fs::read_to_string(&template_full_path).map_err(|e| TemplateLoadError::ReadError {
        message: e.to_string(),
    })
}

/// Loads change context combining graph and completion state.
///
/// Schema resolution order:
/// 1. Explicit `schema_name` parameter (if provided)
/// 2. Schema from metadata (if exists in change directory)
/// 3. Default "spec-driven"
pub fn load_change_context(
    project_root: &Path,
    change_name: &str,
    schema_name: Option<&str>,
    options: LoadChangeContextOptions,
) -> Result<ChangeContext, Box<dyn std::error::Error>> {
    let change_dir = options
        .change_dir
        .unwrap_or_else(|| project_root.join("speckit").join("changes").join(change_name));

    // Schema resolution: explicit > metadata > default
    let resolved_schema_name = schema_name
        .map(|s| s.to_string())
        .unwrap_or_else(|| "spec-driven".to_string());

    let schema = resolve_schema(&resolved_schema_name, Some(project_root))?;
    let graph = ArtifactGraph::from_schema(schema);
    let completed = detect_completed(&graph, &change_dir);

    // Handle skip_specs: artifacts generating into specs/ count as complete
    let skipped_artifacts = apply_skip_specs(&graph, &completed, &change_dir);

    let mut completed = completed;
    if let Some(ref skipped) = skipped_artifacts {
        for id in skipped {
            completed.insert(id.clone());
        }
    }

    Ok(ChangeContext {
        graph,
        completed,
        schema_name: resolved_schema_name,
        change_name: change_name.to_string(),
        change_dir,
        project_root: project_root.to_path_buf(),
        skipped_artifacts: if skipped_artifacts.as_ref().map_or(false, |s| !s.is_empty()) {
            skipped_artifacts
        } else {
            None
        },
    })
}

/// Applies skip_specs logic: artifacts generating into specs/ count as
/// complete when skip_specs is declared in the change's `.speckit.yaml`.
///
/// Mirrors the TypeScript logic: any artifact whose `generates` starts with
/// `specs/` (after stripping leading `./`) is added to both the completed set
/// and the skipped set so that downstream dependents (e.g. tasks) are not
/// blocked on files that must not exist.
fn apply_skip_specs(
    graph: &ArtifactGraph,
    completed: &CompletedSet,
    change_dir: &Path,
) -> Option<CompletedSet> {
    let skip_specs = crate::change_metadata::read_skip_specs_marker(change_dir).unwrap_or(false);
    if !skip_specs {
        return None;
    }

    let mut skipped = CompletedSet::new();
    for artifact in graph.get_all_artifacts() {
        // Strip leading `./` to match globs that normalize the path.
        let generates = artifact.generates.strip_prefix("./").unwrap_or(&artifact.generates);
        if generates.starts_with("specs/") && !completed.contains(&artifact.id) {
            skipped.insert(artifact.id.clone());
        }
    }

    if skipped.is_empty() {
        None
    } else {
        Some(skipped)
    }
}

/// Generates enriched instructions for creating an artifact.
pub fn generate_instructions(
    context: &ChangeContext,
    artifact_id: &str,
) -> Result<ArtifactInstructions, Box<dyn std::error::Error>> {
    let artifact = context
        .graph
        .get_artifact(artifact_id)
        .ok_or_else(|| format!("Artifact '{}' not found in schema '{}'", artifact_id, context.schema_name))?;

    let template_content = load_template(&context.schema_name, &artifact.template, Some(&context.project_root))?;
    let dependencies = get_dependency_info(artifact, &context.graph, &context.completed, context.skipped_artifacts.as_ref());
    let unlocks = get_unlocked_artifacts(&context.graph, artifact_id);

    let resolved_output_path = resolve_artifact_output_path(&context.change_dir, &artifact.generates);
    let existing_output_paths = resolve_artifact_outputs(&context.change_dir, &artifact.generates);

    let skipped = context
        .skipped_artifacts
        .as_ref()
        .and_then(|s| if s.contains(artifact_id) { Some(true) } else { None });
    let warning = if skipped.is_some() {
        Some(SKIP_SPECS_INSTRUCTIONS_WARNING.to_string())
    } else {
        None
    };

    Ok(ArtifactInstructions {
        change_name: context.change_name.clone(),
        artifact_id: artifact.id.clone(),
        schema_name: context.schema_name.clone(),
        change_dir: context.change_dir.clone(),
        output_path: artifact.generates.clone(),
        resolved_output_path,
        existing_output_paths,
        description: artifact.description.clone(),
        instruction: artifact.instruction.clone(),
        template: template_content,
        dependencies,
        unlocks,
        skipped,
        warning,
    })
}

/// Gets dependency info including paths and descriptions.
fn get_dependency_info(
    artifact: &Artifact,
    graph: &ArtifactGraph,
    completed: &CompletedSet,
    skipped_artifacts: Option<&CompletedSet>,
) -> Vec<DependencyInfo> {
    artifact
        .requires
        .iter()
        .map(|id| {
            let dep_artifact = graph.get_artifact(id);
            let skipped = skipped_artifacts
                .and_then(|s| if s.contains(id.as_str()) { Some(true) } else { None });
            DependencyInfo {
                id: id.clone(),
                done: completed.contains(id.as_str()),
                path: dep_artifact
                    .map(|a| a.generates.clone())
                    .unwrap_or_else(|| id.clone()),
                description: dep_artifact
                    .map(|a| a.description.clone())
                    .unwrap_or_default(),
                skipped,
            }
        })
        .collect()
}

/// Gets artifacts that become available after completing the given artifact.
fn get_unlocked_artifacts(graph: &ArtifactGraph, artifact_id: &str) -> Vec<String> {
    graph
        .get_all_artifacts()
        .into_iter()
        .filter(|a| a.requires.iter().any(|r| r == artifact_id))
        .map(|a| a.id.clone())
        .collect()
}

/// Formats the status of all artifacts in a change.
pub fn format_change_status(context: &ChangeContext) -> ChangeStatus {
    let schema = resolve_schema(&context.schema_name, Some(&context.project_root));
    let apply_requires = schema
        .as_ref()
        .ok()
        .and_then(|s| s.apply.as_ref().map(|a| a.requires.clone()))
        .unwrap_or_else(|| {
            context
                .graph
                .get_all_artifacts()
                .into_iter()
                .map(|a| a.id.clone())
                .collect()
        });

    let artifacts = context.graph.get_all_artifacts();
    let ready: std::collections::HashSet<String> = context
        .graph
        .get_next_artifacts(&context.completed)
        .into_iter()
        .collect();
    let blocked = context.graph.get_blocked(&context.completed);

    let mut artifact_paths = HashMap::new();
    let mut artifact_statuses: Vec<ArtifactStatus> = artifacts
        .iter()
        .map(|artifact| {
            let resolved = resolve_artifact_output_path(&context.change_dir, &artifact.generates);
            let existing = resolve_artifact_outputs(&context.change_dir, &artifact.generates);

            artifact_paths.insert(
                artifact.id.clone(),
                ArtifactPathSummary {
                    output_path: artifact.generates.clone(),
                    resolved_output_path: resolved,
                    existing_output_paths: existing,
                },
            );

            let status = if context
                .skipped_artifacts
                .as_ref()
                .map_or(false, |s| s.contains(&artifact.id))
            {
                ArtifactStatusKind::Skipped
            } else if context.completed.contains(&artifact.id) {
                ArtifactStatusKind::Done
            } else if ready.contains(&artifact.id) {
                ArtifactStatusKind::Ready
            } else {
                ArtifactStatusKind::Blocked
            };

            ArtifactStatus {
                id: artifact.id.clone(),
                output_path: artifact.generates.clone(),
                status,
                requires: artifact.requires.clone(),
                missing_deps: blocked.get(&artifact.id).cloned(),
            }
        })
        .collect();

    // Sort by build order
    let build_order = context.graph.get_build_order();
    let order_map: HashMap<String, usize> = build_order
        .into_iter()
        .enumerate()
        .map(|(i, id)| (id, i))
        .collect();
    artifact_statuses.sort_by_key(|a| order_map.get(&a.id).copied().unwrap_or(0));

    let is_complete = context.graph.is_complete(&context.completed);

    ChangeStatus {
        change_name: context.change_name.clone(),
        schema_name: context.schema_name.clone(),
        change_root: context.change_dir.clone(),
        artifact_paths,
        is_planning_complete: is_complete,
        is_complete,
        apply_requires,
        artifacts: artifact_statuses,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_graph::schema::parse_schema;

    fn make_context(tmp: &Path) -> ChangeContext {
        let yaml = r#"
name: test
version: 1
artifacts:
  - id: proposal
    generates: proposal.md
    description: The proposal
    template: proposal.md
  - id: tasks
    generates: tasks.md
    description: Tasks
    template: tasks.md
    requires:
      - proposal
"#;
        let schema = parse_schema(yaml).unwrap();
        let graph = ArtifactGraph::from_schema(schema);
        let completed = detect_completed(&graph, tmp);

        ChangeContext {
            graph,
            completed,
            schema_name: "test".to_string(),
            change_name: "my-change".to_string(),
            change_dir: tmp.to_path_buf(),
            project_root: tmp.to_path_buf(),
            skipped_artifacts: None,
        }
    }

    #[test]
    fn format_change_status_shows_correct_states() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("proposal.md"), "content").unwrap();
        let ctx = make_context(tmp.path());

        let status = format_change_status(&ctx);
        assert_eq!(status.change_name, "my-change");
        assert_eq!(status.schema_name, "test");
        assert!(!status.is_complete);

        let proposal_status = status.artifacts.iter().find(|a| a.id == "proposal").unwrap();
        assert_eq!(proposal_status.status, ArtifactStatusKind::Done);

        let tasks_status = status.artifacts.iter().find(|a| a.id == "tasks").unwrap();
        assert_eq!(tasks_status.status, ArtifactStatusKind::Ready);
    }

    #[test]
    fn get_unlocked_artifacts_returns_dependents() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(tmp.path());
        let unlocks = get_unlocked_artifacts(&ctx.graph, "proposal");
        assert_eq!(unlocks, vec!["tasks"]);
    }

    #[test]
    fn get_dependency_info_reports_correctly() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(tmp.path());
        let tasks = ctx.graph.get_artifact("tasks").unwrap();
        let deps = get_dependency_info(tasks, &ctx.graph, &ctx.completed, None);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].id, "proposal");
        assert!(!deps[0].done);
    }
}
