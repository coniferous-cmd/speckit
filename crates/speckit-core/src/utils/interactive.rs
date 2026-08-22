use std::io::{self, BufRead, IsTerminal, Write};

/// Options controlling interactive mode.
#[derive(Debug, Clone, Default)]
pub struct InteractiveOptions {
    /// Explicit "disable prompts" flag passed by internal callers.
    pub no_interactive: bool,
    /// Commander-style negated option: `--no-interactive` sets this to `false`.
    pub interactive: Option<bool>,
}

impl InteractiveOptions {
    /// Resolves whether non-interactive mode is requested.
    pub fn resolve_no_interactive(&self) -> bool {
        if self.no_interactive {
            return true;
        }
        self.interactive == Some(false)
    }
}

/// Returns `true` when the CLI should present interactive prompts.
///
/// Checks, in order:
/// 1. Explicit `noInteractive` / `--no-interactive` flags.
/// 2. `OPEN_SPEC_INTERACTIVE=0` environment variable.
/// 3. Presence of `CI` environment variable.
/// 4. Whether stdin is a TTY.
pub fn is_interactive(options: Option<&InteractiveOptions>) -> bool {
    if let Some(opts) = options
        && opts.resolve_no_interactive() {
            return false;
        }

    if let Ok(val) = std::env::var("OPEN_SPEC_INTERACTIVE")
        && val == "0" {
            return false;
        }

    // Respect standard CI environment variable.
    if std::env::var("CI").is_ok() {
        return false;
    }

    io::stdin().is_terminal()
}

/// Returns `true` when a prompt failed because no answer could be read.
///
/// This classifies a prompt that has *already failed*, so piped answers are
/// unaffected. It is not a substitute for `is_interactive()`.
pub fn is_non_interactive_prompt_error(error: &str, options: Option<&InteractiveOptions>) -> bool {
    let failed_prompt = error.contains("force closed the prompt")
        || error.contains("ExitPromptError")
        || error.contains("broken pipe")
        || error.contains("unexpected end of file");

    if !failed_prompt {
        return false;
    }

    if error.contains("SIGINT") {
        return false;
    }

    !is_interactive(options)
}

/// Ask a yes/no question. When stdin is a TTY and interactive, uses a rich
/// prompt; otherwise reads one plain line from stdin.
///
/// Returns `true` for yes, `false` for no.
pub fn confirm_prompt(message: &str, default: bool) -> io::Result<bool> {
    let stdin = io::stdin();
    let stdout = io::stdout();

    let is_terminal = stdin.is_terminal() && stdout.is_terminal();

    if is_terminal {
        confirm_prompt_tty(message, default)
    } else {
        confirm_prompt_plain(message, default)
    }
}

/// Rich interactive confirmation for TTY terminals.
fn confirm_prompt_tty(message: &str, default: bool) -> io::Result<bool> {
    let hint = if default { "(Y/n)" } else { "(y/N)" };
    let mut stdout = io::stdout();

    write!(stdout, "{message} {hint} ")?;
    stdout.flush()?;

    let stdin = io::stdin();
    let mut line = String::new();
    let mut handle = stdin.lock();

    match handle.read_line(&mut line) {
        Ok(0) => {
            // EOF
            Ok(default)
        }
        Ok(_) => parse_yes_no(&line, default),
        Err(e) => Err(e),
    }
}

/// Plain confirmation for non-TTY (piped) input.
fn confirm_prompt_plain(message: &str, default: bool) -> io::Result<bool> {
    let hint = if default { "(Y/n)" } else { "(y/N)" };
    let mut stdout = io::stdout();

    write!(stdout, "{message} {hint} ")?;
    stdout.flush()?;

    let stdin = io::stdin();
    let mut line = String::new();
    let mut handle = stdin.lock();

    match handle.read_line(&mut line) {
        Ok(0) => {
            // EOF -- no line could be read.
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "User force closed the prompt",
            ))
        }
        Ok(_) => parse_yes_no(&line, default),
        Err(e) => Err(e),
    }
}

/// Parse a yes/no answer. Mirrors @inquirer/confirm's parser (prefix match on
/// y/yes and n/no, otherwise the default).
fn parse_yes_no(line: &str, default: bool) -> io::Result<bool> {
    let trimmed = line.trim().to_lowercase();
    if trimmed.starts_with('y') || trimmed.starts_with("yes") {
        Ok(true)
    } else if trimmed.starts_with('n') || trimmed.starts_with("no") {
        Ok(false)
    } else {
        Ok(default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_no_interactive_default() {
        let opts = InteractiveOptions::default();
        assert!(!opts.resolve_no_interactive());
    }

    #[test]
    fn resolve_no_interactive_explicit() {
        let opts = InteractiveOptions {
            no_interactive: true,
            ..Default::default()
        };
        assert!(opts.resolve_no_interactive());
    }

    #[test]
    fn resolve_no_interactive_commander_style() {
        let opts = InteractiveOptions {
            interactive: Some(false),
            ..Default::default()
        };
        assert!(opts.resolve_no_interactive());
    }

    #[test]
    fn parse_yes_no_yes() {
        assert!(parse_yes_no("y", false).unwrap());
        assert!(parse_yes_no("yes", false).unwrap());
        assert!(parse_yes_no("Yes\n", false).unwrap());
        assert!(parse_yes_no("yep", false).unwrap());
    }

    #[test]
    fn parse_yes_no_no() {
        assert!(!parse_yes_no("n", true).unwrap());
        assert!(!parse_yes_no("no", true).unwrap());
        assert!(!parse_yes_no("No\n", true).unwrap());
    }

    #[test]
    fn parse_yes_no_default() {
        assert!(parse_yes_no("", true).unwrap());
        assert!(!parse_yes_no("", false).unwrap());
        assert!(parse_yes_no("\n", true).unwrap());
    }

    #[test]
    fn is_non_interactive_prompt_error_matching() {
        assert!(is_non_interactive_prompt_error(
            "User force closed the prompt",
            None
        ));
    }

    #[test]
    fn is_non_interactive_prompt_error_non_matching() {
        assert!(!is_non_interactive_prompt_error("some other error", None));
    }

    #[test]
    fn is_non_interactive_prompt_error_sigint() {
        assert!(!is_non_interactive_prompt_error(
            "force closed the prompt with SIGINT",
            None
        ));
    }
}
