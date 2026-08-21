use super::types::SkillTemplate;

/// Get all skill templates.
pub fn get_all_skill_templates() -> Vec<SkillTemplate> {
    vec![
        SkillTemplate {
            name: "speckit-propose".to_string(),
            description: "Create a new change proposal".to_string(),
            instructions: "# Proposal\n\n## Why\n\n[Explain motivation]\n\n## What Changes\n\n[List changes]\n\n## Capabilities\n\n[List capabilities]\n\n## Impact\n\n[Describe impact]\n".to_string(),
            license: Some("MIT".to_string()),
            compatibility: Some("Requires speckit CLI.".to_string()),
            metadata: None,
        },
        SkillTemplate {
            name: "speckit-explore".to_string(),
            description: "Explore and scope a feature idea".to_string(),
            instructions: "Help explore and scope a feature idea.".to_string(),
            license: Some("MIT".to_string()),
            compatibility: Some("Requires speckit CLI.".to_string()),
            metadata: None,
        },
        SkillTemplate {
            name: "speckit-apply".to_string(),
            description: "Apply change tasks".to_string(),
            instructions: "Work through pending tasks in a change.".to_string(),
            license: Some("MIT".to_string()),
            compatibility: Some("Requires speckit CLI.".to_string()),
            metadata: None,
        },
        SkillTemplate {
            name: "speckit-archive".to_string(),
            description: "Archive a completed change".to_string(),
            instructions: "Archive a completed change and update specs.".to_string(),
            license: Some("MIT".to_string()),
            compatibility: Some("Requires speckit CLI.".to_string()),
            metadata: None,
        },
    ]
}

/// Get a skill template by name.
pub fn get_skill_template(name: &str) -> Option<SkillTemplate> {
    get_all_skill_templates()
        .into_iter()
        .find(|t| t.name == name)
}
