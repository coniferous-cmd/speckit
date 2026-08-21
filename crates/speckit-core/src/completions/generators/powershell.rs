/// PowerShell completion generator.

pub struct PowerShellGenerator;

impl PowerShellGenerator {
    /// Generate PowerShell completion script.
    pub fn generate(binary_name: &str) -> String {
        format!(
            r#"# PowerShell completion for {binary_name}

Register-ArgumentCompleter -Native -CommandName {binary_name} -ScriptBlock {{
    param($commandName, $wordToComplete, $cursorPosition)

    $commands = @(
        'init', 'list', 'show', 'validate', 'archive', 'update',
        'status', 'instructions', 'templates', 'schemas', 'new',
        'completion', 'config', 'schema', 'store', 'doctor',
        'context', 'workset', 'feedback'
    )

    $commands | Where-Object {{ $_ -like "$wordToComplete*" }} | ForEach-Object {{
        [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
    }}
}}
"#,
            binary_name = binary_name
        )
    }
}
