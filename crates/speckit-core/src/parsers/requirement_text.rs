use regex::Regex;
/// Shared, fence-aware requirement-reading helpers.
///
/// The requirement reader used to be implemented twice -- once for main specs
/// and once for change deltas -- and the two drifted apart.  These helpers are
/// the single source of truth for requirement-body extraction, scenario
/// counting, and `SHALL`/`MUST` detection.
use std::sync::LazyLock;

use super::code_fence::build_code_fence_mask;

// Re-export so existing importers keep working.
pub use super::code_fence::build_code_fence_mask as build_code_fence_mask_reexport;

/// Lines that look like `**ID**: ...` / `**Priority**: ...` metadata.
static METADATA_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\*\*[^*]+\*\*:").unwrap());

/// Any markdown header line -- the boundary where a requirement body ends.
static HEADER_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,6}\s").unwrap());

/// A level-4 header.  Deliberately matches ANY `####` header, not only
/// `#### Scenario:` -- the spec path treats every level-4 child of a
/// requirement as a scenario, so the delta counter must too (parity).
pub static SCENARIO_HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^####\s+").unwrap());

/// The one predicate for normative-keyword detection.  Matches `SHALL` or
/// `MUST` as whole words so the change-delta reader and the schema-based
/// reader accept and reject identical text.
pub fn contains_shall_or_must(text: &str) -> bool {
    Regex::new(r"(?i)\b(SHALL|MUST)\b").unwrap().is_match(text)
}

/// Extract the full requirement body from the lines that follow a
/// `### Requirement:` header (the lines may include scenarios and fenced code).
///
/// Captures every body line from the start up to the first header found on a
/// non-fenced line -- usually the first `#### Scenario:`, but also a stray
/// `###` divider the delta reader absorbed into the block -- skipping blank
/// lines and any line inside a fenced code block.  `**metadata**:` lines are
/// skipped only when other body text remains: a requirement written entirely
/// as `**Constraint**: The system MUST ...` keeps that line as its body.
/// Captured lines are trimmed and joined with newlines so a requirement whose
/// text wraps across lines -- or whose `SHALL`/`MUST` lands on a later line
/// -- is read in full.
pub fn extract_requirement_body(body_lines: &[String]) -> String {
    let mask = build_code_fence_mask(body_lines);
    let mut captured: Vec<String> = Vec::new();
    let mut metadata: Vec<String> = Vec::new();

    for (i, line) in body_lines.iter().enumerate() {
        if mask[i] {
            continue; // inside a fenced code block
        }
        if HEADER_LINE.is_match(line) {
            break; // first scenario or stray divider
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue; // blank
        }
        if METADATA_LINE.is_match(trimmed) {
            metadata.push(trimmed.to_string()); // **ID**: / **Priority**: ...
        } else {
            captured.push(trimmed.to_string());
        }
    }

    if !captured.is_empty() {
        captured.join("\n")
    } else {
        metadata.join("\n") // metadata-only body: the metadata IS the body
    }
}

/// Parser/display fallback for a requirement block with no body text.  This is
/// what lets a bare `### The system SHALL ...` header remain readable on the
/// spec path (the title is the requirement).
pub fn extract_requirement_text(header_title: &str, body_lines: &[String]) -> String {
    let body = extract_requirement_body(body_lines);
    if body.is_empty() {
        header_title.trim().to_string()
    } else {
        body
    }
}

/// Count the real scenarios in a requirement block: `#### ` headers on
/// non-fenced lines.  A `#### Scenario:` that lives inside a fenced example
/// is not a real scenario and is not counted.
pub fn count_scenarios(body_lines: &[String]) -> usize {
    let mask = build_code_fence_mask(body_lines);
    body_lines
        .iter()
        .enumerate()
        .filter(|(i, line)| !mask[*i] && SCENARIO_HEADER.is_match(line))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_strings(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn shall_detection() {
        assert!(contains_shall_or_must("The system SHALL do X"));
        assert!(contains_shall_or_must("The system MUST do X"));
        assert!(!contains_shall_or_must("The system should do X"));
    }

    #[test]
    fn shall_as_word_boundary() {
        assert!(!contains_shall_or_must("The marshaller works"));
    }

    #[test]
    fn extract_body_basic() {
        let lines = to_strings(&["The system SHALL process requests.", "It MUST be fast."]);
        assert_eq!(
            extract_requirement_body(&lines),
            "The system SHALL process requests.\nIt MUST be fast."
        );
    }

    #[test]
    fn extract_body_stops_at_header() {
        let lines = to_strings(&["Body text", "#### Scenario: foo", "WHEN bar", "THEN baz"]);
        assert_eq!(extract_requirement_body(&lines), "Body text");
    }

    #[test]
    fn extract_body_skips_blank_lines() {
        let lines = to_strings(&["First line", "", "Third line"]);
        assert_eq!(extract_requirement_body(&lines), "First line\nThird line");
    }

    #[test]
    fn extract_body_skips_metadata_when_body_exists() {
        let lines = to_strings(&["**ID**: REQ-1", "The system SHALL do X."]);
        assert_eq!(extract_requirement_body(&lines), "The system SHALL do X.");
    }

    #[test]
    fn extract_body_keeps_metadata_when_only_content() {
        let lines = to_strings(&["**ID**: REQ-1", "**Priority**: high"]);
        assert_eq!(
            extract_requirement_body(&lines),
            "**ID**: REQ-1\n**Priority**: high"
        );
    }

    #[test]
    fn extract_body_skips_fenced_lines() {
        let lines = to_strings(&["Body text", "```", "### Fake header", "```", "More body"]);
        assert_eq!(extract_requirement_body(&lines), "Body text\nMore body");
    }

    #[test]
    fn extract_text_fallback_to_title() {
        let lines: Vec<String> = vec![];
        assert_eq!(
            extract_requirement_text("The system SHALL do X", &lines),
            "The system SHALL do X"
        );
    }

    #[test]
    fn extract_text_uses_body_when_present() {
        let lines = to_strings(&["Detailed requirement text."]);
        assert_eq!(
            extract_requirement_text("Header", &lines),
            "Detailed requirement text."
        );
    }

    #[test]
    fn count_scenarios_basic() {
        let lines = to_strings(&[
            "Body",
            "#### Scenario: A",
            "WHEN X",
            "#### Scenario: B",
            "WHEN Y",
        ]);
        assert_eq!(count_scenarios(&lines), 2);
    }

    #[test]
    fn count_scenarios_ignores_fenced() {
        let lines = to_strings(&[
            "Body",
            "```",
            "#### Scenario: Fake",
            "```",
            "#### Scenario: Real",
        ]);
        assert_eq!(count_scenarios(&lines), 1);
    }

    #[test]
    fn count_scenarios_any_level4_header() {
        let lines = to_strings(&[
            "Body",
            "#### Not labeled scenario",
            "#### Scenario: Labeled",
        ]);
        assert_eq!(count_scenarios(&lines), 2);
    }
}
