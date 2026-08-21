use std::path::Path;

use super::graph::ArtifactGraph;
use super::outputs::artifact_output_exists;
use super::types::CompletedSet;

/// Detects which artifacts are completed by checking file existence in the
/// change directory.
///
/// Returns a `CompletedSet` containing the IDs of artifacts whose generated
/// files exist on disk. Handles a missing change directory gracefully by
/// returning an empty set.
pub fn detect_completed(graph: &ArtifactGraph, change_dir: &Path) -> CompletedSet {
    let mut completed = CompletedSet::new();

    // Handle missing change directory gracefully
    if !change_dir.exists() {
        return completed;
    }

    for artifact in graph.get_all_artifacts() {
        if artifact_output_exists(change_dir, &artifact.generates) {
            completed.insert(artifact.id.clone());
        }
    }

    completed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_graph::schema::parse_schema;
    use std::fs;

    fn test_graph() -> ArtifactGraph {
        let yaml = r#"
name: test
version: 1
artifacts:
  - id: proposal
    generates: proposal.md
    description: Proposal
    template: proposal.md
  - id: tasks
    generates: tasks.md
    description: Tasks
    template: tasks.md
    requires:
      - proposal
"#;
        ArtifactGraph::from_schema(parse_schema(yaml).unwrap())
    }

    #[test]
    fn detect_completed_empty_for_missing_dir() {
        let graph = test_graph();
        let completed = detect_completed(&graph, Path::new("/nonexistent/dir"));
        assert!(completed.is_empty());
    }

    #[test]
    fn detect_completed_finds_existing_files() {
        let graph = test_graph();
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("proposal.md"), "content").unwrap();

        let completed = detect_completed(&graph, tmp.path());
        assert!(completed.contains("proposal"));
        assert!(!completed.contains("tasks"));
    }

    #[test]
    fn detect_completed_finds_all_when_all_exist() {
        let graph = test_graph();
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("proposal.md"), "content").unwrap();
        fs::write(tmp.path().join("tasks.md"), "content").unwrap();

        let completed = detect_completed(&graph, tmp.path());
        assert_eq!(completed.len(), 2);
        assert!(completed.contains("proposal"));
        assert!(completed.contains("tasks"));
    }
}
