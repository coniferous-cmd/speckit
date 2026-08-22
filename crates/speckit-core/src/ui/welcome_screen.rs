use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use colored::Colorize;

use super::ascii_patterns::welcome_animation;

/// Minimum terminal width for side-by-side layout.
const MIN_WIDTH: usize = 60;

/// Width of the ASCII art column (with padding).
const ART_COLUMN_WIDTH: usize = 24;

/// Builds the welcome text content (right column).
fn get_welcome_text(workflows: &[String]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    lines.push("Welcome to Speckit".white().bold().to_string());
    lines.push("A lightweight spec-driven framework".dimmed().to_string());
    lines.push(String::new());
    lines.push("This setup will configure:".white().to_string());
    lines.push("  * Agent Skills for AI tools".dimmed().to_string());
    lines.push("  * Workflow commands, if supported".dimmed().to_string());

    if !workflows.is_empty() {
        lines.push(String::new());
        lines.push("Quick start after setup:".white().to_string());
        let max_len = workflows.iter().map(|w| w.len()).max().unwrap_or(0);
        for workflow in workflows {
            lines.push(format!(
                "  {} {}",
                workflow.yellow(),
                " ".repeat(max_len - workflow.len() + 1)
            ));
        }
        lines.push("  (spelling varies by tool)".dimmed().to_string());
    }

    lines.push(String::new());
    lines.push("Press Enter to select tools...".cyan().to_string());

    lines
}

/// Renders a single frame with side-by-side layout.
fn render_frame(art_lines: &[String], text_lines: &[String]) -> String {
    let max_lines = art_lines.len().max(text_lines.len());
    let mut lines = Vec::new();

    for i in 0..max_lines {
        let art_line = art_lines.get(i).map_or("", |s| s.as_str());
        let text_line = text_lines.get(i).map_or("", |s| s.as_str());

        // Pad the art column to fixed width.
        let padded_art = format!("{:width$}", art_line, width = ART_COLUMN_WIDTH);

        // Color the ASCII art with cyan.
        let colored_art = padded_art.cyan().to_string();

        // Clear line before writing.
        lines.push(format!("\x1b[2K{colored_art}{text_line}"));
    }

    lines.join("\n")
}

/// Best-effort check of the OS-level reduced-motion preference.
///
/// Any detection failure means "no preference detected" and animation stays enabled.
pub fn prefers_reduced_motion() -> bool {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("defaults")
            .args(["read", "com.apple.universalaccess", "reduceMotion"])
            .output()
            && output.status.success()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return stdout.trim() == "1";
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "enable-animations"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout.trim() == "false";
            }
        }
    }

    false
}

/// Checks if the terminal supports animation.
fn can_animate() -> bool {
    // Must be TTY.
    if !io::stdout().is_terminal() {
        return false;
    }

    // Respect NO_COLOR.
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }

    // Manual override.
    if std::env::var("OPENSPEC_NO_ANIMATION").is_ok() {
        return false;
    }

    // Check terminal width.
    let columns = terminal_size().unwrap_or(80);
    if columns < MIN_WIDTH {
        return false;
    }

    // Reduced motion check.
    if prefers_reduced_motion() {
        return false;
    }

    true
}

/// Get terminal width (best-effort).
fn terminal_size() -> Option<usize> {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = io::stdout().as_raw_fd();
        let mut winsize = libc_winsize {
            rows: 0,
            cols: 0,
            xpixel: 0,
            ypixel: 0,
        };
        // SAFETY: ioctl with TIOCGWINSZ is always safe.
        let result = unsafe { libc_ioctl(fd, TIOCGWINSZ, &mut winsize) };
        if result == 0 && winsize.cols > 0 {
            return Some(winsize.cols as usize);
        }
    }
    None
}

#[cfg(unix)]
#[repr(C)]
struct libc_winsize {
    rows: u16,
    cols: u16,
    xpixel: u16,
    ypixel: u16,
}

#[cfg(unix)]
const TIOCGWINSZ: u64 = 0x5413;

#[cfg(unix)]
unsafe extern "C" {
    #[link_name = "ioctl"]
    fn libc_ioctl(fd: i32, request: u64, ...) -> i32;
}

/// Wait for Enter key press from stdin.
fn wait_for_enter() -> io::Result<()> {
    let mut stdout = io::stdout();
    writeln!(stdout)?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(())
}

/// Shows the animated welcome screen.
///
/// Returns when the user presses Enter, or immediately in non-interactive mode.
pub fn show_welcome_screen(workflows: &[String], animate: Option<bool>) -> io::Result<()> {
    let text_lines = get_welcome_text(workflows);
    let animation = welcome_animation();

    if animate == Some(false) || !can_animate() {
        // Static fallback.
        let frame = &animation.frames[3]; // Peak frame.
        let rendered = render_frame(frame, &text_lines);
        let mut stdout = io::stdout();
        write!(stdout, "\n{rendered}\n\n")?;
        stdout.flush()?;

        if io::stdout().is_terminal() {
            wait_for_enter()?;
        }

        return Ok(());
    }

    // Animated welcome screen.
    let mut stdout = io::stdout();
    writeln!(stdout)?;
    stdout.flush()?;

    let frame_height = animation.frames[0].len().max(text_lines.len());
    let total_height = frame_height + 3; // internal newlines + trailing.
    let num_frames = animation.frames.len();
    let interval = Duration::from_millis(animation.interval_ms);

    // Animation loop.
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let running_clone = running.clone();

    let handle = thread::spawn(move || {
        let mut frame_index = 0;
        let mut is_first = true;

        while running_clone.load(std::sync::atomic::Ordering::Relaxed) {
            let frame = &animation.frames[frame_index];

            if !is_first {
                // Move cursor up to overwrite previous frame.
                print!("\x1b[{frame_height}A");
            }
            is_first = false;

            let rendered = render_frame(frame, &text_lines);
            print!("{rendered}\n\n");
            io::stdout().flush().ok();

            frame_index = (frame_index + 1) % num_frames;
            thread::sleep(interval);
        }
    });

    // Wait for Enter.
    wait_for_enter()?;

    // Stop animation.
    running.store(false, std::sync::atomic::Ordering::Relaxed);
    handle.join().ok();

    // Clear the welcome screen.
    for _ in 0..total_height {
        println!("\x1b[2K");
    }
    print!("\x1b[{total_height}A");
    io::stdout().flush()?;

    Ok(())
}

/// Extension trait to check if a stream is a terminal.
trait IsTerminal {
    fn is_terminal(&self) -> bool;
}

impl IsTerminal for io::Stdout {
    fn is_terminal(&self) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            unsafe { libc_isatty(self.as_raw_fd()) != 0 }
        }
        #[cfg(not(unix))]
        {
            true
        }
    }
}

#[cfg(unix)]
unsafe extern "C" {
    #[link_name = "isatty"]
    fn libc_isatty(fd: std::os::unix::io::RawFd) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_welcome_text_basic() {
        let text = get_welcome_text(&[]);
        assert!(text.iter().any(|l| l.contains("Welcome to Speckit")));
        assert!(text.iter().any(|l| l.contains("Press Enter")));
    }

    #[test]
    fn get_welcome_text_with_workflows() {
        let workflows = vec!["explore".to_string(), "apply".to_string()];
        let text = get_welcome_text(&workflows);
        assert!(text.iter().any(|l| l.contains("explore")));
        assert!(text.iter().any(|l| l.contains("apply")));
    }

    #[test]
    fn render_frame_produces_output() {
        let art = vec!["  ##  ".to_string(), " #### ".to_string()];
        let text = vec!["Hello".to_string(), "World".to_string()];
        let result = render_frame(&art, &text);
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
    }

    #[test]
    fn animation_frames_valid() {
        let anim = welcome_animation();
        assert!(!anim.frames.is_empty());
        for frame in &anim.frames {
            assert!(!frame.is_empty());
        }
    }
}
