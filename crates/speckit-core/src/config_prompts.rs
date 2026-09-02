use crate::project_config::ProjectConfig;

/// Serialize a partial project config to a YAML string with helpful comments.
///
/// This is the string shown in template prompts that guide the user when
/// creating or editing `speckit/config.yaml`.
pub fn serialize_config(config: &ProjectConfig) -> String {
    let mut lines: Vec<String> = Vec::new();

    // Schema (required)
    lines.push(format!("schema: {}", config.schema));
    lines.push(String::new());

    // Context section with comments.
    lines.push("# Project context (optional)".into());
    lines.push("# This is shown to AI when creating artifacts.".into());
    lines.push("# Add your tech stack, conventions, style guides, domain knowledge, etc.".into());
    lines.push("# Example:".into());
    lines.push("#   context: |".into());
    lines.push("#     Tech stack: TypeScript, React, Node.js".into());
    lines.push("#     We use conventional commits".into());
    lines.push("#     Domain: e-commerce platform".into());
    lines.push(String::new());

    // Rules section with comments.
    lines.push("# Per-artifact rules (optional)".into());
    lines.push("# Add custom rules for specific artifacts.".into());
    lines.push("# Example:".into());
    lines.push("#   rules:".into());
    lines.push("#     proposal:".into());
    lines.push("#       - Keep proposals under 500 words".into());
    lines.push("#       - Always include a \"Non-goals\" section".into());
    lines.push("#     tasks:".into());
    lines.push("#       - Break tasks into chunks of max 2 hours".into());
    lines.push(String::new());

    // Operation guidance section with comments.
    lines.push("# Per-operation guidance (optional)".into());
    lines.push(
        "# Add advisory guidance for how implement and archive work should be conducted.".into(),
    );
    lines.push("# This is separate from artifact rules above.".into());
    lines.push("# Example:".into());
    lines.push("#   operations:".into());
    lines.push("#     implement:".into());
    lines.push("#       guidance:".into());
    lines.push("#         - Keep test summaries concise".into());
    lines.push("#     archive:".into());
    lines.push("#       guidance:".into());
    lines.push("#         - Summarize the archive outcome before finishing".into());

    let mut output = lines.join("\n");
    output.push('\n');
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_config::ProjectConfig;

    #[test]
    fn serialize_config_contains_schema() {
        let config = ProjectConfig {
            schema: "spec-driven".into(),
            context: None,
            rules: None,
            operations: None,
            store: None,
            github_copilot: None,
            references: None,
        };
        let output = serialize_config(&config);
        assert!(output.contains("schema: spec-driven"));
    }

    #[test]
    fn serialize_config_has_comment_sections() {
        let config = ProjectConfig {
            schema: "spec-driven".into(),
            context: None,
            rules: None,
            operations: None,
            store: None,
            github_copilot: None,
            references: None,
        };
        let output = serialize_config(&config);
        assert!(output.contains("# Project context (optional)"));
        assert!(output.contains("# Per-artifact rules (optional)"));
        assert!(output.contains("# Per-operation guidance (optional)"));
    }

    #[test]
    fn serialize_config_ends_with_newline() {
        let config = ProjectConfig {
            schema: "spec-driven".into(),
            context: None,
            rules: None,
            operations: None,
            store: None,
            github_copilot: None,
            references: None,
        };
        let output = serialize_config(&config);
        assert!(output.ends_with('\n'));
    }
}
