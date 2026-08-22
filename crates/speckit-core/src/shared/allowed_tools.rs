use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Default set of tools allowed in Speckit skill generation.
const DEFAULT_ALLOWED_TOOLS: &[&str] = &[
    "Read",
    "Write",
    "Edit",
    "Bash",
    "Glob",
    "Grep",
    "WebFetch",
    "WebSearch",
];

/// Configuration for allowed tools in a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct AllowedToolsConfig {
    /// Additional tools beyond the defaults.
    #[serde(default)]
    pub allow: Vec<String>,
    /// Tools explicitly denied, even if matched by a broader pattern.
    #[serde(default)]
    pub deny: Vec<String>,
}


/// Resolves the effective set of allowed tools given a config.
///
/// Starts with the default set, adds any extras from `config.allow`,
/// and removes anything in `config.deny`.
pub fn resolve_allowed_tools(config: &AllowedToolsConfig) -> HashSet<String> {
    let mut tools: HashSet<String> = DEFAULT_ALLOWED_TOOLS
        .iter()
        .map(|s| s.to_string())
        .collect();

    for tool in &config.allow {
        tools.insert(tool.clone());
    }

    for tool in &config.deny {
        tools.remove(tool);
    }

    tools
}

/// Reads an allowed-tools config from a YAML file, if it exists.
pub fn read_allowed_tools_config(path: &Path) -> AllowedToolsConfig {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return AllowedToolsConfig::default(),
    };

    serde_yaml::from_str(&content).unwrap_or_default()
}

/// Returns the sorted list of default allowed tool names.
pub fn default_allowed_tool_names() -> Vec<String> {
    let mut names: Vec<String> = DEFAULT_ALLOWED_TOOLS
        .iter()
        .map(|s| s.to_string())
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tools_contain_basics() {
        let tools = resolve_allowed_tools(&AllowedToolsConfig::default());
        assert!(tools.contains("Read"));
        assert!(tools.contains("Write"));
        assert!(tools.contains("Bash"));
    }

    #[test]
    fn deny_removes_from_defaults() {
        let config = AllowedToolsConfig {
            allow: vec![],
            deny: vec!["Bash".into()],
        };
        let tools = resolve_allowed_tools(&config);
        assert!(!tools.contains("Bash"));
        assert!(tools.contains("Read"));
    }

    #[test]
    fn allow_adds_to_defaults() {
        let config = AllowedToolsConfig {
            allow: vec!["CustomTool".into()],
            deny: vec![],
        };
        let tools = resolve_allowed_tools(&config);
        assert!(tools.contains("CustomTool"));
    }
}
