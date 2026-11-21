//! Workflow template modules for Speckit skills and slash commands.
//!
//! Each public function returns a `SkillTemplate` or `CommandTemplate` for one
//! of the Speckit workflow operations (explore, new, continue, implement, etc.).
//!
//! Instruction bodies are loaded from `text/<workflow>.md` via `include_str!`
//! so the canonical text is loaded directly. Do not duplicate or rewrite this text
//! inline; the parity tests depend on a single source of truth.
//!
//! Each body carries `{{PROJECT_ROOT_GUARD}}`, substituted by
//! [`project_root::with_project_root_guard`] at load time, so the project-root
//! check cannot be dropped from one skill while surviving in the others.

pub mod project_root;

use super::types::{CommandTemplate, SkillTemplate};
use project_root::with_project_root_guard;

// ---------------------------------------------------------------------------
// Implement (shared instructions used by both skill and command surfaces)
// ---------------------------------------------------------------------------

/// Shared implement workflow instructions. The body is loaded from
/// `text/implement-change.md` and reused by the skill and command surfaces.
pub fn get_implement_instructions() -> String {
    with_project_root_guard(include_str!("text/implement-change.md"))
}

// ---------------------------------------------------------------------------
// Explore
// ---------------------------------------------------------------------------

pub fn get_explore_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-explore".into(),
        description: "Enter explore mode - a thinking partner for exploring ideas, investigating problems, and clarifying requirements. Use when the user wants to think through something before or during a change. Also use when the user says \"speckit explore\" or \"speckit-explore\".".into(),
        instructions: with_project_root_guard(include_str!("text/explore.md")),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_specx_explore_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Explore".into(),
        description:
            "Enter explore mode - think through ideas, investigate problems, clarify requirements"
                .into(),
        category: "Workflow".into(),
        tags: vec![
            "workflow".into(),
            "explore".into(),
            "experimental".into(),
            "thinking".into(),
        ],
        content: with_project_root_guard(include_str!("text/explore.md")),
    }
}

// ---------------------------------------------------------------------------
// New Change
// ---------------------------------------------------------------------------

pub fn get_new_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-new-change".into(),
        description: "Start a new Speckit change using the experimental artifact workflow. Also use when the user says \"speckit new change\" or \"speckit-new-change\".".into(),
        instructions: with_project_root_guard(include_str!("text/new-change.md")),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_specx_new_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: New".into(),
        description: "Start a new change using the experimental artifact workflow".into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "artifacts".into(), "experimental".into()],
        content: with_project_root_guard(include_str!("text/new-change.md")),
    }
}

// ---------------------------------------------------------------------------
// Continue Change
// ---------------------------------------------------------------------------

pub fn get_continue_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-continue-change".into(),
        description: "Continue working on a Speckit change by creating the next artifact. Also use when the user says \"speckit continue\" or \"speckit-continue-change\".".into(),
        instructions: with_project_root_guard(include_str!("text/continue-change.md")),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_specx_continue_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Continue".into(),
        description: "Continue working on a change - create the next artifact (Experimental)"
            .into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "artifacts".into(), "experimental".into()],
        content: with_project_root_guard(include_str!("text/continue-change.md")),
    }
}

// ---------------------------------------------------------------------------
// Implement Change
// ---------------------------------------------------------------------------

pub fn get_implement_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-implement-change".into(),
        description: "Implement tasks from a Speckit change. Use when the user wants to start or continue implementing a change, or work through its tasks. Also use when the user says \"speckit implement\" or \"speckit-implement-change\".".into(),
        instructions: get_implement_instructions(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_specx_implement_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Implement".into(),
        description: "Implement tasks from a Speckit change (Experimental)".into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "artifacts".into(), "experimental".into()],
        content: get_implement_instructions(),
    }
}

// ---------------------------------------------------------------------------
// Update Change
// ---------------------------------------------------------------------------

pub fn get_update_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-update-change".into(),
        description: "Update a Speckit change by revising its existing planning artifacts and keeping them coherent with one another. Never edits code. Also use when the user says \"speckit update change\" or \"speckit-update-change\". If the user means the speckit update CLI command, which refreshes generated files, run that command instead.".into(),
        instructions: with_project_root_guard(include_str!("text/update-change.md")),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_specx_update_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Update".into(),
        description: "Update a change - revise existing planning artifacts and keep them coherent"
            .into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "artifacts".into(), "experimental".into()],
        content: with_project_root_guard(include_str!("text/update-change.md")),
    }
}

// ---------------------------------------------------------------------------
// Fast-Forward Change
// ---------------------------------------------------------------------------

pub fn get_ff_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-ff-change".into(),
        description: "Fast-forward through Speckit artifact creation - generate everything needed for implementation in one go. Also use when the user says \"speckit ff\" or \"speckit-ff-change\".".into(),
        instructions: with_project_root_guard(include_str!("text/ff-change.md")),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_specx_ff_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Fast Forward".into(),
        description:
            "Create a change and generate all artifacts needed for implementation in one go".into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "artifacts".into(), "experimental".into()],
        content: with_project_root_guard(include_str!("text/ff-change.md")),
    }
}

// ---------------------------------------------------------------------------
// Sync Specs
// ---------------------------------------------------------------------------

pub fn get_sync_specs_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-sync-specs".into(),
        description: "Sync delta specs from a change to main specs. Also use when the user says \"speckit sync\" or \"speckit-sync-specs\".".into(),
        instructions: with_project_root_guard(include_str!("text/sync-specs.md")),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_specx_sync_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Sync".into(),
        description: "Sync delta specs from a change to main specs".into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "specs".into(), "experimental".into()],
        content: with_project_root_guard(include_str!("text/sync-specs.md")),
    }
}

// ---------------------------------------------------------------------------
// Archive Change
// ---------------------------------------------------------------------------

pub fn get_archive_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-archive-change".into(),
        description: "Archive a completed change in the experimental workflow. Also use when the user says \"speckit archive\" or \"speckit-archive-change\".".into(),
        instructions: with_project_root_guard(include_str!("text/archive-change.md")),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_specx_archive_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Archive".into(),
        description: "Archive a completed change in the experimental workflow".into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "archive".into(), "experimental".into()],
        content: with_project_root_guard(include_str!("text/archive-change.md")),
    }
}

// ---------------------------------------------------------------------------
// Bulk Archive Change
// ---------------------------------------------------------------------------

pub fn get_bulk_archive_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-bulk-archive-change".into(),
        description: "Archive multiple completed changes at once. Also use when the user says \"speckit bulk archive\" or \"speckit-bulk-archive-change\".".into(),
        instructions: with_project_root_guard(include_str!("text/bulk-archive-change.md")),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_specx_bulk_archive_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Bulk Archive".into(),
        description: "Archive multiple completed changes at once".into(),
        category: "Workflow".into(),
        tags: vec![
            "workflow".into(),
            "archive".into(),
            "experimental".into(),
            "bulk".into(),
        ],
        content: with_project_root_guard(include_str!("text/bulk-archive-change.md")),
    }
}

// ---------------------------------------------------------------------------
// Verify Change
// ---------------------------------------------------------------------------

pub fn get_verify_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-verify-change".into(),
        description: "Verify implementation matches change artifacts. Also use when the user says \"speckit verify\" or \"speckit-verify-change\".".into(),
        instructions: with_project_root_guard(include_str!("text/verify-change.md")),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_specx_verify_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Verify".into(),
        description: "Verify implementation matches change artifacts before archiving".into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "verify".into(), "experimental".into()],
        content: with_project_root_guard(include_str!("text/verify-change.md")),
    }
}

// ---------------------------------------------------------------------------
// Onboard
// ---------------------------------------------------------------------------

pub fn get_onboard_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-onboard".into(),
        description: "Guided onboarding for Speckit - walk through a complete workflow cycle. Also use when the user says \"speckit onboard\" or \"speckit-onboard\"."
            .into(),
        instructions: with_project_root_guard(include_str!("text/onboard.md")),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_specx_onboard_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Onboard".into(),
        description: "Guided onboarding - walk through a complete Speckit workflow cycle".into(),
        category: "Workflow".into(),
        tags: vec![
            "workflow".into(),
            "onboarding".into(),
            "tutorial".into(),
            "learning".into(),
        ],
        content: with_project_root_guard(include_str!("text/onboard.md")),
    }
}

// ---------------------------------------------------------------------------
// Propose
// ---------------------------------------------------------------------------

pub fn get_propose_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-propose".into(),
        description: "Propose a new change with all artifacts generated in one step. Also use when the user says \"speckit propose\" or \"speckit-propose\".".into(),
        instructions: with_project_root_guard(include_str!("text/propose.md")),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_specx_propose_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Propose".into(),
        description: "Propose a new change - create it and generate all artifacts in one step"
            .into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "artifacts".into(), "experimental".into()],
        content: with_project_root_guard(include_str!("text/propose.md")),
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Canonical metadata block emitted by every canonical workflow template.
fn speckit_metadata() -> std::collections::HashMap<String, String> {
    let mut m = std::collections::HashMap::new();
    m.insert("author".into(), "speckit".into());
    m.insert("version".into(), "1.0".into());
    m
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// A sentence unique to the guard, so a test failure means the guard is
    /// really gone rather than that some other paragraph moved.
    const GUARD_MARKER: &str = "never create the root as a side effect";

    fn rendered_surfaces() -> Vec<(&'static str, String)> {
        vec![
            ("explore", get_explore_skill_template().instructions),
            ("new-change", get_new_change_skill_template().instructions),
            (
                "continue-change",
                get_continue_change_skill_template().instructions,
            ),
            (
                "implement-change",
                get_implement_change_skill_template().instructions,
            ),
            (
                "update-change",
                get_update_change_skill_template().instructions,
            ),
            ("ff-change", get_ff_change_skill_template().instructions),
            ("sync-specs", get_sync_specs_skill_template().instructions),
            (
                "archive-change",
                get_archive_change_skill_template().instructions,
            ),
            (
                "bulk-archive-change",
                get_bulk_archive_change_skill_template().instructions,
            ),
            (
                "verify-change",
                get_verify_change_skill_template().instructions,
            ),
            ("onboard", get_onboard_skill_template().instructions),
            ("propose", get_propose_skill_template().instructions),
            ("implement-instructions", get_implement_instructions()),
            (
                "explore command",
                get_specx_explore_command_template().content.clone(),
            ),
            (
                "new command",
                get_specx_new_command_template().content.clone(),
            ),
            (
                "continue command",
                get_specx_continue_command_template().content.clone(),
            ),
            (
                "implement command",
                get_specx_implement_command_template().content.clone(),
            ),
            (
                "update command",
                get_specx_update_command_template().content.clone(),
            ),
            (
                "ff command",
                get_specx_ff_command_template().content.clone(),
            ),
            (
                "sync command",
                get_specx_sync_command_template().content.clone(),
            ),
            (
                "archive command",
                get_specx_archive_command_template().content.clone(),
            ),
            (
                "bulk-archive command",
                get_specx_bulk_archive_command_template().content.clone(),
            ),
            (
                "verify command",
                get_specx_verify_command_template().content.clone(),
            ),
            (
                "onboard command",
                get_specx_onboard_command_template().content.clone(),
            ),
            (
                "propose command",
                get_specx_propose_command_template().content.clone(),
            ),
        ]
    }

    /// Every skill and every slash command must carry the project-root guard.
    /// A workflow that loses it silently is the failure this exists to prevent:
    /// in a repository with no `speckit/` root, the agent gets no signal that a
    /// write would create one.
    #[test]
    fn every_workflow_carries_the_project_root_guard() {
        for (name, body) in rendered_surfaces() {
            assert!(
                body.contains(GUARD_MARKER),
                "workflow {name:?} lost the project-root guard"
            );
            assert!(
                !body.contains(project_root::PROJECT_ROOT_GUARD_PLACEHOLDER),
                "workflow {name:?} emitted an unreplaced guard placeholder"
            );
        }
    }

    /// The guard lives once, in `project_root.rs`. A workflow text that grows a
    /// hand-written copy would drift from the shared one while still passing
    /// the check above, so the source files must reference it, never restate it.
    #[test]
    fn workflow_texts_reference_the_guard_instead_of_rewriting_it() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/templates/workflows/text");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).expect("workflow text directory") {
            let entry = entry.expect("readable workflow text entry");
            if entry.path().extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let body = std::fs::read_to_string(entry.path()).expect("readable workflow text");
            assert!(
                body.contains(project_root::PROJECT_ROOT_GUARD_PLACEHOLDER),
                "{} writes its own project-root guidance instead of including the shared guard",
                entry.path().display()
            );
            assert!(
                !body.contains(GUARD_MARKER),
                "{} duplicates the guard text",
                entry.path().display()
            );
            checked += 1;
        }
        assert_eq!(12, checked, "expected one workflow text per skill");
    }
}
