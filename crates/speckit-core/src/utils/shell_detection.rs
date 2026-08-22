use std::env;

/// Detected shell type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
    Unknown,
}

/// Detect the current shell.
pub fn detect_shell() -> Shell {
    // Check SHELL environment variable (Unix)
    if let Ok(shell) = env::var("SHELL") {
        if shell.contains("bash") {
            return Shell::Bash;
        }
        if shell.contains("zsh") {
            return Shell::Zsh;
        }
        if shell.contains("fish") {
            return Shell::Fish;
        }
    }

    // Check PSModulePath (PowerShell)
    if env::var("PSModulePath").is_ok() {
        return Shell::PowerShell;
    }

    // Check COMSPEC (Windows)
    if let Ok(comspec) = env::var("COMSPEC")
        && comspec.contains("cmd.exe")
    {
        return Shell::Cmd;
    }

    Shell::Unknown
}

/// Get the shell name as a string.
pub fn shell_name(shell: &Shell) -> &'static str {
    match shell {
        Shell::Bash => "bash",
        Shell::Zsh => "zsh",
        Shell::Fish => "fish",
        Shell::PowerShell => "powershell",
        Shell::Cmd => "cmd",
        Shell::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell() {
        // In test environment, shell detection will return whatever is set
        let shell = detect_shell();
        // Just verify it doesn't panic
        let _ = shell_name(&shell);
    }
}
