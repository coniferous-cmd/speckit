/// Character set for ASCII art based on Unicode support.
///
/// Block characters for pixel-art aesthetic.
pub struct CharSet {
    pub full: &'static str,
    pub dim: &'static str,
    pub empty: &'static str,
}

/// Unicode block character set.
pub const UNICODE_CHARS: CharSet = CharSet {
    full: "\u{2588}\u{2588}",
    dim: "\u{2591}\u{2591}",
    empty: "  ",
};

/// ASCII fallback character set for terminals without Unicode support.
pub const ASCII_CHARS: CharSet = CharSet {
    full: "##",
    dim: "++",
    empty: "  ",
};

/// Returns the appropriate character set for the current platform.
pub fn detect_char_set() -> &'static CharSet {
    // On Windows, check for Windows Terminal or modern terminal.
    #[cfg(target_os = "windows")]
    {
        if std::env::var("WT_SESSION").is_ok() || std::env::var("TERM_PROGRAM").is_ok() {
            return &UNICODE_CHARS;
        }
        return &ASCII_CHARS;
    }

    // On non-Windows platforms, assume Unicode support.
    #[cfg(not(target_os = "windows"))]
    {
        &UNICODE_CHARS
    }
}

/// A single frame of the welcome animation (array of line strings).
pub type AnimationFrame = Vec<String>;

/// Welcome animation data.
pub struct WelcomeAnimation {
    /// Interval between frames in milliseconds.
    pub interval_ms: u64,
    /// The animation frames.
    pub frames: Vec<AnimationFrame>,
}

/// Generates the welcome animation frames for the Speckit logo.
///
/// The logo is a diamond/rhombus shape with hollow center "O".
/// 10 rows x 8 columns grid; each cell is 2 chars wide.
pub fn welcome_animation() -> WelcomeAnimation {
    let chars = detect_char_set();
    let _ = chars.empty;
    let f = chars.full;
    let d = chars.dim;
    let e = chars.empty;

    WelcomeAnimation {
        interval_ms: 120,
        frames: vec![
            // Frame 1: Empty
            vec![
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
            ],
            // Frame 2: Center blocks appear (dim)
            vec![
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{d}{d}{e}{e}"),
                format!("{e}{e}{e}{e}{d}{d}{e}{e}"),
                format!("{e}{e}{e}{e}{d}{d}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
            ],
            // Frame 3: Center blocks solidify
            vec![
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
            ],
            // Frame 4: Top and bottom points appear
            vec![
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{d}{d}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{d}{d}{e}{e}"),
            ],
            // Frame 5: Inner ring forming
            vec![
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
                format!("{e}{e}{e}{d}{e}{e}{d}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
                format!("{e}{e}{e}{d}{e}{e}{d}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
            ],
            // Frame 6: Outer ring appearing
            vec![
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
                format!("{e}{e}{e}{f}{e}{e}{f}{e}"),
                format!("{e}{e}{d}{e}{f}{f}{e}{d}"),
                format!("{e}{e}{d}{e}{f}{f}{e}{d}"),
                format!("{e}{e}{d}{e}{f}{f}{e}{d}"),
                format!("{e}{e}{e}{f}{e}{e}{f}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
            ],
            // Frame 7: Full logo
            vec![
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
                format!("{e}{e}{e}{f}{e}{e}{f}{e}"),
                format!("{e}{e}{f}{e}{f}{f}{e}{f}"),
                format!("{e}{e}{f}{e}{f}{f}{e}{f}"),
                format!("{e}{e}{f}{e}{f}{f}{e}{f}"),
                format!("{e}{e}{e}{f}{e}{e}{f}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
            ],
            // Frame 8: Hold complete logo
            vec![
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{e}{e}{e}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
                format!("{e}{e}{e}{f}{e}{e}{f}{e}"),
                format!("{e}{e}{f}{e}{f}{f}{e}{f}"),
                format!("{e}{e}{f}{e}{f}{f}{e}{f}"),
                format!("{e}{e}{f}{e}{f}{f}{e}{f}"),
                format!("{e}{e}{e}{f}{e}{e}{f}{e}"),
                format!("{e}{e}{e}{e}{f}{f}{e}{e}"),
            ],
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welcome_animation_has_frames() {
        let anim = welcome_animation();
        assert_eq!(anim.frames.len(), 8);
        assert_eq!(anim.interval_ms, 120);
    }

    #[test]
    fn all_frames_same_height() {
        let anim = welcome_animation();
        let height = anim.frames[0].len();
        for (i, frame) in anim.frames.iter().enumerate() {
            assert_eq!(frame.len(), height, "Frame {i} has wrong height");
        }
    }

    #[test]
    fn last_frame_has_logo() {
        let anim = welcome_animation();
        let last = &anim.frames[anim.frames.len() - 1];
        // The logo should have non-empty lines.
        assert!(last.iter().any(|line| !line.trim().is_empty()));
    }
}
