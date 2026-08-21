/// Validation threshold constants.

/// Minimum character lengths.
pub const MIN_WHY_SECTION_LENGTH: usize = 50;
pub const MIN_PURPOSE_LENGTH: usize = 50;

/// Maximum character/item limits.
pub const MAX_WHY_SECTION_LENGTH: usize = 1000;
pub const MAX_REQUIREMENT_TEXT_LENGTH: usize = 500;
pub const MAX_DELTAS_PER_CHANGE: usize = 10;

/// Minimum length for a delta description (used in applyChangeRules).
pub const MIN_DELTA_DESCRIPTION_LENGTH: usize = 10;

// ---------------------------------------------------------------------------
// Validation messages
// ---------------------------------------------------------------------------

// Required content
pub const SCENARIO_EMPTY: &str = "Scenario text cannot be empty";
pub const REQUIREMENT_EMPTY: &str = "Requirement text cannot be empty";
pub const REQUIREMENT_NO_SHALL: &str = "Requirement must contain SHALL or MUST keyword";
pub const REQUIREMENT_NO_SCENARIOS: &str = "Requirement must have at least one scenario";
pub const SPEC_NAME_EMPTY: &str = "Spec name cannot be empty";
pub const SPEC_PURPOSE_EMPTY: &str = "Purpose section cannot be empty";
pub const SPEC_NO_REQUIREMENTS: &str = "Spec must have at least one requirement";
pub const CHANGE_NAME_EMPTY: &str = "Change name cannot be empty";
pub const CHANGE_WHAT_EMPTY: &str = "What Changes section cannot be empty";
pub const CHANGE_NO_DELTAS: &str = "Change must have at least one delta";
pub const DELTA_SPEC_EMPTY: &str = "Spec name cannot be empty";
pub const DELTA_DESCRIPTION_EMPTY: &str = "Delta description cannot be empty";

// Messages that include interpolated constants
pub fn change_why_too_short() -> String {
    format!("Why section must be at least {MIN_WHY_SECTION_LENGTH} characters")
}

pub fn change_why_too_long() -> String {
    format!("Why section should not exceed {MAX_WHY_SECTION_LENGTH} characters")
}

pub fn change_too_many_deltas() -> String {
    format!("Consider splitting changes with more than {MAX_DELTAS_PER_CHANGE} deltas")
}

// skip_specs messages
pub const CHANGE_SKIP_SPECS_CONFLICT: &str = "skip_specs is set in .speckit.yaml but spec files exist under specs/. Remove skip_specs or delete the delta spec files";
pub const CHANGE_SKIP_SPECS_ACCEPTED: &str = "skip_specs is set in .speckit.yaml: change declares no spec-level behavior changes, zero deltas accepted";
pub const CHANGE_SKIP_SPECS_INVALID_METADATA: &str = "skip_specs is set but .speckit.yaml is not valid change metadata, so the marker is not honored. Fix the metadata";

// Warnings
pub fn purpose_too_brief() -> String {
    format!("Purpose section is too brief (less than {MIN_PURPOSE_LENGTH} characters)")
}

pub fn requirement_too_long() -> String {
    format!(
        "Requirement text is very long (>{MAX_REQUIREMENT_TEXT_LENGTH} characters). Consider breaking it down."
    )
}

pub const DELTA_DESCRIPTION_TOO_BRIEF: &str = "Delta description is too brief";
pub const DELTA_MISSING_REQUIREMENTS: &str = "Delta should include requirements";

// Guidance snippets (appended to primary messages for remediation)
pub const GUIDE_NO_DELTAS: &str = "No deltas found. Ensure your change has a specs/ directory with capability folders (e.g. specs/http-server/spec.md) containing .md files that use delta headers (## ADDED/MODIFIED/REMOVED/RENAMED Requirements) and that each requirement includes at least one \"#### Scenario:\" block. If this change intentionally modifies no specs (pure refactor, tooling, docs), set \"skip_specs: true\" in the change's .speckit.yaml instead. Tip: run \"speckit change show <change-id> --json --deltas-only\" to inspect parsed deltas.";

pub const GUIDE_MISSING_SPEC_SECTIONS: &str = "Missing required sections. Expected headers: \"## Purpose\" and \"## Requirements\". Example:\n## Purpose\n[brief purpose]\n\n## Requirements\n### Requirement: Clear requirement statement\nUsers SHALL ...\n\n#### Scenario: Descriptive name\n- **WHEN** ...\n- **THEN** ...";

pub const GUIDE_MISSING_CHANGE_SECTIONS: &str = "Missing required sections. Expected headers: \"## Why\" and \"## What Changes\". Ensure deltas are documented in specs/ using delta headers.";

pub const GUIDE_SCENARIO_FORMAT: &str = "Scenarios must use level-4 headers. Convert bullet lists into:\n#### Scenario: Short name\n- **WHEN** ...\n- **THEN** ...\n- **AND** ...";
