use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

/// Regex matching `/opsx:<command-id>` references in text.
static COMMAND_REF_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/opsx:([a-z-]+)").unwrap());

/// Maps command short names to their skill names.
///
/// Keep in sync with the workflow-to-skill directory mapping.
static COMMAND_TO_SKILL_NAME: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("explore", "speckit-explore");
    m.insert("new", "speckit-new-change");
    m.insert("continue", "speckit-continue-change");
    m.insert("apply", "speckit-apply-change");
    m.insert("update", "speckit-update-change");
    m.insert("ff", "speckit-ff-change");
    m.insert("sync", "speckit-sync-specs");
    m.insert("archive", "speckit-archive-change");
    m.insert("bulk-archive", "speckit-bulk-archive-change");
    m.insert("verify", "speckit-verify-change");
    m.insert("onboard", "speckit-onboard");
    m.insert("propose", "speckit-propose");
    m
});

/// Tools whose skill invocation uses a non-default prefix. The default is `/`
/// (e.g. `/speckit-propose`); Kimi Code invokes skills as `/skill:<name>` and
/// Codex CLI as `$<name>`.
static SKILL_INVOCATION_PREFIX: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();
        m.insert("kimi", "/skill:");
        m.insert("codex", "$");
        m
    });

/// Tools that have no slash-command surface at all: skills are matched
/// automatically or invoked by natural-language prompts.
static NATURAL_LANGUAGE_SKILL_TOOLS: &[&str] = &["rovodev"];

/// The invocation style for a command.
#[derive(Debug, Clone)]
pub struct CommandInvocation {
    /// How the command is invoked: "flat" uses filename, "namespaced" uses directory.
    pub style: InvocationStyle,
    /// The prefix character (e.g., `/` or `@`).
    pub prefix: String,
}

/// How a tool's commands are named on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvocationStyle {
    /// Command named by filename (e.g., `opsx-apply.md` -> `/opsx-apply`).
    Flat,
    /// Command in an `opsx/` directory (e.g., `opsx/apply.md` -> `/opsx:apply`).
    Namespaced,
}

impl CommandInvocation {
    /// Format a command invocation for this tool.
    pub fn format(&self, command_id: &str) -> String {
        match self.style {
            InvocationStyle::Flat => format!("{}opsx-{}", self.prefix, command_id),
            InvocationStyle::Namespaced => format!("{}opsx:{}", self.prefix, command_id),
        }
    }

    /// Returns `true` when this invocation style needs a rewrite from the
    /// canonical `/opsx:<command>` form.
    pub fn needs_rewrite(&self) -> bool {
        self.style == InvocationStyle::Flat || self.prefix != "/"
    }
}

/// Transforms `/opsx:<command>` references in text to the tool's invocation form.
///
/// Only known command ids are rewritten; unrecognized references are left as-is.
pub fn transform_command_invocations(text: &str, invocation: &CommandInvocation) -> String {
    COMMAND_REF_REGEX
        .replace_all(text, |caps: &regex::Captures| {
            let command_id = &caps[1];
            if COMMAND_TO_SKILL_NAME.contains_key(command_id) {
                invocation.format(command_id)
            } else {
                caps[0].to_string()
            }
        })
        .into_owned()
}

/// Whether a tool references skills by natural language rather than a slash command.
pub fn uses_natural_language_skill_references(tool_id: &str) -> bool {
    NATURAL_LANGUAGE_SKILL_TOOLS.contains(&tool_id)
}

/// Replace `/opsx:<command>` with natural-language skill references
/// (e.g., "the speckit-propose skill").
fn replace_with_natural_language(text: &str) -> String {
    COMMAND_REF_REGEX
        .replace_all(text, |caps: &regex::Captures| {
            let command_id = &caps[1];
            match COMMAND_TO_SKILL_NAME.get(command_id) {
                Some(skill_name) => format!("the {skill_name} skill"),
                None => caps[0].to_string(),
            }
        })
        .into_owned()
}

/// Replace `/opsx:<command>` with skill references using the given prefix.
fn replace_with_skill_references(text: &str, prefix: &str) -> String {
    COMMAND_REF_REGEX
        .replace_all(text, |caps: &regex::Captures| {
            let command_id = &caps[1];
            match COMMAND_TO_SKILL_NAME.get(command_id) {
                Some(skill_name) => format!("{prefix}{skill_name}"),
                None => caps[0].to_string(),
            }
        })
        .into_owned()
}

/// Transforms command references to skill references using the default `/`
/// prefix (e.g., `/speckit-apply-change`).
pub fn transform_to_skill_references(text: &str) -> String {
    replace_with_skill_references(text, "/")
}

/// Transforms to Codex-compatible references: `$<name> (Codex) or /<name> (other agents)`.
pub fn transform_to_codex_compatible_skill_references(text: &str) -> String {
    COMMAND_REF_REGEX
        .replace_all(text, |caps: &regex::Captures| {
            let command_id = &caps[1];
            match COMMAND_TO_SKILL_NAME.get(command_id) {
                Some(skill_name) => {
                    format!("${skill_name} (Codex) or /{skill_name} (other agents)")
                }
                None => caps[0].to_string(),
            }
        })
        .into_owned()
}

/// Returns the skill-reference transformer for a specific tool, honoring the
/// tool's documented skill invocation syntax.
///
/// Tools with no slash surface get natural-language references; everything
/// else falls back to the default `/speckit-*` form.
pub fn get_skill_reference_transformer(tool_id: &str) -> Box<dyn Fn(&str) -> String> {
    if uses_natural_language_skill_references(tool_id) {
        Box::new(|text: &str| replace_with_natural_language(text))
    } else if let Some(prefix) = SKILL_INVOCATION_PREFIX.get(tool_id) {
        let prefix = prefix.to_string();
        Box::new(move |text: &str| replace_with_skill_references(text, &prefix))
    } else {
        Box::new(|text: &str| transform_to_skill_references(text))
    }
}

/// The delivery mode for a tool's commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    Both,
    Skills,
    Commands,
}

/// The command surface capability for a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSurfaceCapability {
    /// Commands are backed by a file adapter.
    AdapterBacked,
    /// No command surface at all (skills-only).
    None,
    /// Skills are invocable directly.
    SkillsInvocable,
}

/// Selects the command-reference transformer for a skill generation target.
///
/// Returns `None` when the tool already answers to the canonical `/opsx:<id>`.
pub fn get_transformer_for_tool(
    tool_id: &str,
    delivery: DeliveryMode,
    capability: CommandSurfaceCapability,
    invocation: Option<&CommandInvocation>,
) -> Option<Box<dyn Fn(&str) -> String>> {
    if delivery == DeliveryMode::Skills || capability != CommandSurfaceCapability::AdapterBacked {
        return if tool_id == "codex" {
            Some(Box::new(|text: &str| {
                transform_to_codex_compatible_skill_references(text)
            }))
        } else {
            Some(get_skill_reference_transformer(tool_id))
        };
    }

    if tool_id == "devin" && delivery == DeliveryMode::Both {
        return Some(get_skill_reference_transformer(tool_id));
    }

    if let Some(inv) = invocation
        && inv.needs_rewrite() {
            let inv = inv.clone();
            return Some(Box::new(move |text: &str| {
                transform_command_invocations(text, &inv)
            }));
        }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_to_skill_references_basic() {
        let result = transform_to_skill_references("/opsx:apply");
        assert_eq!(result, "/speckit-apply-change");
    }

    #[test]
    fn transform_to_skill_references_unknown() {
        let result = transform_to_skill_references("/opsx:unknown");
        assert_eq!(result, "/opsx:unknown");
    }

    #[test]
    fn transform_to_skill_references_multiple() {
        let result = transform_to_skill_references("Use /opsx:apply and /opsx:archive");
        assert_eq!(
            result,
            "Use /speckit-apply-change and /speckit-archive-change"
        );
    }

    #[test]
    fn transform_command_invocations_flat() {
        let inv = CommandInvocation {
            style: InvocationStyle::Flat,
            prefix: "/".to_string(),
        };
        let result = transform_command_invocations("/opsx:apply", &inv);
        assert_eq!(result, "/opsx-apply");
    }

    #[test]
    fn transform_command_invocations_at_prefix() {
        let inv = CommandInvocation {
            style: InvocationStyle::Flat,
            prefix: "@".to_string(),
        };
        let result = transform_command_invocations("/opsx:apply", &inv);
        assert_eq!(result, "@opsx-apply");
    }

    #[test]
    fn transform_codex_compatible() {
        let result = transform_to_codex_compatible_skill_references("/opsx:apply");
        assert_eq!(
            result,
            "$speckit-apply-change (Codex) or /speckit-apply-change (other agents)"
        );
    }

    #[test]
    fn natural_language_references() {
        assert!(uses_natural_language_skill_references("rovodev"));
        assert!(!uses_natural_language_skill_references("claude"));
    }

    #[test]
    fn get_transformer_rovodev() {
        let transformer = get_skill_reference_transformer("rovodev");
        let result = transformer("/opsx:apply");
        assert_eq!(result, "the speckit-apply-change skill");
    }

    #[test]
    fn get_transformer_kimi() {
        let transformer = get_skill_reference_transformer("kimi");
        let result = transformer("/opsx:apply");
        assert_eq!(result, "/skill:speckit-apply-change");
    }

    #[test]
    fn get_transformer_codex() {
        let transformer = get_skill_reference_transformer("codex");
        let result = transformer("/opsx:apply");
        assert_eq!(result, "$speckit-apply-change");
    }
}
