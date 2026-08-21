use std::collections::HashMap;

/// Template types for skill generation.

/// A skill template.
#[derive(Debug, Clone)]
pub struct SkillTemplate {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
}

/// A command template.
#[derive(Debug, Clone)]
pub struct CommandTemplate {
    pub name: String,
    pub description: String,
    pub category: String,
    pub tags: Vec<String>,
    pub content: String,
}

/// A workflow template.
#[derive(Debug, Clone)]
pub struct WorkflowTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub content: String,
}
