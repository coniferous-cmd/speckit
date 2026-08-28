//! Completion Command
//!
//! Manage shell completions for the Speckit CLI.

/// Supported shells for completion generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedShell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

impl SupportedShell {
    pub fn as_str(&self) -> &'static str {
        match self {
            SupportedShell::Bash => "bash",
            SupportedShell::Zsh => "zsh",
            SupportedShell::Fish => "fish",
            SupportedShell::PowerShell => "powershell",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "bash" => Some(SupportedShell::Bash),
            "zsh" => Some(SupportedShell::Zsh),
            "fish" => Some(SupportedShell::Fish),
            "powershell" | "pwsh" => Some(SupportedShell::PowerShell),
            _ => None,
        }
    }

    pub fn all() -> &'static [SupportedShell] {
        &[
            SupportedShell::Bash,
            SupportedShell::Zsh,
            SupportedShell::Fish,
            SupportedShell::PowerShell,
        ]
    }
}

/// Detect the current shell from the environment.
fn detect_shell() -> Option<SupportedShell> {
    let shell = std::env::var("SHELL").ok()?;
    if shell.contains("bash") {
        Some(SupportedShell::Bash)
    } else if shell.contains("zsh") {
        Some(SupportedShell::Zsh)
    } else if shell.contains("fish") {
        Some(SupportedShell::Fish)
    } else if shell.contains("powershell") || shell.contains("pwsh") {
        Some(SupportedShell::PowerShell)
    } else {
        None
    }
}

/// Resolve the shell parameter, or auto-detect.
fn resolve_shell(shell: Option<&str>) -> anyhow::Result<SupportedShell> {
    if let Some(s) = shell {
        return SupportedShell::from_str(s).ok_or_else(|| {
            anyhow::anyhow!(
                "Shell '{s}' is not supported. Supported shells: {}",
                SupportedShell::all()
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        });
    }

    detect_shell().ok_or_else(|| {
        anyhow::anyhow!(
            "Could not auto-detect shell. Please specify shell explicitly.\nUsage: speckit completion generate [shell]\nSupported shells: {}",
            SupportedShell::all()
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

/// Generate completion script for a shell.
pub async fn completion_generate(shell: Option<&str>) -> anyhow::Result<()> {
    let resolved = resolve_shell(shell)?;
    let script = generate_completion_script(resolved);
    println!("{script}");
    Ok(())
}

/// Install completion script for a shell.
pub async fn completion_install(shell: Option<&str>, verbose: bool) -> anyhow::Result<()> {
    let resolved = resolve_shell(shell)?;

    match resolved {
        SupportedShell::Bash => install_bash(verbose)?,
        SupportedShell::Zsh => install_zsh(verbose)?,
        SupportedShell::Fish => install_fish(verbose)?,
        SupportedShell::PowerShell => install_powershell(verbose)?,
    }

    Ok(())
}

/// Uninstall completion script for a shell.
pub async fn completion_uninstall(shell: Option<&str>, yes: bool) -> anyhow::Result<()> {
    let resolved = resolve_shell(shell)?;

    if !yes && atty_is_tty() {
        let config_path = match resolved {
            SupportedShell::Bash => "~/.bashrc",
            SupportedShell::Zsh => "~/.zshrc",
            SupportedShell::Fish => "~/.config/fish/config.fish",
            SupportedShell::PowerShell => "$PROFILE",
        };
        let confirmed =
            inquire::Confirm::new(&format!("Remove Speckit configuration from {config_path}?"))
                .with_default(false)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Prompt cancelled: {e}"))?;

        if !confirmed {
            println!("Uninstall cancelled.");
            return Ok(());
        }
    }

    match resolved {
        SupportedShell::Bash => uninstall_bash()?,
        SupportedShell::Zsh => uninstall_zsh()?,
        SupportedShell::Fish => uninstall_fish()?,
        SupportedShell::PowerShell => uninstall_powershell()?,
    }

    Ok(())
}

/// Output machine-readable completion data.
pub async fn completion_complete(completion_type: &str) -> anyhow::Result<()> {
    match completion_type.to_lowercase().as_str() {
        "changes" => {
            let project_root = std::env::current_dir()?.to_string_lossy().to_string();
            let changes = crate::change::get_active_change_ids(&project_root).await?;
            for id in changes {
                println!("{id}\tactive change");
            }
        }
        "specs" => {
            let project_root = std::env::current_dir()?.to_string_lossy().to_string();
            let specs = crate::spec::get_spec_ids(&project_root).await?;
            for id in specs {
                println!("{id}\tspecification");
            }
        }
        "schemas" => {
            let project_root = std::env::current_dir()?.to_string_lossy().to_string();
            let schemas = crate::workflow::shared::list_schemas(&project_root);
            for name in schemas {
                println!("{name}\tschema");
            }
        }
        _ => {
            std::process::exit(1);
        }
    }
    Ok(())
}

fn generate_completion_script(shell: SupportedShell) -> String {
    match shell {
        SupportedShell::Bash => {
            r#"#!/bin/bash
_speckit_completions()
{
    local cur prev commands
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    commands="init update list view change archive validate show completion status instructions templates schemas new spec config schema store doctor context workset"

    if [[ ${cur} == -* ]] ; then
        COMPREPLY=( $(compgen -W "--help --version --no-color" -- ${cur}) )
        return 0
    fi

    COMPREPLY=( $(compgen -W "${commands}" -- ${cur}) )
    return 0
}
complete -F _speckit_completions speckit
"#
            .to_string()
        }
        SupportedShell::Zsh => {
            r#"#compdef speckit

_speckit() {
    _arguments \
        '1:command:(init update list view change archive validate show completion status instructions templates schemas new spec config schema store doctor context workset)' \
        '*::arg:->args'
}

_speckit "$@"
"#
            .to_string()
        }
        SupportedShell::Fish => {
            r#"complete -c speckit -f
complete -c speckit -n '__fish_use_subcommand' -a init -d 'Initialize Speckit in your project'
complete -c speckit -n '__fish_use_subcommand' -a update -d 'Update Speckit instruction files'
complete -c speckit -n '__fish_use_subcommand' -a list -d 'List items'
complete -c speckit -n '__fish_use_subcommand' -a view -d 'Display an interactive dashboard'
complete -c speckit -n '__fish_use_subcommand' -a validate -d 'Validate changes and specs'
complete -c speckit -n '__fish_use_subcommand' -a show -d 'Show a change or spec'
complete -c speckit -n '__fish_use_subcommand' -a status -d 'Display artifact completion status'
complete -c speckit -n '__fish_use_subcommand' -a doctor -d 'Report relationship health'
complete -c speckit -n '__fish_use_subcommand' -a context -d 'Print the working context'
"#
            .to_string()
        }
        SupportedShell::PowerShell => speckit_core::completions::generators::PowerShellGenerator::generate("speckit"),
    }
}

fn install_bash(verbose: bool) -> anyhow::Result<()> {
    let script = generate_completion_script(SupportedShell::Bash);
    let completions_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".local")
        .join("share")
        .join("speckit")
        .join("completions");
    std::fs::create_dir_all(&completions_dir)?;
    let script_path = completions_dir.join("speckit.bash");
    std::fs::write(&script_path, script)?;

    // Add to .bashrc if not already there
    let bashrc = dirs::home_dir().unwrap().join(".bashrc");
    let source_line = format!("source {}", script_path.display());
    if bashrc.exists() {
        let content = std::fs::read_to_string(&bashrc)?;
        if !content.contains(&source_line) {
            std::fs::write(&bashrc, format!("{content}\n{source_line}\n"))?;
        }
    }

    println!("\u{2713} Bash completion script installed");
    if verbose {
        println!("  Installed to: {}", script_path.display());
        println!("  ~/.bashrc configured automatically");
    }
    println!();
    println!("Restart your shell or run: exec bash");
    Ok(())
}

fn install_zsh(verbose: bool) -> anyhow::Result<()> {
    let script = generate_completion_script(SupportedShell::Zsh);
    let completions_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".local")
        .join("share")
        .join("speckit")
        .join("completions");
    std::fs::create_dir_all(&completions_dir)?;
    let script_path = completions_dir.join("_speckit");
    std::fs::write(&script_path, script)?;

    // Add to .zshrc if not already there
    let zshrc = dirs::home_dir().unwrap().join(".zshrc");
    let fpath_line = format!("fpath=({} $fpath)", completions_dir.display());
    if zshrc.exists() {
        let content = std::fs::read_to_string(&zshrc)?;
        if !content.contains(&fpath_line) {
            std::fs::write(
                &zshrc,
                format!("{content}\n{fpath_line}\nautoload -Uz compinit && compinit\n"),
            )?;
        }
    }

    println!("\u{2713} Zsh completion script installed");
    if verbose {
        println!("  Installed to: {}", script_path.display());
        println!("  ~/.zshrc configured automatically");
    }
    println!();
    println!("Restart your shell or run: exec zsh");
    Ok(())
}

fn install_fish(verbose: bool) -> anyhow::Result<()> {
    let script = generate_completion_script(SupportedShell::Fish);
    let completions_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".config")
        .join("fish")
        .join("completions");
    std::fs::create_dir_all(&completions_dir)?;
    let script_path = completions_dir.join("speckit.fish");
    std::fs::write(&script_path, script)?;

    println!("\u{2713} Fish completion script installed");
    if verbose {
        println!("  Installed to: {}", script_path.display());
    }
    println!();
    println!("Restart your shell or run: exec fish");
    Ok(())
}

fn uninstall_bash() -> anyhow::Result<()> {
    let completions_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".local")
        .join("share")
        .join("speckit")
        .join("completions");
    let script_path = completions_dir.join("speckit.bash");
    if script_path.exists() {
        std::fs::remove_file(&script_path)?;
    }

    // Remove from .bashrc
    let bashrc = dirs::home_dir().unwrap().join(".bashrc");
    if bashrc.exists() {
        let content = std::fs::read_to_string(&bashrc)?;
        let source_line = format!("source {}", script_path.display());
        let new_content = content.replace(&source_line, "");
        std::fs::write(&bashrc, new_content)?;
    }

    println!("\u{2713} Bash completion uninstalled");
    Ok(())
}

fn uninstall_zsh() -> anyhow::Result<()> {
    let completions_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".local")
        .join("share")
        .join("speckit")
        .join("completions");
    let script_path = completions_dir.join("_speckit");
    if script_path.exists() {
        std::fs::remove_file(&script_path)?;
    }

    // Remove from .zshrc
    let zshrc = dirs::home_dir().unwrap().join(".zshrc");
    if zshrc.exists() {
        let content = std::fs::read_to_string(&zshrc)?;
        let fpath_line = format!("fpath=({} $fpath)", completions_dir.display());
        let new_content = content
            .replace(&fpath_line, "")
            .replace("autoload -Uz compinit && compinit", "");
        std::fs::write(&zshrc, new_content)?;
    }

    println!("\u{2713} Zsh completion uninstalled");
    Ok(())
}

fn uninstall_fish() -> anyhow::Result<()> {
    let completions_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".config")
        .join("fish")
        .join("completions");
    let script_path = completions_dir.join("speckit.fish");
    if script_path.exists() {
        std::fs::remove_file(&script_path)?;
    }

    println!("\u{2713} Fish completion uninstalled");
    Ok(())
}

fn powershell_profile_path() -> anyhow::Result<std::path::PathBuf> {
    if let Ok(profile) = std::env::var("PROFILE") {
        return Ok(std::path::PathBuf::from(profile));
    }
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let relative = if cfg!(windows) {
        "Documents/WindowsPowerShell/Microsoft.PowerShell_profile.ps1"
    } else {
        ".config/powershell/Microsoft.PowerShell_profile.ps1"
    };
    Ok(home.join(relative))
}

fn install_powershell(verbose: bool) -> anyhow::Result<()> {
    let profile = powershell_profile_path()?;
    if let Some(parent) = profile.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let marker_start = "# >>> speckit completion >>>";
    let marker_end = "# <<< speckit completion <<<";
    let block = format!(
        "{marker_start}\n{}\n{marker_end}\n",
        generate_completion_script(SupportedShell::PowerShell)
    );
    let content = if profile.exists() {
        std::fs::read_to_string(&profile)?
    } else {
        String::new()
    };
    let content = remove_marked_block(&content, marker_start, marker_end);
    std::fs::write(&profile, format!("{content}{block}"))?;
    println!("\u{2713} PowerShell completion installed");
    if verbose {
        println!("  Installed to: {}", profile.display());
    }
    println!("Reload with: . $PROFILE");
    Ok(())
}

fn uninstall_powershell() -> anyhow::Result<()> {
    let profile = powershell_profile_path()?;
    if profile.exists() {
        let content = std::fs::read_to_string(&profile)?;
        let content = remove_marked_block(
            &content,
            "# >>> speckit completion >>>",
            "# <<< speckit completion <<<",
        );
        std::fs::write(profile, content)?;
    }
    println!("\u{2713} PowerShell completion uninstalled");
    Ok(())
}

fn remove_marked_block(content: &str, start: &str, end: &str) -> String {
    if let (Some(begin), Some(finish)) = (content.find(start), content.find(end)) {
        let finish = finish + end.len();
        let mut result = String::with_capacity(content.len());
        result.push_str(&content[..begin]);
        result.push_str(&content[finish..]);
        result
    } else {
        content.to_string()
    }
}

fn atty_is_tty() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}
