use super::command_registry::CommandRegistry;
use super::types::{CompletionContext, CompletionItem, CompletionKind};

/// Provides completions for shell input.
pub struct CompletionProvider {
    registry: CommandRegistry,
}

impl CompletionProvider {
    pub fn new(registry: CommandRegistry) -> Self {
        Self { registry }
    }

    /// Get completions for the given context.
    pub fn get_completions(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        // If we're at the start, suggest commands
        if context.previous_word.is_none() {
            for cmd in self.registry.get_commands() {
                items.push(CompletionItem {
                    value: cmd.name.clone(),
                    description: Some(cmd.description.clone()),
                    kind: CompletionKind::Command,
                });
            }
        }

        items
    }
}
