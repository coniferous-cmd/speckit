//! Command Invocation
//!
//! How a tool spells an Speckit command has two parts:
//!
//! - The *name* comes from the file. `.../commands/opsx/<id>.md` is namespaced
//!   by its directory, so the tool registers `opsx:<id>`. `.../commands/opsx-<id>.md`
//!   names the command with the filename, so the tool registers `opsx-<id>`.
//! - The *prefix* is the tool's own and cannot be derived. Almost every tool
//!   uses `/`; Amazon Q loads these files into its prompt library, which is
//!   invoked with `@`.

use std::path::Path;

use super::types::{
    CommandInvocation, CommandInvocationStyle, ToolCommandAdapter, canonical_invocation,
};

/// Classifies a generated command file by the name the tool will answer to.
///
/// The test is the filename, not the directory: an `opsx-` prefix means the
/// filename is the command. Every other shape is treated as namespaced.
pub fn get_invocation_style_for_path(command_file_path: &str) -> CommandInvocationStyle {
    let basename = Path::new(command_file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if basename.starts_with("opsx-") {
        CommandInvocationStyle::Flat
    } else {
        CommandInvocationStyle::Namespaced
    }
}

/// Resolves how a tool's generated commands are invoked: the name from the
/// files its adapter writes, the prefix from the adapter's own declaration.
pub fn get_invocation_for_adapter(adapter: &dyn ToolCommandAdapter) -> CommandInvocation {
    let default = canonical_invocation();
    CommandInvocation {
        style: get_invocation_style_for_path(&adapter.get_file_path("explore")),
        prefix: adapter
            .invocation_prefix()
            .unwrap_or(&default.prefix)
            .to_string(),
    }
}

/// Spells one command the way the tool registers it.
///
/// Returns what the user types, e.g. `/opsx:apply`, `/opsx-apply`, `@opsx-apply`
pub fn format_command_invocation(invocation: &CommandInvocation, command_id: &str) -> String {
    let separator = match invocation.style {
        CommandInvocationStyle::Namespaced => ":",
        CommandInvocationStyle::Flat => "-",
    };
    format!("{}opsx{}{}", invocation.prefix, separator, command_id)
}

/// Whether a tool's invocation differs from the canonical `/opsx:<id>` that
/// command bodies and skill templates are authored in — that is, whether
/// generated text has to be rewritten for that tool at all.
pub fn needs_invocation_rewrite(invocation: &CommandInvocation) -> bool {
    let canonical = canonical_invocation();
    invocation.style != canonical.style || invocation.prefix != canonical.prefix
}

/// Known command IDs that can be rewritten.
const KNOWN_COMMAND_IDS: &[&str] = &[
    "explore",
    "new",
    "continue",
    "apply",
    "update",
    "ff",
    "sync",
    "archive",
    "bulk-archive",
    "verify",
    "onboard",
    "propose",
];

/// Rewrites the canonical `/opsx:<command>` references in text into the form
/// one tool actually registers.
///
/// Only known command ids are rewritten, matching how unrecognized references
/// are left alone so a mistyped or invented `/opsx:<something>` is left as
/// written rather than silently reshaped.
pub fn transform_command_invocations(text: &str, invocation: &CommandInvocation) -> String {
    let mut result = String::with_capacity(text.len());
    let mut remaining = text;

    while let Some(pos) = remaining.find("/opsx:") {
        result.push_str(&remaining[..pos]);
        let after_prefix = &remaining[pos + 6..]; // skip "/opsx:"

        // Find the end of the command id (lowercase letters and hyphens)
        let cmd_end = after_prefix
            .find(|c: char| !c.is_ascii_lowercase() && c != '-')
            .unwrap_or(after_prefix.len());

        let command_id = &after_prefix[..cmd_end];

        if KNOWN_COMMAND_IDS.contains(&command_id) {
            result.push_str(&format_command_invocation(invocation, command_id));
        } else {
            result.push_str("/opsx:");
            result.push_str(command_id);
        }

        remaining = &after_prefix[cmd_end..];
    }

    result.push_str(remaining);
    result
}
