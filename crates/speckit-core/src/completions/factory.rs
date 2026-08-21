use super::command_registry::CommandRegistry;
use super::provider::CompletionProvider;

/// Factory for creating completion providers.
pub struct CompletionFactory;

impl CompletionFactory {
    /// Create a new completion provider with default commands.
    pub fn create_provider() -> CompletionProvider {
        let mut registry = CommandRegistry::new();

        // Register main commands
        registry.register(super::command_registry::RegisteredCommand {
            name: "init".to_string(),
            description: "Initialize Speckit in your project".to_string(),
            subcommands: Vec::new(),
            options: Vec::new(),
        });

        registry.register(super::command_registry::RegisteredCommand {
            name: "list".to_string(),
            description: "List items (changes by default)".to_string(),
            subcommands: Vec::new(),
            options: Vec::new(),
        });

        registry.register(super::command_registry::RegisteredCommand {
            name: "show".to_string(),
            description: "Show a change or spec".to_string(),
            subcommands: Vec::new(),
            options: Vec::new(),
        });

        registry.register(super::command_registry::RegisteredCommand {
            name: "validate".to_string(),
            description: "Validate changes and specs".to_string(),
            subcommands: Vec::new(),
            options: Vec::new(),
        });

        registry.register(super::command_registry::RegisteredCommand {
            name: "archive".to_string(),
            description: "Archive a completed change".to_string(),
            subcommands: Vec::new(),
            options: Vec::new(),
        });

        CompletionProvider::new(registry)
    }
}
