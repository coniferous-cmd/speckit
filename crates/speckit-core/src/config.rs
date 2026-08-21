use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

/// The name of the Speckit directory within a project.
pub const SPECKIT_DIR_NAME: &str = "speckit";

/// All Speckit skill names (slash-command identifiers).
pub const OPENSPEC_SKILL_NAMES: &[&str] = &[
    "speckit-explore",
    "speckit-new-change",
    "speckit-continue-change",
    "speckit-apply-change",
    "speckit-update-change",
    "speckit-ff-change",
    "speckit-sync-specs",
    "speckit-archive-change",
    "speckit-bulk-archive-change",
    "speckit-verify-change",
    "speckit-onboard",
    "speckit-propose",
];

/// Comment markers used to delimit Speckit-managed sections in files.
pub struct SpeckitMarkers {
    pub start: &'static str,
    pub end: &'static str,
}

pub const OPENSPEC_MARKERS: SpeckitMarkers = SpeckitMarkers {
    start: "<!-- OPENSPEC:START -->",
    end: "<!-- OPENSPEC:END -->",
};

/// Configuration for a single AI tool supported by Speckit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiToolOption {
    pub name: String,
    pub value: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_label: Option<String>,
    /// Directory name for skills (e.g., `.claude`); `/skills` suffix is appended per Agent Skills spec.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills_dir: Option<String>,
    /// Former roots read for detection and migrated after replacement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_skills_dirs: Option<Vec<String>>,
    /// Global skills directory resolved from the user's home directory (e.g., `.minimax`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_skills_dir: Option<String>,
    /// Override `skills_dir` for auto-detection; any existing path triggers detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_paths: Option<Vec<String>>,
    /// Manual setup note shown after init/update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup_note: Option<String>,
    /// True when slash commands are loaded by an IDE/editor process.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_ide_restart: Option<bool>,
}

/// All 38 supported AI tools.
pub static AI_TOOLS: LazyLock<Vec<AiToolOption>> = LazyLock::new(|| {
    vec![
        AiToolOption {
            name: "Amazon Q Developer".into(),
            value: "amazon-q".into(),
            available: true,
            success_label: Some("Amazon Q Developer".into()),
            skills_dir: Some(".amazonq".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "Antigravity".into(),
            value: "antigravity".into(),
            available: true,
            success_label: Some("Antigravity".into()),
            skills_dir: Some(".agent".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "Auggie (Augment CLI)".into(),
            value: "auggie".into(),
            available: true,
            success_label: Some("Auggie".into()),
            skills_dir: Some(".augment".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Bob Shell".into(),
            value: "bob".into(),
            available: true,
            success_label: Some("Bob Shell".into()),
            skills_dir: Some(".bob".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Claude Code".into(),
            value: "claude".into(),
            available: true,
            success_label: Some("Claude Code".into()),
            skills_dir: Some(".claude".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Cline".into(),
            value: "cline".into(),
            available: true,
            success_label: Some("Cline".into()),
            skills_dir: Some(".cline".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "Command Code".into(),
            value: "command-code".into(),
            available: true,
            success_label: Some("Command Code".into()),
            skills_dir: Some(".commandcode".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "CodeArts".into(),
            value: "codeartsagent".into(),
            available: true,
            success_label: Some("CodeArts".into()),
            skills_dir: Some(".codeartsdoer".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Codex".into(),
            value: "codex".into(),
            available: true,
            success_label: Some("Codex".into()),
            skills_dir: Some(".agents".into()),
            legacy_skills_dirs: Some(vec![".codex".into()]),
            global_skills_dir: None,
            detection_paths: Some(vec![".agents/skills".into(), ".codex/skills".into()]),
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Devin Desktop (formerly Windsurf)".into(),
            value: "devin".into(),
            available: true,
            success_label: Some("Devin Desktop".into()),
            skills_dir: Some(".devin".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: Some(vec![".devin".into(), ".windsurf".into()]),
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "ForgeCode".into(),
            value: "forgecode".into(),
            available: true,
            success_label: Some("ForgeCode".into()),
            skills_dir: Some(".forge".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "CodeBuddy Code (CLI)".into(),
            value: "codebuddy".into(),
            available: true,
            success_label: Some("CodeBuddy Code".into()),
            skills_dir: Some(".codebuddy".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Continue".into(),
            value: "continue".into(),
            available: true,
            success_label: Some("Continue (VS Code / JetBrains / Cli)".into()),
            skills_dir: Some(".continue".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "CoStrict".into(),
            value: "costrict".into(),
            available: true,
            success_label: Some("CoStrict".into()),
            skills_dir: Some(".cospec".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "Crush".into(),
            value: "crush".into(),
            available: true,
            success_label: Some("Crush".into()),
            skills_dir: Some(".crush".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Cursor".into(),
            value: "cursor".into(),
            available: true,
            success_label: Some("Cursor".into()),
            skills_dir: Some(".cursor".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "Factory Droid".into(),
            value: "factory".into(),
            available: true,
            success_label: Some("Factory Droid".into()),
            skills_dir: Some(".factory".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Gemini CLI".into(),
            value: "gemini".into(),
            available: true,
            success_label: Some("Gemini CLI".into()),
            skills_dir: Some(".gemini".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "GitHub Copilot".into(),
            value: "github-copilot".into(),
            available: true,
            success_label: Some("GitHub Copilot".into()),
            skills_dir: Some(".github".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: Some(vec![
                ".github/copilot-instructions.md".into(),
                ".github/instructions".into(),
                ".github/workflows/copilot-setup-steps.yml".into(),
                ".github/prompts".into(),
                ".github/agents".into(),
                ".github/skills".into(),
                ".github/.mcp.json".into(),
            ]),
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "Hermes Agent".into(),
            value: "hermes".into(),
            available: true,
            success_label: Some("Hermes Agent".into()),
            skills_dir: Some(".hermes".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: Some(vec![
                ".hermes".into(),
                "HERMES.md".into(),
                ".hermes.md".into(),
            ]),
            setup_note: Some("Hermes only loads skills from ~/.hermes/skills by default. Add this project's .hermes/skills directory to skills.external_dirs in ~/.hermes/config.yaml so Hermes picks up the generated Speckit skills.".into()),
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "iFlow".into(),
            value: "iflow".into(),
            available: true,
            success_label: Some("iFlow".into()),
            skills_dir: Some(".iflow".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Junie".into(),
            value: "junie".into(),
            available: true,
            success_label: Some("Junie".into()),
            skills_dir: Some(".junie".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "Kilo Code".into(),
            value: "kilocode".into(),
            available: true,
            success_label: Some("Kilo Code".into()),
            skills_dir: Some(".kilocode".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "Kimi Code".into(),
            value: "kimi".into(),
            available: true,
            success_label: Some("Kimi Code".into()),
            skills_dir: Some(".kimi-code".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: Some(vec![".kimi-code".into(), ".kimi".into()]),
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Kiro".into(),
            value: "kiro".into(),
            available: true,
            success_label: Some("Kiro".into()),
            skills_dir: Some(".kiro".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "Lingma".into(),
            value: "lingma".into(),
            available: true,
            success_label: Some("Lingma".into()),
            skills_dir: Some(".lingma".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "MiniMax Code".into(),
            value: "minimax-code".into(),
            available: true,
            success_label: Some("MiniMax Code".into()),
            skills_dir: None,
            legacy_skills_dirs: None,
            global_skills_dir: Some(".minimax".into()),
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Mistral Vibe".into(),
            value: "vibe".into(),
            available: true,
            success_label: Some("Mistral Vibe".into()),
            skills_dir: Some(".vibe".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Oh My Pi".into(),
            value: "oh-my-pi".into(),
            available: true,
            success_label: Some("Oh My Pi".into()),
            skills_dir: Some(".omp".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "OpenCode".into(),
            value: "opencode".into(),
            available: true,
            success_label: Some("OpenCode".into()),
            skills_dir: Some(".opencode".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Pi".into(),
            value: "pi".into(),
            available: true,
            success_label: Some("Pi".into()),
            skills_dir: Some(".pi".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Qoder".into(),
            value: "qoder".into(),
            available: true,
            success_label: Some("Qoder".into()),
            skills_dir: Some(".qoder".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "Qwen Code".into(),
            value: "qwen".into(),
            available: true,
            success_label: Some("Qwen Code".into()),
            skills_dir: Some(".qwen".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Rovo Dev CLI".into(),
            value: "rovodev".into(),
            available: true,
            success_label: Some("Rovo Dev CLI".into()),
            skills_dir: Some(".rovodev".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: Some(vec![".rovodev/skills".into(), ".rovodev".into()]),
            setup_note: None,
            requires_ide_restart: None,
        },
        AiToolOption {
            name: "Zoo Code".into(),
            value: "roocode".into(),
            available: true,
            success_label: Some("Zoo Code".into()),
            skills_dir: Some(".roo".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "Trae".into(),
            value: "trae".into(),
            available: true,
            success_label: Some("Trae".into()),
            skills_dir: Some(".trae".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: Some(true),
        },
        AiToolOption {
            name: "ZCode".into(),
            value: "zcode".into(),
            available: true,
            success_label: Some("ZCode".into()),
            skills_dir: Some(".zcode".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: None,
            setup_note: None,
            requires_ide_restart: None,
        },
        // Vendor-neutral target for assistants that read the shared `.agents` root.
        // Detection keys off `.agents/skills` rather than the bare root: frameworks use
        // `.agents/` for more than skills, so the root alone says nothing about skills.
        // A project that does keep skills there is a project this target fits, the same
        // way `.claude/` selects Claude Code -- the signal is the user's setup, not
        // Speckit's own files.
        AiToolOption {
            name: "Shared .agents skills".into(),
            value: "agents".into(),
            available: true,
            success_label: Some("shared .agents skills".into()),
            skills_dir: Some(".agents".into()),
            legacy_skills_dirs: None,
            global_skills_dir: None,
            detection_paths: Some(vec![".agents/skills".into()]),
            setup_note: None,
            requires_ide_restart: None,
        },
    ]
});

/// Retired tool ids that still resolve, so a rebrand does not break scripted
/// `--tools` invocations.  Windsurf was rebranded to Devin Desktop on
/// 2026-06-02 and its config directory moved from `.windsurf/` to `.devin/`;
/// `--tools windsurf` therefore configures `devin`.
pub static TOOL_ID_ALIASES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("windsurf", "devin");
    m
});

/// Resolves a tool id through [`TOOL_ID_ALIASES`], leaving current ids untouched.
pub fn resolve_tool_id_alias(tool_id: &str) -> &str {
    TOOL_ID_ALIASES.get(tool_id).copied().unwrap_or(tool_id)
}

/// Find a tool by its value (id).
pub fn find_tool(tool_id: &str) -> Option<&'static AiToolOption> {
    AI_TOOLS.iter().find(|t| t.value == tool_id)
}

/// Return all tool ids (values).
pub fn all_tool_ids() -> Vec<String> {
    AI_TOOLS.iter().map(|t| t.value.clone()).collect()
}

/// Top-level Speckit configuration (project-level `speckit/config.yaml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeckitConfig {
    pub ai_tools: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_id_alias_windsurf_resolves_to_devin() {
        assert_eq!(resolve_tool_id_alias("windsurf"), "devin");
    }

    #[test]
    fn tool_id_alias_unknown_passthrough() {
        assert_eq!(resolve_tool_id_alias("claude"), "claude");
    }

    #[test]
    fn ai_tools_count() {
        // Count the actual entries in the array (38 tools).
        assert_eq!(AI_TOOLS.len(), 38);
    }

    #[test]
    fn speckit_skill_names_count() {
        assert_eq!(OPENSPEC_SKILL_NAMES.len(), 12);
    }

    #[test]
    fn markers_roundtrip() {
        assert_eq!(OPENSPEC_MARKERS.start, "<!-- OPENSPEC:START -->");
        assert_eq!(OPENSPEC_MARKERS.end, "<!-- OPENSPEC:END -->");
    }
}
