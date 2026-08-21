/// Zsh completion generator.

pub struct ZshGenerator;

impl ZshGenerator {
    /// Generate zsh completion script.
    pub fn generate(binary_name: &str) -> String {
        format!(
            r#"#compdef {binary_name}

_{binary_name}() {{
    local -a commands
    commands=(
        'init:Initialize Speckit in your project'
        'list:List items (changes by default)'
        'show:Show a change or spec'
        'validate:Validate changes and specs'
        'archive:Archive a completed change'
        'update:Update Speckit instruction files'
        'status:Display artifact completion status'
        'instructions:Output enriched instructions'
        'templates:Show resolved template paths'
        'schemas:List available workflow schemas'
        'new:Create new items'
        'completion:Manage shell completions'
        'config:Manage configuration'
        'schema:Manage schemas'
        'store:Manage stores'
        'doctor:Run diagnostics'
        'context:Manage context'
        'workset:Manage worksets'
        'feedback:Submit feedback'
    )

    _arguments -C \
        '1: :->command' \
        '*: :->args'

    case $state in
        command)
            _describe 'command' commands
            ;;
    esac
}}

_{binary_name} "$@"
"#,
            binary_name = binary_name
        )
    }
}
