use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use thiserror::Error;

use super::types::{Artifact, SchemaYaml};

/// Error returned when schema validation fails.
#[derive(Debug, Error)]
pub enum SchemaValidationError {
    #[error("Invalid schema YAML: {0}")]
    ParseError(String),

    #[error("Duplicate artifact ID: {0}")]
    DuplicateId(String),

    #[error("Invalid dependency reference in artifact '{artifact}': '{dependency}' does not exist")]
    InvalidDependency { artifact: String, dependency: String },

    #[error("Cyclic dependency detected: {cycle}")]
    CyclicDependency { cycle: String },
}

/// Loads and validates an artifact schema from a YAML file.
pub fn load_schema(file_path: &Path) -> Result<SchemaYaml, SchemaValidationError> {
    let content = fs::read_to_string(file_path).map_err(|e| {
        SchemaValidationError::ParseError(format!("Failed to read '{}': {}", file_path.display(), e))
    })?;
    parse_schema(&content)
}

/// Parses and validates an artifact schema from YAML content.
pub fn parse_schema(yaml_content: &str) -> Result<SchemaYaml, SchemaValidationError> {
    let schema: SchemaYaml = serde_yaml::from_str(yaml_content)
        .map_err(|e| SchemaValidationError::ParseError(e.to_string()))?;

    validate_no_duplicate_ids(&schema.artifacts)?;
    validate_requires_references(&schema.artifacts)?;
    validate_no_cycles(&schema.artifacts)?;

    Ok(schema)
}

/// Validates that there are no duplicate artifact IDs.
fn validate_no_duplicate_ids(artifacts: &[Artifact]) -> Result<(), SchemaValidationError> {
    let mut seen = HashSet::new();
    for artifact in artifacts {
        if !seen.insert(&artifact.id) {
            return Err(SchemaValidationError::DuplicateId(artifact.id.clone()));
        }
    }
    Ok(())
}

/// Validates that all `requires` references point to valid artifact IDs.
fn validate_requires_references(artifacts: &[Artifact]) -> Result<(), SchemaValidationError> {
    let valid_ids: HashSet<&str> = artifacts.iter().map(|a| a.id.as_str()).collect();

    for artifact in artifacts {
        for req in &artifact.requires {
            if !valid_ids.contains(req.as_str()) {
                return Err(SchemaValidationError::InvalidDependency {
                    artifact: artifact.id.clone(),
                    dependency: req.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Validates that there are no cyclic dependencies using DFS.
fn validate_no_cycles(artifacts: &[Artifact]) -> Result<(), SchemaValidationError> {
    let artifact_map: HashMap<&str, &Artifact> = artifacts.iter().map(|a| (a.id.as_str(), a)).collect();
    let mut visited = HashSet::new();
    let mut in_stack = HashSet::new();
    let mut parent: HashMap<String, String> = HashMap::new();

    fn dfs(
        id: &str,
        artifact_map: &HashMap<&str, &Artifact>,
        visited: &mut HashSet<String>,
        in_stack: &mut HashSet<String>,
        parent: &mut HashMap<String, String>,
    ) -> Option<String> {
        visited.insert(id.to_string());
        in_stack.insert(id.to_string());

        let artifact = match artifact_map.get(id) {
            Some(a) => a,
            None => return None,
        };

        for dep in &artifact.requires {
            if !visited.contains(dep.as_str()) {
                parent.insert(dep.clone(), id.to_string());
                if let Some(cycle) = dfs(dep, artifact_map, visited, in_stack, parent) {
                    return Some(cycle);
                }
            } else if in_stack.contains(dep.as_str()) {
                // Found a cycle - reconstruct the path
                let mut cycle_path = vec![dep.clone()];
                let mut current = id.to_string();
                while current != *dep {
                    cycle_path.insert(0, current.clone());
                    current = parent.get(&current).cloned().unwrap_or_default();
                }
                cycle_path.insert(0, dep.clone());
                return Some(cycle_path.join(" -> "));
            }
        }

        in_stack.remove(id);
        None
    }

    for artifact in artifacts {
        if !visited.contains(artifact.id.as_str()) {
            if let Some(cycle) = dfs(
                &artifact.id,
                &artifact_map,
                &mut visited,
                &mut in_stack,
                &mut parent,
            ) {
                return Err(SchemaValidationError::CyclicDependency { cycle });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_schema() {
        let yaml = r#"
name: test-schema
version: 1
artifacts:
  - id: proposal
    generates: proposal.md
    description: A proposal
    template: proposal.md
  - id: tasks
    generates: tasks.md
    description: Task list
    template: tasks.md
    requires:
      - proposal
"#;
        let schema = parse_schema(yaml).unwrap();
        assert_eq!(schema.name, "test-schema");
        assert_eq!(schema.artifacts.len(), 2);
        assert_eq!(schema.artifacts[1].requires, vec!["proposal"]);
    }

    #[test]
    fn reject_duplicate_ids() {
        let yaml = r#"
name: test
version: 1
artifacts:
  - id: foo
    generates: foo.md
    description: Foo
    template: foo.md
  - id: foo
    generates: bar.md
    description: Bar
    template: bar.md
"#;
        let err = parse_schema(yaml).unwrap_err();
        assert!(matches!(err, SchemaValidationError::DuplicateId(id) if id == "foo"));
    }

    #[test]
    fn reject_invalid_dependency() {
        let yaml = r#"
name: test
version: 1
artifacts:
  - id: foo
    generates: foo.md
    description: Foo
    template: foo.md
    requires:
      - nonexistent
"#;
        let err = parse_schema(yaml).unwrap_err();
        assert!(matches!(
            err,
            SchemaValidationError::InvalidDependency { artifact, dependency }
            if artifact == "foo" && dependency == "nonexistent"
        ));
    }

    #[test]
    fn reject_cyclic_dependency() {
        let yaml = r#"
name: test
version: 1
artifacts:
  - id: a
    generates: a.md
    description: A
    template: a.md
    requires:
      - b
  - id: b
    generates: b.md
    description: B
    template: b.md
    requires:
      - a
"#;
        let err = parse_schema(yaml).unwrap_err();
        assert!(matches!(err, SchemaValidationError::CyclicDependency { .. }));
    }

    #[test]
    fn load_schema_missing_file() {
        let err = load_schema(Path::new("/nonexistent/schema.yaml")).unwrap_err();
        assert!(matches!(err, SchemaValidationError::ParseError(_)));
    }
}
