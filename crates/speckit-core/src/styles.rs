/// Color palette for Speckit terminal output.
/// Mirrors the TypeScript PALETTE constant.
pub struct Palette {
    pub primary: &'static str,
    pub secondary: &'static str,
    pub success: &'static str,
    pub warning: &'static str,
    pub error: &'static str,
    pub info: &'static str,
    pub muted: &'static str,
    pub accent: &'static str,
}

pub const PALETTE: Palette = Palette {
    primary: "#6366f1",   // Indigo
    secondary: "#8b5cf6", // Violet
    success: "#22c55e",   // Green
    warning: "#f59e0b",   // Amber
    error: "#ef4444",     // Red
    info: "#3b82f6",      // Blue
    muted: "#6b7280",     // Gray
    accent: "#06b6d4",    // Cyan
};

/// Spinner frames for progress indicators
pub const PROGRESS_SPINNER: [&str; 9] = [
    "░░░",
    "▒░░",
    "▒▒░",
    "▒▒▒",
    "▓▒▒",
    "▓▓▒",
    "▓▓▓",
    "▒▓▓",
    "░▒▓",
];

/// The default workflow schema name
pub const DEFAULT_SCHEMA: &str = "spec-driven";
