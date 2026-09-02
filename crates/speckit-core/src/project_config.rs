use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Hard limit on the context field size (50 KB), shared with the references index.
pub const MAX_CONTEXT_SIZE: usize = 50 * 1024;

/// Operation identifiers used as keys in the `operations:` map.
pub const OPERATION_IDS: &[&str] = &["implement", "archive"];

/// Advisory guidance for a single operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guidance: Option<Vec<String>>,
}

/// Per-operation advisory guidance (apply / archive).
pub type OperationsConfig = HashMap<String, OperationConfig>;

/// Normalized in-memory shape of a referenced store declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclarationEntry {
    pub id: String,
    /// Clone source rendered into onboarding fixes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
}

/// GitHub Copilot integration preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubCopilotConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_agent: Option<bool>,
}

/// Project configuration (`speckit/config.yaml`).
///
/// This is the primary type returned by [`read_project_config`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    /// Required: which schema to use (e.g. `"spec-driven"`).
    pub schema: String,
    /// Optional: project context injected into all artifact instructions.
    /// Max size: 50 KB (enforced during parsing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Optional: per-artifact rules, keyed by artifact ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<HashMap<String, Vec<String>>>,
    /// Optional: per-operation advisory guidance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operations: Option<OperationsConfig>,
    /// Optional: declared default store.  Only consulted by root resolution
    /// when this `speckit/` directory is config-only (no `specs/` or
    /// `changes/`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<String>,
    /// Optional: GitHub Copilot integration preferences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_copilot: Option<GitHubCopilotConfig>,
    /// Referenced store declarations.  Parsed by hand from the YAML rather
    /// than via the schema, because entries can be plain strings or
    /// `{id, remote}` maps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<Vec<DeclarationEntry>>,
}

/// Inputs loaded from the project config for a specific operation.
#[derive(Debug, Clone, Default)]
pub struct OperationInputs {
    pub context: Option<String>,
    pub operation_guidance: Option<Vec<String>>,
}

/// Load operation-specific inputs from the project config.
pub fn load_operation_inputs(
    project_config: Option<&ProjectConfig>,
    operation_id: &str,
) -> OperationInputs {
    let config = match project_config {
        Some(c) => c,
        None => return OperationInputs::default(),
    };

    let context = config.context.as_ref().and_then(|c| {
        let trimmed = c.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(c.clone())
        }
    });

    let operation_guidance = config
        .operations
        .as_ref()
        .and_then(|ops| ops.get(operation_id))
        .and_then(|op| op.guidance.as_ref())
        .and_then(|g| {
            let non_empty: Vec<String> = g.iter().filter(|s| !s.is_empty()).cloned().collect();
            if non_empty.is_empty() {
                None
            } else {
                Some(non_empty)
            }
        });

    OperationInputs {
        context,
        operation_guidance,
    }
}

// ---------------------------------------------------------------------------
// Config file resolution
// ---------------------------------------------------------------------------

/// Probe for `speckit/config.yaml` or `speckit/config.yml` under the given
/// project root.  Returns the first path that exists, or `None`.
pub fn resolve_config_file_path(project_root: &Path) -> Option<PathBuf> {
    let yaml_path = project_root.join("speckit").join("config.yaml");
    if yaml_path.exists() {
        return Some(yaml_path);
    }
    let yml_path = project_root.join("speckit").join("config.yml");
    if yml_path.exists() {
        return Some(yml_path);
    }
    None
}

/// Human rendering of a malformed pointer reason, shared by every surface.
pub fn store_pointer_problem(reason: &str) -> &'static str {
    match reason {
        "unparseable" => "the config file could not be read as YAML",
        "non_string" => "the store key must be a single store id string",
        _ => "unknown problem",
    }
}

// ---------------------------------------------------------------------------
// Store pointer (declared default store)
// ---------------------------------------------------------------------------

/// Result of a targeted read of the `store:` pointer.
#[derive(Debug, Clone)]
pub struct StorePointerRead {
    /// The declared store id, when present and a string.
    pub value: Option<String>,
    /// Set when the pointer cannot be trusted: the config file could not be
    /// read as YAML, or the store key is present but not a string.
    pub malformed: Option<String>,
    /// Absolute path of the config file actually read, or `None` when none
    /// exists.
    pub file_path: Option<PathBuf>,
}

/// Warning-silent targeted read of the `store:` pointer.
///
/// Used by root resolution (which must not re-emit the resilient parser's
/// field warnings) and by `speckit init`'s pointer guard.  Unlike
/// [`read_project_config`], a malformed value is **reported**, not dropped --
/// a dropped pointer would silently flip where work lands.
pub fn read_store_pointer(project_root: &Path) -> StorePointerRead {
    let config_path = match resolve_config_file_path(project_root) {
        Some(p) => p,
        None => {
            return StorePointerRead {
                value: None,
                malformed: None,
                file_path: None,
            };
        }
    };

    let content = match fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => {
            return StorePointerRead {
                value: None,
                malformed: Some("unparseable".into()),
                file_path: Some(config_path),
            };
        }
    };

    let raw: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(_) => {
            return StorePointerRead {
                value: None,
                malformed: Some("unparseable".into()),
                file_path: Some(config_path),
            };
        }
    };

    // Empty, comments-only, or non-mapping configs carry no pointer; they are
    // imperfect, not malformed.
    let mapping = match &raw {
        serde_yaml::Value::Mapping(m) => m,
        _ => {
            return StorePointerRead {
                value: None,
                malformed: None,
                file_path: Some(config_path),
            };
        }
    };

    let store_key = serde_yaml::Value::String("store".into());
    let store_value = mapping.get(&store_key);

    match store_value {
        None => StorePointerRead {
            value: None,
            malformed: None,
            file_path: Some(config_path),
        },
        Some(serde_yaml::Value::String(s)) => StorePointerRead {
            value: Some(s.clone()),
            malformed: None,
            file_path: Some(config_path),
        },
        Some(_) => StorePointerRead {
            value: None,
            malformed: Some("non_string".into()),
            file_path: Some(config_path),
        },
    }
}

// ---------------------------------------------------------------------------
// Speckit directory classification
// ---------------------------------------------------------------------------

/// Classification of an `speckit/` directory.
#[derive(Debug, Clone)]
pub struct SpeckitDirClassification {
    /// `true` when `speckit/specs` or `speckit/changes` exists as a directory.
    pub has_planning_shape: bool,
    pub pointer: StorePointerRead,
}

fn is_dir(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

/// One classification for "real root vs config-only pointer dir", shared by
/// root resolution and the init pointer guard so they can never disagree
/// (slice 3.2).
pub fn classify_speckit_dir(project_root: &Path) -> SpeckitDirClassification {
    let speckit_dir = project_root.join("speckit");
    let has_planning_shape =
        is_dir(&speckit_dir.join("specs")) || is_dir(&speckit_dir.join("changes"));
    SpeckitDirClassification {
        has_planning_shape,
        pointer: read_store_pointer(project_root),
    }
}

// ---------------------------------------------------------------------------
// Project config parsing (resilient, field-by-field)
// ---------------------------------------------------------------------------

/// Parse `operations:` from the raw YAML value.
fn parse_operations(raw: &serde_yaml::Value) -> Option<OperationsConfig> {
    let mapping = match raw {
        serde_yaml::Value::Mapping(m) => m,
        _ => {
            eprintln!("Invalid 'operations' field in config (must be object)");
            return None;
        }
    };

    let supported: std::collections::HashSet<&str> = OPERATION_IDS.iter().copied().collect();
    let mut operations = OperationsConfig::new();

    for (key, value) in mapping {
        let operation_id = match key {
            serde_yaml::Value::String(s) => s.as_str(),
            _ => continue,
        };

        if operation_id == "apply" {
            eprintln!(
                "The `operations.apply` key has been renamed to `operations.implement`. \
                 Please update your speckit config."
            );
            continue;
        }

        if !supported.contains(operation_id) {
            eprintln!(
                "Unknown operation ID '{}' in config. Supported operation IDs: {}",
                operation_id,
                OPERATION_IDS.join(", ")
            );
            continue;
        }

        let op_mapping = match value {
            serde_yaml::Value::Mapping(m) => m,
            _ => {
                eprintln!(
                    "Invalid 'operations.{operation_id}' field in config (must be object), ignoring this operation"
                );
                continue;
            }
        };

        // Warn about unknown fields.
        let unknown_fields: Vec<&str> = op_mapping
            .keys()
            .filter_map(|k| match k {
                serde_yaml::Value::String(s) if s != "guidance" => Some(s.as_str()),
                _ => None,
            })
            .collect();
        if !unknown_fields.is_empty() {
            eprintln!(
                "Unknown field(s) in 'operations.{operation_id}': {}. Supported fields: guidance",
                unknown_fields.join(", ")
            );
        }

        let guidance_key = serde_yaml::Value::String("guidance".into());
        let guidance_value = match op_mapping.get(&guidance_key) {
            Some(v) => v,
            None => continue,
        };

        let guidance_seq = match guidance_value {
            serde_yaml::Value::Sequence(seq) => seq,
            _ => {
                eprintln!(
                    "Guidance for operation '{operation_id}' must be an array of strings, ignoring this operation's guidance"
                );
                continue;
            }
        };

        let guidance: Vec<String> = guidance_seq
            .iter()
            .filter_map(|v| match v {
                serde_yaml::Value::String(s) => {
                    if s.is_empty() {
                        None
                    } else {
                        Some(s.clone())
                    }
                }
                _ => None,
            })
            .collect();

        if guidance.len() < guidance_seq.len() {
            eprintln!(
                "Some guidance for operation '{operation_id}' are empty strings, ignoring them"
            );
        }

        if !guidance.is_empty() {
            operations.insert(
                operation_id.to_string(),
                OperationConfig {
                    guidance: Some(guidance),
                },
            );
        }
    }

    if operations.is_empty() {
        None
    } else {
        Some(operations)
    }
}

/// Parse `references:` declarations from the raw YAML value.
///
/// Entries can be plain strings or `{id, remote}` maps.  Deduplicates on `id`
/// and keeps the first position; the first entry carrying a `remote` supplies
/// it.
fn parse_declaration_list(raw: &serde_yaml::Value) -> Option<Vec<DeclarationEntry>> {
    let seq = match raw {
        serde_yaml::Value::Sequence(s) => s,
        _ => {
            eprintln!("Invalid 'references' field in config (must be an array of store ids)");
            return None;
        }
    };

    // Use a Vec to preserve insertion order; dedup by id.
    let mut result: Vec<DeclarationEntry> = Vec::new();
    let mut id_positions: HashMap<String, usize> = HashMap::new();
    let mut dropped_entries = false;
    let mut dropped_remotes = false;

    for entry in seq {
        let declaration: Option<DeclarationEntry> = match entry {
            serde_yaml::Value::String(s) => Some(DeclarationEntry {
                id: s.clone(),
                remote: None,
            }),
            serde_yaml::Value::Mapping(m) => {
                let id_key = serde_yaml::Value::String("id".into());
                match m.get(&id_key) {
                    Some(serde_yaml::Value::String(id)) => {
                        let remote_key = serde_yaml::Value::String("remote".into());
                        let remote = match m.get(&remote_key) {
                            Some(serde_yaml::Value::String(r)) if !r.is_empty() => Some(r.clone()),
                            Some(_) => {
                                dropped_remotes = true;
                                None
                            }
                            None => None,
                        };
                        Some(DeclarationEntry {
                            id: id.clone(),
                            remote,
                        })
                    }
                    _ => {
                        dropped_entries = true;
                        None
                    }
                }
            }
            _ => {
                dropped_entries = true;
                None
            }
        };

        let declaration = match declaration {
            Some(d) => d,
            None => continue,
        };

        if let Some(&pos) = id_positions.get(&declaration.id) {
            // Fill in a missing remote on the existing entry.
            if result[pos].remote.is_none() && declaration.remote.is_some() {
                result[pos].remote = declaration.remote;
            }
        } else {
            let pos = result.len();
            id_positions.insert(declaration.id.clone(), pos);
            result.push(declaration);
        }
    }

    if dropped_entries {
        eprintln!("Some 'references' entries are invalid, ignoring them");
    }
    if dropped_remotes {
        eprintln!(
            "Some 'references' remotes are not non-empty strings; the ids are kept without a clone source"
        );
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Read and parse `speckit/config.yaml` from the project root.
///
/// Uses resilient parsing -- validates each field independently.
/// Returns `None` if the file doesn't exist.
/// Returns a partial config if some fields are invalid (with warnings).
pub fn read_project_config(project_root: &Path) -> Option<ProjectConfig> {
    let config_path = resolve_config_file_path(project_root)?;

    let content = match fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "Warning: could not parse {} ({}); ignoring it.",
                config_path.display(),
                e
            );
            return None;
        }
    };

    let raw: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "Warning: could not parse {} ({}); ignoring it.",
                config_path.display(),
                e
            );
            return None;
        }
    };

    let raw_mapping = match &raw {
        serde_yaml::Value::Mapping(m) => m,
        _ => {
            eprintln!("speckit/config.yaml is not a valid YAML object");
            return None;
        }
    };

    // Parse schema field.
    let schema_key = serde_yaml::Value::String("schema".into());
    let schema = match raw_mapping.get(&schema_key) {
        Some(serde_yaml::Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(_) => {
            eprintln!("Invalid 'schema' field in config (must be non-empty string)");
            None
        }
        None => None,
    };

    // Parse context field with size limit.
    let context_key = serde_yaml::Value::String("context".into());
    let context = match raw_mapping.get(&context_key) {
        Some(serde_yaml::Value::String(s)) => {
            let context_size = s.len();
            if context_size > MAX_CONTEXT_SIZE {
                eprintln!(
                    "Context too large ({:.1}KB, limit: {}KB)",
                    context_size as f64 / 1024.0,
                    MAX_CONTEXT_SIZE / 1024
                );
                eprintln!("Ignoring context field");
                None
            } else {
                Some(s.clone())
            }
        }
        Some(_) => {
            eprintln!("Invalid 'context' field in config (must be string)");
            None
        }
        None => None,
    };

    // Parse rules field.
    let rules_key = serde_yaml::Value::String("rules".into());
    let rules = match raw_mapping.get(&rules_key) {
        Some(serde_yaml::Value::Mapping(rules_map)) => {
            let mut parsed_rules: HashMap<String, Vec<String>> = HashMap::new();

            for (artifact_key, rules_value) in rules_map {
                let artifact_id = match artifact_key {
                    serde_yaml::Value::String(s) => s.clone(),
                    _ => continue,
                };

                match rules_value {
                    serde_yaml::Value::Sequence(seq) => {
                        let valid_rules: Vec<String> = seq
                            .iter()
                            .filter_map(|v| match v {
                                serde_yaml::Value::String(s) if !s.is_empty() => Some(s.clone()),
                                _ => None,
                            })
                            .collect();
                        if valid_rules.len() < seq.len() {
                            eprintln!(
                                "Some rules for '{artifact_id}' are empty strings, ignoring them"
                            );
                        }
                        if !valid_rules.is_empty() {
                            parsed_rules.insert(artifact_id, valid_rules);
                        }
                    }
                    _ => {
                        eprintln!(
                            "Rules for '{artifact_id}' must be an array of strings, ignoring this artifact's rules"
                        );
                    }
                }
            }

            if parsed_rules.is_empty() {
                None
            } else {
                Some(parsed_rules)
            }
        }
        Some(_) => {
            eprintln!("Invalid 'rules' field in config (must be object)");
            None
        }
        None => None,
    };

    // Parse operations field.
    let ops_key = serde_yaml::Value::String("operations".into());
    let operations = raw_mapping.get(&ops_key).and_then(parse_operations);

    // Parse references field.
    let refs_key = serde_yaml::Value::String("references".into());
    let references = raw_mapping.get(&refs_key).and_then(parse_declaration_list);

    // Parse store pointer field.
    let store_key = serde_yaml::Value::String("store".into());
    let store = match raw_mapping.get(&store_key) {
        Some(serde_yaml::Value::String(s)) => Some(s.clone()),
        Some(_) => {
            eprintln!(
                "Warning: ignoring invalid store: field in {} (must be a single store id string).",
                config_path.display()
            );
            None
        }
        None => None,
    };

    // Parse githubCopilot preferences.
    let gc_key = serde_yaml::Value::String("githubCopilot".into());
    let github_copilot = match raw_mapping.get(&gc_key) {
        Some(serde_yaml::Value::Mapping(m)) => {
            let cloud_key = serde_yaml::Value::String("cloudAgent".into());
            match m.get(&cloud_key) {
                Some(serde_yaml::Value::Bool(b)) => Some(GitHubCopilotConfig {
                    cloud_agent: Some(*b),
                }),
                Some(_) => {
                    eprintln!(
                        "Invalid 'githubCopilot.cloudAgent' field in config (must be a boolean)"
                    );
                    None
                }
                None => None,
            }
        }
        Some(_) => {
            eprintln!("Invalid 'githubCopilot' field in config (must be an object)");
            None
        }
        None => None,
    };

    // Build config; return None if nothing was parsed.
    let config = ProjectConfig {
        schema: schema?,
        context,
        rules,
        operations,
        store,
        github_copilot,
        references,
    };

    Some(config)
}

/// Validate artifact IDs in rules against the artifacts of every available
/// schema.  Returns warnings for keys that are unknown everywhere.
pub fn validate_config_rules(
    rules: &HashMap<String, Vec<String>>,
    valid_artifact_ids: &std::collections::HashSet<String>,
) -> Vec<String> {
    let mut warnings = Vec::new();

    for artifact_id in rules.keys() {
        if !valid_artifact_ids.contains(artifact_id.as_str()) {
            let mut sorted: Vec<&str> = valid_artifact_ids.iter().map(|s| s.as_str()).collect();
            sorted.sort();
            warnings.push(format!(
                "Unknown artifact ID in rules: \"{artifact_id}\". \
                 It matches no artifact in any available schema. \
                 Known artifact IDs: {}",
                sorted.join(", ")
            ));
        }
    }

    warnings
}

/// Simple Levenshtein distance for fuzzy schema name suggestions.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let n = a_chars.len();
    let m = b_chars.len();

    let mut matrix = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m {
        matrix[i][0] = i;
    }
    for j in 0..=n {
        matrix[0][j] = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if b_chars[i - 1] == a_chars[j - 1] {
                0
            } else {
                1
            };
            matrix[i][j] = (matrix[i - 1][j - 1] + cost)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j] + 1);
        }
    }
    matrix[m][n]
}

/// Suggest valid schema names when the user provides an invalid schema.
/// Uses fuzzy matching to find similar names.
pub fn suggest_schemas(invalid_schema_name: &str, available_schemas: &[(String, bool)]) -> String {
    // Find closest matches (distance <= 3).
    let mut suggestions: Vec<(usize, &str, bool)> = available_schemas
        .iter()
        .map(|(name, is_built_in)| {
            let dist = levenshtein(invalid_schema_name, name);
            (dist, name.as_str(), *is_built_in)
        })
        .filter(|(dist, _, _)| *dist <= 3)
        .collect();
    suggestions.sort_by_key(|(dist, _, _)| *dist);
    suggestions.truncate(3);

    let built_in: Vec<&str> = available_schemas
        .iter()
        .filter(|(_, b)| *b)
        .map(|(n, _)| n.as_str())
        .collect();
    let project_local: Vec<&str> = available_schemas
        .iter()
        .filter(|(_, b)| !*b)
        .map(|(n, _)| n.as_str())
        .collect();

    let mut message =
        format!("Schema '{invalid_schema_name}' not found in speckit/config.yaml\n\n");

    if !suggestions.is_empty() {
        message.push_str("Did you mean one of these?\n");
        for (_, name, is_built_in) in &suggestions {
            let type_label = if *is_built_in {
                "built-in"
            } else {
                "project-local"
            };
            message.push_str(&format!("  - {name} ({type_label})\n"));
        }
        message.push('\n');
    }

    message.push_str("Available schemas:\n");
    if !built_in.is_empty() {
        message.push_str(&format!("  Built-in: {}\n", built_in.join(", ")));
    }
    if !project_local.is_empty() {
        message.push_str(&format!("  Project-local: {}\n", project_local.join(", ")));
    } else {
        message.push_str("  Project-local: (none found)\n");
    }

    message.push_str(&format!(
        "\nFix: Edit speckit/config.yaml and change 'schema: {invalid_schema_name}' to a valid schema name"
    ));

    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_pointer_problem_messages() {
        assert_eq!(
            store_pointer_problem("unparseable"),
            "the config file could not be read as YAML"
        );
        assert_eq!(
            store_pointer_problem("non_string"),
            "the store key must be a single store id string"
        );
    }

    #[test]
    fn levenshtein_identical() {
        assert_eq!(levenshtein("abc", "abc"), 0);
    }

    #[test]
    fn levenshtein_one_edit() {
        assert_eq!(levenshtein("abc", "ab"), 1);
        assert_eq!(levenshtein("abc", "abcd"), 1);
        assert_eq!(levenshtein("abc", "adc"), 1);
    }

    #[test]
    fn load_operation_inputs_empty_config() {
        let inputs = load_operation_inputs(None, "implement");
        assert!(inputs.context.is_none());
        assert!(inputs.operation_guidance.is_none());
    }

    #[test]
    fn load_operation_inputs_with_context() {
        let config = ProjectConfig {
            schema: "spec-driven".into(),
            context: Some("My project context".into()),
            rules: None,
            operations: None,
            store: None,
            github_copilot: None,
            references: None,
        };
        let inputs = load_operation_inputs(Some(&config), "implement");
        assert_eq!(inputs.context.as_deref(), Some("My project context"));
    }

    #[test]
    fn load_operation_inputs_with_guidance() {
        let mut operations = OperationsConfig::new();
        operations.insert(
            "implement".into(),
            OperationConfig {
                guidance: Some(vec!["Keep it short".into()]),
            },
        );
        let config = ProjectConfig {
            schema: "spec-driven".into(),
            context: None,
            rules: None,
            operations: Some(operations),
            store: None,
            github_copilot: None,
            references: None,
        };
        let inputs = load_operation_inputs(Some(&config), "implement");
        assert_eq!(
            inputs.operation_guidance,
            Some(vec!["Keep it short".into()])
        );
    }

    #[test]
    fn validate_config_rules_detects_unknown() {
        let mut rules = HashMap::new();
        rules.insert("proposal".into(), vec!["rule1".into()]);

        let mut valid = std::collections::HashSet::new();
        valid.insert("tasks".into());

        let warnings = validate_config_rules(&rules, &valid);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("proposal"));
    }

    #[test]
    fn suggest_schemas_finds_close_match() {
        let schemas = vec![
            ("spec-driven".into(), true),
            ("custom-schema".into(), false),
        ];
        let msg = suggest_schemas("spec_driven", &schemas);
        assert!(msg.contains("Did you mean"));
    }

    #[test]
    fn classify_speckit_dir_nonexistent() {
        let dir = classify_speckit_dir(Path::new("/nonexistent/project"));
        assert!(!dir.has_planning_shape);
        assert!(dir.pointer.file_path.is_none());
    }
}
