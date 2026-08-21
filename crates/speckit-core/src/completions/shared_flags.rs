/// Common flags shared across commands.

/// Store option description.
pub const STORE_OPTION_DESCRIPTION: &str = "Specify a registered store by id";

/// Common flags for commands.
pub struct CommonFlags;

impl CommonFlags {
    pub const STORE: (&'static str, &'static str) = ("--store", STORE_OPTION_DESCRIPTION);
    pub const JSON: (&'static str, &'static str) = ("--json", "Output as JSON");
    pub const NO_INTERACTIVE: (&'static str, &'static str) =
        ("--no-interactive", "Disable interactive prompts");
}
