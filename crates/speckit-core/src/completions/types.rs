/// Completion types for shell completions.

/// A completion item.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub value: String,
    pub description: Option<String>,
    pub kind: CompletionKind,
}

/// The kind of completion item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionKind {
    Command,
    Option,
    Argument,
    Value,
}

/// Completion context.
#[derive(Debug, Clone)]
pub struct CompletionContext {
    pub current_word: String,
    pub previous_word: Option<String>,
    pub line: String,
    pub position: usize,
}
