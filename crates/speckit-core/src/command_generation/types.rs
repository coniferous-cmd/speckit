//! Command Generation Types
//!
//! Tool-agnostic interfaces for command generation.
//! These types separate "what to generate" from "how to format it".

use std::fmt;

/// Tool-agnostic command data.
/// Represents the content of a command without any tool-specific formatting.
#[derive(Debug, Clone)]
pub struct CommandContent {
    /// Command identifier (e.g., 'explore', 'apply', 'new')
    pub id: String,
    /// Human-readable name (e.g., 'Speckit Explore')
    pub name: String,
    /// Brief description of command purpose
    pub description: String,
    /// Grouping category (e.g., 'Workflow')
    pub category: String,
    /// Array of tag strings
    pub tags: Vec<String>,
    /// The command instruction content (body text)
    pub body: String,
}

/// Per-tool formatting strategy.
/// Each AI tool implements this trait to handle its specific file path
/// and frontmatter format requirements.
pub trait ToolCommandAdapter {
    /// Tool identifier matching AIToolOption value (e.g., 'claude', 'cursor')
    fn tool_id(&self) -> &str;

    /// Returns the file path for a command.
    /// May be absolute for tools with global-scoped command files.
    fn get_file_path(&self, command_id: &str) -> String;

    /// What the user types before the command name, when it is not the default `/`.
    /// Amazon Q loads these files into its prompt library, which is invoked with `@`,
    /// so its adapter returns Some("@"). Most tools return None (defaults to "/").
    fn invocation_prefix(&self) -> Option<&str> {
        None
    }

    /// Formats the complete file content including frontmatter.
    fn format_file(&self, content: &CommandContent) -> String;
}

/// Result of generating a command file.
#[derive(Debug, Clone)]
pub struct GeneratedCommand {
    /// File path from project root, or absolute for global-scoped command files
    pub path: String,
    /// Complete file content (frontmatter + body)
    pub file_content: String,
}

/// How a tool spells its Speckit commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandInvocationStyle {
    /// The command file lives in an `opsx/` subdirectory; the tool registers `opsx:<id>`
    Namespaced,
    /// The command file uses an `opsx-` filename prefix; the tool registers `opsx-<id>`
    Flat,
}

impl fmt::Display for CommandInvocationStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandInvocationStyle::Namespaced => write!(f, "namespaced"),
            CommandInvocationStyle::Flat => write!(f, "flat"),
        }
    }
}

/// Everything needed to spell one of a tool's Speckit commands.
#[derive(Debug, Clone)]
pub struct CommandInvocation {
    /// How the command file names the command.
    pub style: CommandInvocationStyle,
    /// What the user types before the name, e.g. `/` or Amazon Q's `@`.
    pub prefix: String,
}

/// The form these docs, command bodies, and skill templates are authored in.
pub const CANONICAL_INVOCATION: CommandInvocation = CommandInvocation {
    style: CommandInvocationStyle::Namespaced,
    prefix: String::new(), // Will be set via const fn workaround below
};

/// Returns the canonical invocation with "/" prefix.
pub fn canonical_invocation() -> CommandInvocation {
    CommandInvocation {
        style: CommandInvocationStyle::Namespaced,
        prefix: "/".to_string(),
    }
}

/// Command surface capability for a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSurfaceCapability {
    /// Tool has a command adapter registered
    AdapterBacked,
    /// Tool invokes skills directly (e.g., Codex)
    SkillsInvocable,
    /// Tool has no command surface
    None,
}

impl fmt::Display for CommandSurfaceCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandSurfaceCapability::AdapterBacked => write!(f, "adapter-backed"),
            CommandSurfaceCapability::SkillsInvocable => write!(f, "skills-invocable"),
            CommandSurfaceCapability::None => write!(f, "none"),
        }
    }
}

/// Delivery mode for commands and skills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    Both,
    Skills,
    Commands,
}
