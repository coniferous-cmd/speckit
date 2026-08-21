use std::collections::HashMap;

use super::schema::parse_schema;
use super::types::{Artifact, BlockedArtifacts, CompletedSet, SchemaYaml};

/// Represents an artifact dependency graph.
///
/// Provides methods for querying build order, ready artifacts, and completion
/// status. Ties between siblings are broken by declaration order in the schema,
/// preserving the schema author's intended sequence.
#[derive(Debug, Clone)]
pub struct ArtifactGraph {
    artifacts: HashMap<String, Artifact>,
    schema: SchemaYaml,
    /// Artifact id -> its position in the schema's `artifacts:` list.
    declaration_order: HashMap<String, usize>,
}

impl ArtifactGraph {
    /// Creates an `ArtifactGraph` from a pre-validated schema object.
    pub fn from_schema(schema: SchemaYaml) -> Self {
        let artifacts: HashMap<String, Artifact> = schema
            .artifacts
            .iter()
            .map(|a| (a.id.clone(), a.clone()))
            .collect();
        let declaration_order: HashMap<String, usize> = schema
            .artifacts
            .iter()
            .enumerate()
            .map(|(i, a)| (a.id.clone(), i))
            .collect();

        Self {
            artifacts,
            schema,
            declaration_order,
        }
    }

    /// Creates an `ArtifactGraph` from a YAML content string.
    pub fn from_yaml_content(yaml_content: &str) -> Result<Self, super::schema::SchemaValidationError> {
        let schema = parse_schema(yaml_content)?;
        Ok(Self::from_schema(schema))
    }

    /// Gets a single artifact by ID.
    pub fn get_artifact(&self, id: &str) -> Option<&Artifact> {
        self.artifacts.get(id)
    }

    /// Returns a reference to all artifacts, in declaration order.
    pub fn get_all_artifacts(&self) -> Vec<&Artifact> {
        let mut artifacts: Vec<&Artifact> = self.schema.artifacts.iter().collect();
        artifacts.sort_by_key(|a| self.declaration_order.get(&a.id).copied().unwrap_or(usize::MAX));
        artifacts
    }

    /// Gets the schema name.
    pub fn name(&self) -> &str {
        &self.schema.name
    }

    /// Gets the schema version.
    pub fn version(&self) -> u32 {
        self.schema.version
    }

    /// Returns a reference to the underlying schema.
    pub fn schema(&self) -> &SchemaYaml {
        &self.schema
    }

    /// Computes the topological build order using Kahn's algorithm.
    ///
    /// Returns artifact IDs in the order they should be built, with ties broken
    /// by declaration order in the schema.
    pub fn get_build_order(&self) -> Vec<String> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

        // Initialize all artifacts
        for artifact in self.artifacts.values() {
            in_degree.insert(artifact.id.clone(), artifact.requires.len());
            dependents.entry(artifact.id.clone()).or_default();
        }

        // Build reverse adjacency (who depends on whom)
        for artifact in self.artifacts.values() {
            for req in &artifact.requires {
                dependents
                    .entry(req.clone())
                    .or_default()
                    .push(artifact.id.clone());
            }
        }

        // Start with roots (in-degree 0), sorted by declaration order
        let mut queue: Vec<String> = self
            .artifacts
            .keys()
            .filter(|id| in_degree.get(*id).copied().unwrap_or(0) == 0)
            .cloned()
            .collect();
        queue.sort_by(|a, b| self.compare_by_declaration_order(a, b));

        let mut result = Vec::new();

        while let Some(current) = queue.first().cloned() {
            queue.remove(0);
            result.push(current.clone());

            // Collect newly ready artifacts
            let mut newly_ready = Vec::new();
            if let Some(deps) = dependents.get(&current) {
                for dep in deps {
                    let new_degree = in_degree.get(dep).copied().unwrap_or(1).saturating_sub(1);
                    in_degree.insert(dep.clone(), new_degree);
                    if new_degree == 0 {
                        newly_ready.push(dep.clone());
                    }
                }
            }

            // Re-sort the whole queue for determinism
            queue.extend(newly_ready);
            queue.sort_by(|a, b| self.compare_by_declaration_order(a, b));
        }

        result
    }

    /// Gets artifacts that are ready to be created (all dependencies completed).
    ///
    /// Returns artifact IDs sorted by declaration order.
    pub fn get_next_artifacts(&self, completed: &CompletedSet) -> Vec<String> {
        let mut ready: Vec<String> = Vec::new();

        for artifact in self.artifacts.values() {
            if completed.contains(&artifact.id) {
                continue; // Already completed
            }

            let all_deps_completed = artifact.requires.iter().all(|req| completed.contains(req));
            if all_deps_completed {
                ready.push(artifact.id.clone());
            }
        }

        ready.sort_by(|a, b| self.compare_by_declaration_order(a, b));
        ready
    }

    /// Checks if all artifacts in the graph are completed.
    pub fn is_complete(&self, completed: &CompletedSet) -> bool {
        self.artifacts.values().all(|a| completed.contains(&a.id))
    }

    /// Gets blocked artifacts and their unmet dependencies.
    ///
    /// Returns a map from artifact ID to its list of unmet dependencies, sorted
    /// by declaration order.
    pub fn get_blocked(&self, completed: &CompletedSet) -> BlockedArtifacts {
        let mut blocked = BlockedArtifacts::new();

        for artifact in self.artifacts.values() {
            if completed.contains(&artifact.id) {
                continue; // Already completed
            }

            let mut unmet_deps: Vec<String> = artifact
                .requires
                .iter()
                .filter(|req| !completed.contains(*req))
                .cloned()
                .collect();

            if !unmet_deps.is_empty() {
                unmet_deps.sort_by(|a, b| self.compare_by_declaration_order(a, b));
                blocked.insert(artifact.id.clone(), unmet_deps);
            }
        }

        blocked
    }

    /// Orders artifact IDs by where the schema declares them.
    ///
    /// The dependency graph leaves siblings tied -- spec-driven's `specs` and
    /// `design` both require only `proposal`, so both become ready at the same
    /// time. Breaking ties by declaration order follows the sequence the schema
    /// author wrote, for built-in and custom schemas alike.
    fn compare_by_declaration_order(&self, a: &str, b: &str) -> std::cmp::Ordering {
        let a_order = self.declaration_order.get(a).copied().unwrap_or(usize::MAX);
        let b_order = self.declaration_order.get(b).copied().unwrap_or(usize::MAX);
        a_order.cmp(&b_order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_driven_schema() -> SchemaYaml {
        let yaml = r#"
name: spec-driven
version: 1
artifacts:
  - id: proposal
    generates: proposal.md
    description: Project proposal
    template: proposal.md
  - id: specs
    generates: "specs/**/*.md"
    description: Requirements specs
    template: spec.md
    requires:
      - proposal
  - id: design
    generates: design.md
    description: Design document
    template: design.md
    requires:
      - proposal
  - id: tasks
    generates: tasks.md
    description: Implementation tasks
    template: tasks.md
    requires:
      - specs
      - design
"#;
        parse_schema(yaml).unwrap()
    }

    #[test]
    fn build_order_respects_dependencies() {
        let graph = ArtifactGraph::from_schema(spec_driven_schema());
        let order = graph.get_build_order();

        assert_eq!(order[0], "proposal");
        // specs and design both depend on proposal; declaration order: specs before design
        assert_eq!(order[1], "specs");
        assert_eq!(order[2], "design");
        assert_eq!(order[3], "tasks");
    }

    #[test]
    fn next_artifacts_with_empty_completed() {
        let graph = ArtifactGraph::from_schema(spec_driven_schema());
        let completed = CompletedSet::new();
        let next = graph.get_next_artifacts(&completed);
        assert_eq!(next, vec!["proposal"]);
    }

    #[test]
    fn next_artifacts_after_proposal() {
        let graph = ArtifactGraph::from_schema(spec_driven_schema());
        let mut completed = CompletedSet::new();
        completed.insert("proposal".to_string());
        let next = graph.get_next_artifacts(&completed);
        assert_eq!(next, vec!["specs", "design"]);
    }

    #[test]
    fn is_complete_false_when_not_all_done() {
        let graph = ArtifactGraph::from_schema(spec_driven_schema());
        let mut completed = CompletedSet::new();
        completed.insert("proposal".to_string());
        assert!(!graph.is_complete(&completed));
    }

    #[test]
    fn is_complete_true_when_all_done() {
        let graph = ArtifactGraph::from_schema(spec_driven_schema());
        let mut completed = CompletedSet::new();
        for id in &["proposal", "specs", "design", "tasks"] {
            completed.insert(id.to_string());
        }
        assert!(graph.is_complete(&completed));
    }

    #[test]
    fn blocked_reports_unmet_deps() {
        let graph = ArtifactGraph::from_schema(spec_driven_schema());
        let completed = CompletedSet::new();
        let blocked = graph.get_blocked(&completed);
        assert_eq!(blocked["specs"], vec!["proposal"]);
        assert_eq!(blocked["design"], vec!["proposal"]);
        assert!(blocked["tasks"].contains(&"specs".to_string()));
        assert!(blocked["tasks"].contains(&"design".to_string()));
        // proposal has no deps so it's not blocked
        assert!(!blocked.contains_key("proposal"));
    }

    #[test]
    fn get_artifact_by_id() {
        let graph = ArtifactGraph::from_schema(spec_driven_schema());
        let artifact = graph.get_artifact("proposal").unwrap();
        assert_eq!(artifact.description, "Project proposal");
        assert!(graph.get_artifact("nonexistent").is_none());
    }

    #[test]
    fn name_and_version() {
        let graph = ArtifactGraph::from_schema(spec_driven_schema());
        assert_eq!(graph.name(), "spec-driven");
        assert_eq!(graph.version(), 1);
    }

    #[test]
    fn from_yaml_content_parses_valid() {
        let yaml = r#"
name: simple
version: 1
artifacts:
  - id: output
    generates: out.md
    description: Output
    template: out.md
"#;
        let graph = ArtifactGraph::from_yaml_content(yaml).unwrap();
        assert_eq!(graph.name(), "simple");
    }
}
