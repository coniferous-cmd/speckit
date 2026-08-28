/// Fish completion generator.

pub struct FishGenerator;

impl FishGenerator {
    /// Generate fish completion script.
    pub fn generate(binary_name: &str) -> String {
        format!(
            r#"# Fish completion for {binary_name}

# Disable file completions by default
complete -c {binary_name} -f

# Subcommands
complete -c {binary_name} -n '__fish_use_subcommand' -a 'init' -d 'Initialize Speckit in your project'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'list' -d 'List items (changes by default)'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'show' -d 'Show a change or spec'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'validate' -d 'Validate changes and specs'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'archive' -d 'Archive a completed change'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'update' -d 'Update Speckit instruction files'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'status' -d 'Display artifact completion status'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'instructions' -d 'Output enriched instructions'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'templates' -d 'Show resolved template paths'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'schemas' -d 'List available workflow schemas'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'new' -d 'Create new items'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'completion' -d 'Manage shell completions'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'config' -d 'Manage configuration'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'schema' -d 'Manage schemas'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'store' -d 'Manage stores'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'doctor' -d 'Run diagnostics'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'context' -d 'Manage context'
complete -c {binary_name} -n '__fish_use_subcommand' -a 'workset' -d 'Manage worksets'

# Global options
complete -c {binary_name} -l 'no-color' -d 'Disable color output'
"#,
            binary_name = binary_name
        )
    }
}
