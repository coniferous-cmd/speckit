//! Workflow template modules for Speckit skills and slash commands.
//!
//! Each public function returns a `SkillTemplate` or `CommandTemplate` for one
//! of the Speckit workflow operations (explore, new, continue, apply, etc.).
//!
//! Instruction bodies are loaded from `text/<workflow>.md` via `include_str!`
//! so the canonical text is byte-equivalent to the OpenSpec source after the
//! documented brand substitution (`openspec` -> `speckit`, `OpenSpec` ->
//! `Speckit`, `/opsx:` -> `/speckit:`). Do not duplicate or rewrite this text
//! inline; the parity tests depend on a single source of truth.

use super::types::{CommandTemplate, SkillTemplate};

// ---------------------------------------------------------------------------
// Apply (shared instructions used by both skill and command surfaces)
// ---------------------------------------------------------------------------

/// Shared apply workflow instructions. The body is loaded from
/// `text/apply-change.md` and reused by the skill and command surfaces.
pub fn get_apply_instructions() -> String {
    include_str!("text/apply-change.md").to_string()
}

// ---------------------------------------------------------------------------
// Explore
// ---------------------------------------------------------------------------

pub fn get_explore_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-explore".into(),
        description: "Enter explore mode - a thinking partner for exploring ideas, investigating problems, and clarifying requirements. Use when the user wants to think through something before or during a change.".into(),
        instructions: include_str!("text/explore.md").to_string(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_opsx_explore_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Explore".into(),
        description: "Enter explore mode - think through ideas, investigate problems, clarify requirements".into(),
        category: "Workflow".into(),
        tags: vec![
            "workflow".into(),
            "explore".into(),
            "experimental".into(),
            "thinking".into(),
        ],
        content: include_str!("text/explore.md").to_string(),
    }
}

// ---------------------------------------------------------------------------
// New Change
// ---------------------------------------------------------------------------

pub fn get_new_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-new-change".into(),
        description: "Start a new Speckit change using the experimental artifact workflow.".into(),
        instructions: include_str!("text/new-change.md").to_string(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_opsx_new_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: New".into(),
        description: "Start a new change using the experimental artifact workflow".into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "artifacts".into(), "experimental".into()],
        content: include_str!("text/new-change.md").to_string(),
    }
}

// ---------------------------------------------------------------------------
// Continue Change
// ---------------------------------------------------------------------------

pub fn get_continue_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-continue-change".into(),
        description: "Continue working on an Speckit change by creating the next artifact.".into(),
        instructions: include_str!("text/continue-change.md").to_string(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_opsx_continue_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Continue".into(),
        description: "Continue working on a change - create the next artifact (Experimental)"
            .into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "artifacts".into(), "experimental".into()],
        content: include_str!("text/continue-change.md").to_string(),
    }
}

// ---------------------------------------------------------------------------
// Apply Change
// ---------------------------------------------------------------------------

pub fn get_apply_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-apply-change".into(),
        description: "Implement tasks from an Speckit change.".into(),
        instructions: get_apply_instructions(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_opsx_apply_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Apply".into(),
        description: "Implement tasks from an Speckit change (Experimental)".into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "artifacts".into(), "experimental".into()],
        content: get_apply_instructions(),
    }
}

// ---------------------------------------------------------------------------
// Update Change
// ---------------------------------------------------------------------------

pub fn get_update_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-update-change".into(),
        description: "Update an Speckit change by revising its existing planning artifacts and keeping them coherent with one another. Never edits code.".into(),
        instructions: include_str!("text/update-change.md").to_string(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_opsx_update_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Update".into(),
        description: "Update a change - revise existing planning artifacts and keep them coherent"
            .into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "artifacts".into(), "experimental".into()],
        content: include_str!("text/update-change.md").to_string(),
    }
}

// ---------------------------------------------------------------------------
// Fast-Forward Change
// ---------------------------------------------------------------------------

pub fn get_ff_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-ff-change".into(),
        description: "Fast-forward through Speckit artifact creation - generate everything needed for implementation in one go.".into(),
        instructions: include_str!("text/ff-change.md").to_string(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_opsx_ff_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Fast Forward".into(),
        description:
            "Create a change and generate all artifacts needed for implementation in one go".into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "artifacts".into(), "experimental".into()],
        content: include_str!("text/ff-change.md").to_string(),
    }
}

// ---------------------------------------------------------------------------
// Sync Specs
// ---------------------------------------------------------------------------

pub fn get_sync_specs_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-sync-specs".into(),
        description: "Sync delta specs from a change to main specs.".into(),
        instructions: include_str!("text/sync-specs.md").to_string(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_opsx_sync_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Sync".into(),
        description: "Sync delta specs from a change to main specs".into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "specs".into(), "experimental".into()],
        content: include_str!("text/sync-specs.md").to_string(),
    }
}

// ---------------------------------------------------------------------------
// Archive Change
// ---------------------------------------------------------------------------

pub fn get_archive_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-archive-change".into(),
        description: "Archive a completed change in the experimental workflow.".into(),
        instructions: include_str!("text/archive-change.md").to_string(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_opsx_archive_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Archive".into(),
        description: "Archive a completed change in the experimental workflow".into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "archive".into(), "experimental".into()],
        content: include_str!("text/archive-change.md").to_string(),
    }
}

// ---------------------------------------------------------------------------
// Bulk Archive Change
// ---------------------------------------------------------------------------

pub fn get_bulk_archive_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-bulk-archive-change".into(),
        description: "Archive multiple completed changes at once.".into(),
        instructions: include_str!("text/bulk-archive-change.md").to_string(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_opsx_bulk_archive_command_template() -> CommandTemplate {
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
        content: include_str!("text/bulk-archive-change.md").to_string(),
    }
}

// ---------------------------------------------------------------------------
// Verify Change
// ---------------------------------------------------------------------------

pub fn get_verify_change_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-verify-change".into(),
        description: "Verify implementation matches change artifacts.".into(),
        instructions: include_str!("text/verify-change.md").to_string(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_opsx_verify_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Verify".into(),
        description: "Verify implementation matches change artifacts before archiving".into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "verify".into(), "experimental".into()],
        content: include_str!("text/verify-change.md").to_string(),
    }
}

// ---------------------------------------------------------------------------
// Onboard
// ---------------------------------------------------------------------------

pub fn get_onboard_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-onboard".into(),
        description: "Guided onboarding for Speckit - walk through a complete workflow cycle."
            .into(),
        instructions: include_str!("text/onboard.md").to_string(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_opsx_onboard_command_template() -> CommandTemplate {
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
        content: include_str!("text/onboard.md").to_string(),
    }
}

// ---------------------------------------------------------------------------
// Propose
// ---------------------------------------------------------------------------

pub fn get_propose_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "speckit-propose".into(),
        description: "Propose a new change with all artifacts generated in one step.".into(),
        instructions: include_str!("text/propose.md").to_string(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
    }
}

pub fn get_opsx_propose_command_template() -> CommandTemplate {
    CommandTemplate {
        name: "SPECKIT: Propose".into(),
        description: "Propose a new change - create it and generate all artifacts in one step"
            .into(),
        category: "Workflow".into(),
        tags: vec!["workflow".into(), "artifacts".into(), "experimental".into()],
        content: include_str!("text/propose.md").to_string(),
    }
}

// ---------------------------------------------------------------------------
// Feedback (not part of the canonical 12-workflow registry, kept for legacy)
// ---------------------------------------------------------------------------

pub fn get_feedback_skill_template() -> SkillTemplate {
    SkillTemplate {
        name: "feedback".into(),
        description: "Collect and submit user feedback about Speckit with context enrichment and anonymization.".into(),
        instructions: r#"Help the user submit feedback about Speckit.

**Goal**: Guide the user through collecting, enriching, and submitting feedback while ensuring privacy through anonymization.

**Process**

1. **Gather context from the conversation**
2. **Draft enriched feedback** - clear title, body with context
3. **Anonymize sensitive information** - paths, tokens, company names
4. **Present draft for approval**
5. **Submit on confirmation** - `speckit feedback "title" --body "body content"`

**Guardrails**
- MUST show complete draft before submitting
- MUST ask for explicit approval
- MUST anonymize sensitive information
- DO NOT submit without user confirmation"#
            .into(),
        license: Some("MIT".into()),
        compatibility: Some("Requires speckit CLI.".into()),
        metadata: Some(speckit_metadata()),
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