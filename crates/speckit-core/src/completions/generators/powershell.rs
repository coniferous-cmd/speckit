//! PowerShell completion generator.

pub struct PowerShellGenerator;

impl PowerShellGenerator {
    /// Generate a native PowerShell completer with dynamic project values.
    pub fn generate(binary_name: &str) -> String {
        format!(
            r#"# PowerShell completion for {binary_name}
# Auto-generated - do not edit manually

function Get-{binary_name}CompletionData([string] $Type) {{
    & {binary_name} __complete $Type 2>$null | ForEach-Object {{
        $parts = $_ -split "`t", 2
        if ($parts.Count -gt 0 -and $parts[0]) {{
            [PSCustomObject]@{{ Name = $parts[0]; Description = if ($parts.Count -gt 1) {{ $parts[1] }} else {{ $parts[0] }} }}
        }}
    }}
}}

Register-ArgumentCompleter -Native -CommandName {binary_name} -ScriptBlock {{
    param($commandName, $wordToComplete, $commandAst, $cursorPosition)
    $tokens = @($commandAst.CommandElements | ForEach-Object {{ $_.Extent.Text }})
    $args = if ($tokens.Count -gt 1) {{ @($tokens[1..($tokens.Count - 1)]) }} else {{ @() }}
    $current = $wordToComplete

    function Complete-Items($Items, [string] $ResultType = "ParameterValue") {{
        $Items | Where-Object {{ $_.Name -like "$current*" }} | ForEach-Object {{
            [System.Management.Automation.CompletionResult]::new($_.Name, $_.Name, $ResultType, $_.Description)
        }}
    }}

    $commands = @(
        @{{Name="init";Description="Initialize Speckit in your project"}}
        @{{Name="update";Description="Update Speckit instruction files"}}
        @{{Name="list";Description="List items"}}
        @{{Name="view";Description="Display an interactive dashboard"}}
        @{{Name="change";Description="Manage change proposals"}}
        @{{Name="archive";Description="Archive a completed change"}}
        @{{Name="validate";Description="Validate changes and specs"}}
        @{{Name="show";Description="Show a change or spec"}}
        @{{Name="feedback";Description="Submit feedback"}}
        @{{Name="completion";Description="Manage shell completions"}}
        @{{Name="status";Description="Display artifact completion status"}}
        @{{Name="instructions";Description="Output enriched instructions"}}
        @{{Name="templates";Description="Show resolved template paths"}}
        @{{Name="schemas";Description="List available workflow schemas"}}
        @{{Name="new";Description="Create new items"}}
        @{{Name="spec";Description="Manage specifications"}}
        @{{Name="config";Description="Manage configuration"}}
        @{{Name="schema";Description="Manage schemas"}}
        @{{Name="store";Description="Manage stores"}}
        @{{Name="doctor";Description="Run diagnostics"}}
        @{{Name="context";Description="Print the working context"}}
        @{{Name="workset";Description="Manage worksets"}}
    )
    if ($args.Count -eq 0) {{ Complete-Items $commands; return }}

    $command = $args[0]
    if ($args.Count -eq 1 -and $current -notlike "-*") {{
        $subcommands = switch ($command) {{
            "completion" {{ @("generate", "install", "uninstall") | ForEach-Object {{ [PSCustomObject]@{{Name=$_;Description="Completion $_"}} }} }}
            "change" {{ @("show", "list", "validate") | ForEach-Object {{ [PSCustomObject]@{{Name=$_;Description="Change $_"}} }} }}
            "spec" {{ @("show", "list", "validate") | ForEach-Object {{ [PSCustomObject]@{{Name=$_;Description="Spec $_"}} }} }}
            "new" {{ @([PSCustomObject]@{{Name="change";Description="Create a change"}}) }}
            "schema" {{ @("which", "validate", "fork", "init") | ForEach-Object {{ [PSCustomObject]@{{Name=$_;Description="Schema $_"}} }} }}
            "store" {{ @("setup", "register", "unregister", "remove", "list", "doctor") | ForEach-Object {{ [PSCustomObject]@{{Name=$_;Description="Store $_"}} }} }}
            "workset" {{ @("create", "list", "open", "remove") | ForEach-Object {{ [PSCustomObject]@{{Name=$_;Description="Workset $_"}} }} }}
        }}
        if ($subcommands) {{ Complete-Items $subcommands; return }}
    }}
    if ($current -like "-*" ) {{
        Complete-Items @(@{{Name="--help";Description="Show help"}}, @{{Name="--version";Description="Show version"}}, @{{Name="--no-color";Description="Disable color output"}}, @{{Name="--json";Description="Output as JSON"}}, @{{Name="--store";Description="Store ID"}}) "ParameterName"
        return
    }}
    if ($command -in @("show", "validate", "archive")) {{
        Complete-Items (Get-{binary_name}CompletionData $(if ($command -eq "show" -and $args -contains "spec") {{ "specs" }} else {{ "changes" }}))
        return
    }}
    if ($command -eq "completion" -and $args.Count -ge 2) {{
        Complete-Items (@("bash", "zsh", "fish", "powershell") | ForEach-Object {{ [PSCustomObject]@{{Name=$_;Description="Shell: $_"}} }})
    }}
}}
"#,
            binary_name = binary_name
        )
    }
}
