//! Command Generation Module
//!
//! Generic command generation system with tool-specific adapters.
//!
//! Usage:
//! ```rust
//! use speckit_core::command_generation::{
//!     generate_commands, CommandAdapterRegistry, CommandContent,
//! };
//!
//! let contents: Vec<CommandContent> = vec![/* ... */];
//! let registry = CommandAdapterRegistry::global();
//! if let Some(adapter) = registry.get("cursor") {
//!     let commands = generate_commands(&contents, adapter);
//!     // Write commands to disk
//! }
//! ```

pub mod adapters;
pub mod generator;
pub mod invocation;
pub mod registry;
pub mod types;
pub mod yaml;

// Re-export core types
pub use types::{
    CommandContent, CommandInvocation, CommandInvocationStyle, CommandSurfaceCapability, Delivery,
    GeneratedCommand, ToolCommandAdapter,
};

// Re-export registry
pub use registry::CommandAdapterRegistry;

// Re-export generator functions
pub use generator::{generate_command, generate_commands};

// Re-export invocation helpers
pub use invocation::{
    format_command_invocation, get_invocation_for_adapter, get_invocation_style_for_path,
    needs_invocation_rewrite, transform_command_invocations,
};

// Re-export YAML helpers
pub use yaml::{escape_yaml_value, format_tags_array};

// Re-export adapter structs for direct access
pub use adapters::{
    AmazonQAdapter, AntigravityAdapter, AuggieAdapter, BobAdapter, ClaudeAdapter, ClineAdapter,
    CodebuddyAdapter, CommandCodeAdapter, ContinueAdapter, CostrictAdapter, CrushAdapter,
    CursorAdapter, DevinAdapter, FactoryAdapter, GeminiAdapter, GithubCopilotAdapter, IflowAdapter,
    JunieAdapter, KilocodeAdapter, KiroAdapter, LingmaAdapter, OhMyPiAdapter, OpencodeAdapter,
    PiAdapter, QoderAdapter, QwenAdapter, RoocodeAdapter, TraeAdapter, ZcodeAdapter,
};

/// Resolves how a tool's generated commands are invoked.
/// Returns None for tools with no command adapter.
pub fn resolve_command_invocation(tool_id: &str) -> Option<CommandInvocation> {
    let registry = CommandAdapterRegistry::global();
    registry
        .get(tool_id)
        .map(|adapter| get_invocation_for_adapter(adapter))
}

/// Resolves the command surface capability for a tool.
pub fn resolve_command_surface_capability(tool_id: &str) -> CommandSurfaceCapability {
    let registry = CommandAdapterRegistry::global();
    if registry.has(tool_id) {
        CommandSurfaceCapability::AdapterBacked
    } else if tool_id == "codex" {
        CommandSurfaceCapability::SkillsInvocable
    } else {
        CommandSurfaceCapability::None
    }
}

/// Whether skills should be generated for a tool given the delivery mode.
pub fn should_generate_skills_for_tool(tool_id: &str, delivery: Delivery) -> bool {
    delivery != Delivery::Commands
        || resolve_command_surface_capability(tool_id) == CommandSurfaceCapability::SkillsInvocable
}

/// Whether skills should be removed for a tool given the delivery mode.
pub fn should_remove_skills_for_tool(tool_id: &str, delivery: Delivery) -> bool {
    delivery == Delivery::Commands
        && resolve_command_surface_capability(tool_id) != CommandSurfaceCapability::SkillsInvocable
}

/// Whether commands should be generated for a tool given the delivery mode.
pub fn should_generate_commands_for_tool(tool_id: &str, delivery: Delivery) -> bool {
    delivery != Delivery::Skills
        && resolve_command_surface_capability(tool_id) == CommandSurfaceCapability::AdapterBacked
}

/// Whether command files should be reconciled for a tool given the delivery mode.
pub fn should_reconcile_command_files_for_tool(tool_id: &str, delivery: Delivery) -> bool {
    delivery == Delivery::Skills
        && resolve_command_surface_capability(tool_id) == CommandSurfaceCapability::AdapterBacked
}
