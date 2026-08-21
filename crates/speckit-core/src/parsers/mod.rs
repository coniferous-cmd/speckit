pub mod change_parser;
/// Speckit Markdown parsers.
///
/// This module provides parsers for the two main Speckit document types:
///
/// * **Specs** -- `## Purpose` + `## Requirements` with `### Requirement:` blocks.
/// * **Changes** -- `## Why` + `## What Changes` with delta specs under `specs/`.
///
/// Code fences are masked throughout so Markdown structure inside fenced
/// blocks is never parsed.
pub mod code_fence;
pub mod markdown;
pub mod requirement_blocks;
pub mod requirement_text;
pub mod spec_structure;

// Re-export the most commonly used types at the parsers level for convenience.

pub use code_fence::build_code_fence_mask;

pub use markdown::{MarkdownParser, Section};

pub use change_parser::ChangeParser;

pub use requirement_text::{
    contains_shall_or_must, count_scenarios, extract_requirement_body, extract_requirement_text,
};

pub use requirement_blocks::{
    DeltaPlan, RenamePair, RequirementBlock, RequirementsSectionParts, SectionPresence,
    SkippedHeader, extract_requirements_section, find_missing_current_scenarios,
    fold_requirement_name, normalize_requirement_name, parse_delta_spec,
};

pub use spec_structure::{
    MainSpecIssueKind, MainSpecStructureIssue, find_main_spec_structure_issues,
    strip_fenced_code_blocks_preserving_lines,
};
