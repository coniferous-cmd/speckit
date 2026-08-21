//! Command Generator
//!
//! Functions for generating command files using tool adapters.

use super::invocation::{
    get_invocation_for_adapter, needs_invocation_rewrite, transform_command_invocations,
};
use super::types::{CommandContent, GeneratedCommand, ToolCommandAdapter};

/// Generate a single command file using the provided adapter.
///
/// Command bodies are authored with `/opsx:<id>` references. Tools whose command
/// files are invoked by filename register `/opsx-<id>` instead, and Amazon Q
/// surfaces them in its prompt library as `@opsx-<id>`, so the body is rewritten
/// to the form that tool answers to before the adapter formats it. Doing it here
/// rather than per adapter keeps every tool in step; adapters stay pure formatters.
pub fn generate_command(
    content: &CommandContent,
    adapter: &dyn ToolCommandAdapter,
) -> GeneratedCommand {
    let invocation = get_invocation_for_adapter(adapter);

    let formatted = if needs_invocation_rewrite(&invocation) {
        let mut rewritten = content.clone();
        rewritten.body = transform_command_invocations(&content.body, &invocation);
        rewritten
    } else {
        content.clone()
    };

    GeneratedCommand {
        path: adapter.get_file_path(&content.id),
        file_content: adapter.format_file(&formatted),
    }
}

/// Generate multiple command files using the provided adapter.
pub fn generate_commands(
    contents: &[CommandContent],
    adapter: &dyn ToolCommandAdapter,
) -> Vec<GeneratedCommand> {
    contents
        .iter()
        .map(|content| generate_command(content, adapter))
        .collect()
}
