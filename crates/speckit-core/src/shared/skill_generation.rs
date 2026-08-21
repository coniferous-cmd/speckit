use serde::{Deserialize, Serialize};

use crate::shared::skill_paths::SkillPaths;

/// Metadata about a generated skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// Unique skill name (kebab-case).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Tool this skill targets (e.g. "claude", "cursor").
    pub tool_id: String,
    /// The spec this skill is derived from, if any.
    pub spec_name: Option<String>,
}

/// Result of generating a skill file.
#[derive(Debug, Clone)]
pub struct GeneratedSkill {
    pub metadata: SkillMetadata,
    /// The resolved output path.
    pub output_path: std::path::PathBuf,
    /// The generated content.
    pub content: String,
}

/// Generates the markdown content for a Claude Code skill file.
///
/// Skills are markdown files that provide Claude with context about
/// a particular spec or workflow. This function produces a structured
/// markdown document from the given inputs.
pub fn generate_skill_content(
    name: &str,
    description: &str,
    body: &str,
    spec_content: Option<&str>,
) -> String {
    let mut content = String::new();

    content.push_str(&format!("# {name}\n\n"));
    content.push_str(&format!("{description}\n\n"));

    if let Some(spec) = spec_content {
        content.push_str("## Specification\n\n");
        content.push_str(spec);
        content.push_str("\n\n");
    }

    content.push_str("## Instructions\n\n");
    content.push_str(body);
    content.push('\n');

    content
}

/// Generates a skill file for Claude Code.
///
/// The skill is written to the `.claude/skills/` directory under the project root.
pub fn generate_claude_skill(
    project_root: &std::path::Path,
    metadata: &SkillMetadata,
    body: &str,
    spec_content: Option<&str>,
) -> GeneratedSkill {
    let paths = SkillPaths::resolve(project_root, "claude");
    let content = generate_skill_content(&metadata.name, &metadata.description, body, spec_content);
    let output_path = paths.skill_file(&metadata.name);

    GeneratedSkill {
        metadata: metadata.clone(),
        output_path,
        content,
    }
}

/// Writes a generated skill to disk, creating parent directories as needed.
pub fn write_skill(skill: &GeneratedSkill) -> Result<(), std::io::Error> {
    if let Some(parent) = skill.output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&skill.output_path, &skill.content)
}

/// Generates a summary of all specs in the specs directory as a skill.
pub fn generate_specs_summary_skill(
    project_root: &std::path::Path,
    spec_names: &[&str],
) -> GeneratedSkill {
    let metadata = SkillMetadata {
        name: "specs-summary".into(),
        description: "Summary of all project specifications".into(),
        tool_id: "claude".into(),
        spec_name: None,
    };

    let mut body = String::from("This project contains the following specifications:\n\n");
    for name in spec_names {
        body.push_str(&format!("- `{name}`\n"));
    }
    body.push('\n');
    body.push_str("Use the Read tool to load a specific spec when needed.\n");

    generate_claude_skill(project_root, &metadata, &body, None)
}

/// Generates a changelog skill from a list of change names.
pub fn generate_changes_skill(
    project_root: &std::path::Path,
    change_names: &[&str],
) -> GeneratedSkill {
    let metadata = SkillMetadata {
        name: "changes-summary".into(),
        description: "Summary of all active changes".into(),
        tool_id: "claude".into(),
        spec_name: None,
    };

    let mut body = String::from("This project has the following active changes:\n\n");
    for name in change_names {
        body.push_str(&format!("- `{name}`\n"));
    }
    body.push('\n');
    body.push_str("Use the Read tool to load a specific change file when needed.\n");

    generate_claude_skill(project_root, &metadata, &body, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn generate_skill_content_includes_all_sections() {
        let content = generate_skill_content(
            "my-skill",
            "A test skill",
            "Do the thing.",
            Some("The spec says to do the thing."),
        );
        assert!(content.contains("# my-skill"));
        assert!(content.contains("A test skill"));
        assert!(content.contains("## Specification"));
        assert!(content.contains("The spec says to do the thing."));
        assert!(content.contains("## Instructions"));
        assert!(content.contains("Do the thing."));
    }

    #[test]
    fn generate_skill_content_omits_spec_when_absent() {
        let content = generate_skill_content("my-skill", "A test skill", "Do the thing.", None);
        assert!(!content.contains("## Specification"));
        assert!(content.contains("## Instructions"));
    }

    #[test]
    fn generate_claude_skill_produces_correct_path() {
        let skill = generate_claude_skill(
            Path::new("/project"),
            &SkillMetadata {
                name: "test-skill".into(),
                description: "Test".into(),
                tool_id: "claude".into(),
                spec_name: None,
            },
            "Body",
            None,
        );
        assert_eq!(
            skill.output_path,
            std::path::PathBuf::from("/project/.claude/skills/test-skill.md")
        );
    }

    #[test]
    fn specs_summary_lists_all_specs() {
        let skill = generate_specs_summary_skill(
            Path::new("/project"),
            &["auth", "billing", "notifications"],
        );
        assert!(skill.content.contains("- `auth`"));
        assert!(skill.content.contains("- `billing`"));
        assert!(skill.content.contains("- `notifications`"));
    }
}
