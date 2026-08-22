//! Command Adapter Registry
//!
//! Centralized registry for tool command adapters.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::adapters::*;
use super::types::ToolCommandAdapter;

/// Registry for looking up tool command adapters.
pub struct CommandAdapterRegistry {
    adapters: HashMap<String, Box<dyn ToolCommandAdapter + Send + Sync>>,
}

static GLOBAL_REGISTRY: OnceLock<CommandAdapterRegistry> = OnceLock::new();

impl Default for CommandAdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandAdapterRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Register a tool command adapter.
    pub fn register(&mut self, adapter: Box<dyn ToolCommandAdapter + Send + Sync>) {
        self.adapters.insert(adapter.tool_id().to_string(), adapter);
    }

    /// Get an adapter by tool ID.
    pub fn get(&self, tool_id: &str) -> Option<&(dyn ToolCommandAdapter + Send + Sync)> {
        self.adapters.get(tool_id).map(|a| a.as_ref())
    }

    /// Get all registered adapters.
    pub fn get_all(&self) -> Vec<&(dyn ToolCommandAdapter + Send + Sync)> {
        self.adapters.values().map(|a| a.as_ref()).collect()
    }

    /// Check if an adapter is registered for a tool.
    pub fn has(&self, tool_id: &str) -> bool {
        self.adapters.contains_key(tool_id)
    }

    /// Returns a registry with ALL 30 built-in adapters pre-registered.
    pub fn with_all_adapters() -> Self {
        let mut registry = Self::new();

        registry.register(Box::new(AmazonQAdapter));
        registry.register(Box::new(AntigravityAdapter));
        registry.register(Box::new(AuggieAdapter));
        registry.register(Box::new(BobAdapter));
        registry.register(Box::new(ClaudeAdapter));
        registry.register(Box::new(ClineAdapter));
        registry.register(Box::new(CodebuddyAdapter));
        registry.register(Box::new(CommandCodeAdapter));
        registry.register(Box::new(ContinueAdapter));
        registry.register(Box::new(CostrictAdapter));
        registry.register(Box::new(CrushAdapter));
        registry.register(Box::new(CursorAdapter));
        registry.register(Box::new(DevinAdapter));
        registry.register(Box::new(FactoryAdapter));
        registry.register(Box::new(GeminiAdapter));
        registry.register(Box::new(GithubCopilotAdapter));
        registry.register(Box::new(IflowAdapter));
        registry.register(Box::new(JunieAdapter));
        registry.register(Box::new(KilocodeAdapter));
        registry.register(Box::new(KiroAdapter));
        registry.register(Box::new(LingmaAdapter));
        registry.register(Box::new(OhMyPiAdapter));
        registry.register(Box::new(OpencodeAdapter));
        registry.register(Box::new(PiAdapter));
        registry.register(Box::new(QoderAdapter));
        registry.register(Box::new(QwenAdapter));
        registry.register(Box::new(RoocodeAdapter));
        registry.register(Box::new(TraeAdapter));
        registry.register(Box::new(ZcodeAdapter));

        registry
    }

    /// Returns the global registry singleton with all 30 adapters pre-registered.
    pub fn global() -> &'static Self {
        GLOBAL_REGISTRY.get_or_init(Self::with_all_adapters)
    }
}
