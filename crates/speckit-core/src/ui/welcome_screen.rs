use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use colored::Colorize;

use super::ascii_patterns::welcome_animation;

/// Minimum terminal width for side-by-side layout.
const MIN_WIDTH: usize = 60;

/// Width of the ASCII art column (with padding).
const ART_COLUMN_WIDTH: usize = 24;

/// Visible cell width of a string, ignoring ANSI escape sequences.
///
/// `colored` wraps every line in escape codes, so `chars().count()` would count
/// the markup as if it were printed text.
fn visible_width(text: &str) -> usize {
    let mut width = 0;
    let mut chars = text.chars();

    while let Some(c) = chars.next() {
        if c != '\x1b' {
            width += 1;
            continue;
        }

        // CSI sequence: ESC [ parameter bytes ... final byte in @..~
        if chars.next() == Some('[') {
            for byte in chars.by_ref() {
                if ('@'..='~').contains(&byte) {
                    break;
                }
            }
        }
    }

    width
}

/// Widest row the side-by-side layout can produce, in terminal cells.
///
/// A row that exceeds the terminal width wraps onto a second line, and wrapped
/// rows are invisible to cursor-up arithmetic — the animation would drift down
/// the screen exactly as it does when the row count is simply wrong.
fn required_width(text_lines: &[String]) -> usize {
    let text_width = text_lines
        .iter()
        .map(|line| visible_width(line))
        .max()
        .unwrap_or(0);

    ART_COLUMN_WIDTH + text_width
}

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

/// Rows a frame occupies on screen, measured from the frame it actually renders.
///
/// [`render_frame`] joins its lines without a trailing newline and callers append
/// `"\n\n"`, so one frame leaves the cursor exactly this many rows below its top.
/// Every overwrite and every clear must move by this value: counting rows some
/// other way drifts, and a drift the height of the frame per redraw looks like the
/// welcome screen reprinting itself forever.
fn frame_rows(art_lines: &[String], text_lines: &[String]) -> usize {
    // The joined lines, plus the blank row that follows them.
    render_frame(art_lines, text_lines).lines().count() + 1
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
fn can_animate(text_lines: &[String]) -> bool {
    // Must be TTY.
    if !io::stdout().is_terminal() {
        return false;
    }

    // Respect NO_COLOR.
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }

    // Manual override.
    if std::env::var("SPECKIT_NO_ANIMATION").is_ok() {
        return false;
    }

    // Check terminal width: the layout floor, and enough room that no row wraps.
    let columns = terminal_size().unwrap_or(80);
    if columns < MIN_WIDTH || columns < required_width(text_lines) {
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

/// Blocks until the user presses Enter.
///
/// Writes nothing on purpose: in the animated branch this thread shares the
/// terminal with the redraw loop, and one stray newline desynchronises the row
/// counting the animation relies on. Callers that need a blank line own it.
fn wait_for_enter() -> io::Result<()> {
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

    if animate == Some(false) || !can_animate(&text_lines) {
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

    let num_frames = animation.frames.len();
    let interval = Duration::from_millis(animation.interval_ms);
    // Every frame renders at the same height, so the block to erase is known up
    // front — the redraw thread takes `animation` and `text_lines` with it.
    let rows = frame_rows(&animation.frames[0], &text_lines);

    // Animation loop.
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let running_clone = running.clone();

    let handle = thread::spawn(move || {
        let mut frame_index = 0;
        // Rows covered by the frame currently on screen; 0 until one is drawn.
        let mut previous_rows = 0;

        while running_clone.load(std::sync::atomic::Ordering::Relaxed) {
            let frame = &animation.frames[frame_index];
            let rendered = render_frame(frame, &text_lines);

            // Return to the top of the frame on screen before redrawing it.
            if previous_rows > 0 {
                print!("\x1b[{previous_rows}A");
            }

            print!("{rendered}\n\n");
            io::stdout().flush().ok();

            previous_rows = frame_rows(frame, &text_lines);
            frame_index = (frame_index + 1) % num_frames;
            thread::sleep(interval);
        }
    });

    // Wait for Enter.
    wait_for_enter()?;

    // Stop animation.
    running.store(false, std::sync::atomic::Ordering::Relaxed);
    handle.join().ok();

    // Clear the welcome screen. The cursor sits `rows` below the top of the frame
    // just drawn, plus one more row echoed by the terminal for the Enter keypress,
    // so climb back to that top, erase downward, then return to it and let the next
    // prompt print over the cleared block. A frame drawn while this thread was
    // still blocked in `read_line` lands one row off and can leave a trace; cleanup
    // here is best-effort, the flood of repaints above is not.
    let mut clear = String::new();
    clear.push_str(&format!("\x1b[{}A", rows + 1));
    for _ in 0..rows {
        clear.push_str("\x1b[2K\x1b[1B");
    }
    clear.push_str(&format!("\x1b[{rows}A\r"));
    write!(stdout, "{clear}")?;
    stdout.flush()?;

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
        let workflows = vec!["explore".to_string(), "implement".to_string()];
        let text = get_welcome_text(&workflows);
        assert!(text.iter().any(|l| l.contains("explore")));
        assert!(text.iter().any(|l| l.contains("implement")));
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

    /// The redraw loop moves the cursor up by [`frame_rows`] between frames, and
    /// writes each frame as `render_frame(..)` plus `"\n\n"`. Any mismatch is a
    /// one-row-per-frame drift, which reads as the welcome screen reprinting
    /// itself down the terminal forever.
    #[test]
    fn frame_rows_counts_every_row_a_frame_touches() {
        let anim = welcome_animation();

        let empty: [String; 0] = [];
        let with_one = ["explore".to_string()];

        for workflows in [&empty[..], &with_one[..]] {
            let text = get_welcome_text(workflows);

            for frame in &anim.frames {
                let written = render_frame(frame, &text).matches('\n').count() + 2;
                assert_eq!(frame_rows(frame, &text), written);
            }
        }
    }

    #[test]
    fn visible_width_ignores_ansi_escapes() {
        assert_eq!(visible_width("plain"), 5);
        assert_eq!(visible_width("\x1b[38;5;51mcolored\x1b[0m"), 7);
        assert_eq!(visible_width("\x1b[1m\x1b[37mbold\x1b[0m"), 4);
    }

    #[test]
    fn required_width_covers_the_longest_row() {
        let workflows = vec!["explore".to_string(), "implement".to_string()];
        let text = get_welcome_text(&workflows);
        let required = required_width(&text);

        for line in &text {
            assert!(ART_COLUMN_WIDTH + visible_width(line) <= required);
        }
    }
}
