/// Main-spec structural issue detection.
///
/// Finds structural problems in main specification files that would cause
/// silent data loss or invisible requirements:
///
/// * Delta headers (`## ADDED Requirements`, etc.) that belong only in
///   change delta specs, not in main specs.
/// * `### Requirement:` headers outside the `## Requirements` section.
/// * Duplicate requirement names within `## Requirements`.
use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use super::code_fence::build_code_fence_mask;

// ---------------------------------------------------------------------------
// Regex constants
// ---------------------------------------------------------------------------

static REQUIREMENTS_SECTION_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^##\s+Requirements\s*$").unwrap());

static TOP_LEVEL_SECTION_HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^##\s+").unwrap());

static DELTA_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^##\s+(ADDED|MODIFIED|REMOVED|RENAMED)\s+Requirements\s*$").unwrap()
});

static REQUIREMENT_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^###\s+Requirement:\s*(.+)\s*$").unwrap());

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The kind of structural issue found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MainSpecIssueKind {
    /// A delta header (`## ADDED Requirements`, etc.) was found in a main spec.
    DeltaHeader,
    /// A `### Requirement:` header appears outside the `## Requirements` section.
    RequirementOutsideRequirements,
    /// Two `### Requirement:` headers share the same name.
    DuplicateRequirement,
}

/// A single structural issue found in a main spec file.
#[derive(Debug, Clone)]
pub struct MainSpecStructureIssue {
    pub kind: MainSpecIssueKind,
    /// 1-based line number.
    pub line: usize,
    /// The header text (trimmed).
    pub header: String,
    /// Human-readable message describing the issue.
    pub message: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scan a main spec file for structural issues.
///
/// The content is expected to be a main specification (not a delta spec).
/// Issues detected:
///
/// 1. **Delta headers** -- `## ADDED/MODIFIED/REMOVED/RENAMED Requirements`
///    are only valid inside change delta specs.
/// 2. **Requirement outside Requirements** -- `### Requirement:` headers
///    outside the `## Requirements` section are invisible to validate, list,
///    and archive.
/// 3. **Duplicate requirements** -- Two `### Requirement:` headers with the
///    same name within `## Requirements`.
pub fn find_main_spec_structure_issues(content: &str) -> Vec<MainSpecStructureIssue> {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let stripped = strip_fenced_code_blocks_preserving_lines(&normalized);
    let lines: Vec<&str> = stripped.split('\n').collect();
    let mut issues: Vec<MainSpecStructureIssue> = Vec::new();
    let mut requirement_lines: HashMap<String, usize> = HashMap::new();

    // Locate the `## Requirements` section boundaries.
    let requirements_header_index = lines
        .iter()
        .position(|line| REQUIREMENTS_SECTION_HEADER.is_match(line));

    let mut requirements_end_index = lines.len();
    if let Some(start) = requirements_header_index {
        for i in (start + 1)..lines.len() {
            if TOP_LEVEL_SECTION_HEADER.is_match(lines[i]) {
                requirements_end_index = i;
                break;
            }
        }
    }

    // Tracks the line index of the most recently seen delta header.  When
    // `Some(end)`, a `### Requirement:` header nested inside the delta
    // section is considered part of the delta section rather than a
    // standalone requirement header outside the canonical `## Requirements`
    // section.
    let mut delta_section_end: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Close the current delta section when we encounter a different
        // top-level section header (i.e. anything starting with `## ` that
        // is not itself a delta header).
        if delta_section_end.is_some() && TOP_LEVEL_SECTION_HEADER.is_match(line) {
            delta_section_end = None;
        }

        // Check for delta headers (invalid in main specs).
        if DELTA_HEADER.is_match(line) {
            issues.push(MainSpecStructureIssue {
                kind: MainSpecIssueKind::DeltaHeader,
                line: i + 1,
                header: trimmed.to_string(),
                message: format!(
                    "Main spec contains delta header \"{}\". \
                     Delta headers are only valid inside \
                     speckit/changes/<name>/specs/<capability-path>/spec.md \
                     and truncate the parsed ## Requirements section.",
                    trimmed
                ),
            });
            // Anything after the delta header, up to the next top-level
            // section, belongs to the (invalid) delta section.
            delta_section_end = Some(lines.len());
            continue;
        }

        // Check for requirement headers.
        let requirement_match = REQUIREMENT_HEADER.captures(line);
        let Some(_caps) = requirement_match else {
            continue;
        };

        let inside_requirements = requirements_header_index.is_some()
            && i > requirements_header_index.unwrap()
            && i < requirements_end_index;
        let inside_delta_section = delta_section_end.is_some();
        let has_requirements_section = requirements_header_index.is_some();

        // A `### Requirement:` header inside a delta section is only
        // considered "inside" when the document has no canonical
        // `## Requirements` section of its own.  When both sections are
        // present, requirement headers appearing before the
        // `## Requirements` section are still flagged as outside.
        let suppress_outside = inside_delta_section && !has_requirements_section;

        if !inside_requirements && !suppress_outside {
            issues.push(MainSpecStructureIssue {
                kind: MainSpecIssueKind::RequirementOutsideRequirements,
                line: i + 1,
                header: trimmed.to_string(),
                message: format!(
                    "Requirement header \"{}\" appears outside the main \
                     ## Requirements section. Main specs only parse \
                     requirements inside that section, so this requirement \
                     is currently invisible to validate, list, and archive.",
                    trimmed
                ),
            });
            continue;
        }

        // Check for duplicate requirement names.
        let requirement_name = REQUIREMENT_HEADER
            .captures(line)
            .map(|c| c[1].trim().to_string())
            .unwrap_or_default();

        if let Some(&previous_line) = requirement_lines.get(&requirement_name) {
            issues.push(MainSpecStructureIssue {
                kind: MainSpecIssueKind::DuplicateRequirement,
                line: i + 1,
                header: trimmed.to_string(),
                message: format!(
                    "Requirement header \"{}\" duplicates the requirement \
                     declared on line {}. Requirement names must be unique \
                     so spec updates cannot discard one block while updating \
                     another.",
                    trimmed, previous_line
                ),
            });
        } else {
            requirement_lines.insert(requirement_name, i + 1);
        }
    }

    issues
}

/// Strip fenced code blocks from content while preserving line count.
///
/// Lines inside fenced code blocks are replaced with empty strings so that
/// line numbers in the output match the original document.
pub fn strip_fenced_code_blocks_preserving_lines(content: &str) -> String {
    let lines: Vec<String> = content.split('\n').map(String::from).collect();
    let mask = build_code_fence_mask(&lines);
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| if mask[i] { String::new() } else { line.clone() })
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_issues_in_clean_spec() {
        let content = "\
## Purpose
The purpose of this spec.

## Requirements

### Requirement: Auth
The system SHALL authenticate users.

#### Scenario: Valid login
WHEN valid credentials
THEN authenticated
";
        let issues = find_main_spec_structure_issues(content);
        assert!(issues.is_empty());
    }

    #[test]
    fn detects_delta_header() {
        let content = "\
## Purpose
Purpose.

## ADDED Requirements

### Requirement: New
Body.
";
        let issues = find_main_spec_structure_issues(content);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind, MainSpecIssueKind::DeltaHeader);
    }

    #[test]
    fn detects_requirement_outside_requirements() {
        let content = "\
## Purpose
Purpose.

### Requirement: Rogue
Body.

## Requirements

### Requirement: Legit
Body.
";
        let issues = find_main_spec_structure_issues(content);
        assert!(
            issues
                .iter()
                .any(|i| i.kind == MainSpecIssueKind::RequirementOutsideRequirements)
        );
    }

    #[test]
    fn detects_duplicate_requirement() {
        let content = "\
## Requirements

### Requirement: Duplicate
First instance.

### Requirement: Duplicate
Second instance.
";
        let issues = find_main_spec_structure_issues(content);
        let dupes: Vec<_> = issues
            .iter()
            .filter(|i| i.kind == MainSpecIssueKind::DuplicateRequirement)
            .collect();
        assert_eq!(dupes.len(), 1);
        assert!(dupes[0].message.contains("line "));
    }

    #[test]
    fn code_fences_ignored() {
        let content = "\
## Requirements

```
### Requirement: Fake
```

### Requirement: Real
Body.
";
        let issues = find_main_spec_structure_issues(content);
        // The fake requirement inside the fence should not be detected.
        let rogue: Vec<_> = issues
            .iter()
            .filter(|i| i.kind == MainSpecIssueKind::RequirementOutsideRequirements)
            .collect();
        assert!(rogue.is_empty());
    }

    #[test]
    fn strip_preserves_line_count() {
        let content = "line1\n```\nfake\n```\nline5";
        let stripped = strip_fenced_code_blocks_preserving_lines(content);
        let lines: Vec<&str> = stripped.split('\n').collect();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "line1");
        assert_eq!(lines[1], "");
        assert_eq!(lines[2], "");
        assert_eq!(lines[3], "");
        assert_eq!(lines[4], "line5");
    }

    #[test]
    fn multiple_issues_detected() {
        let content = "\
## Purpose
Purpose.

## ADDED Requirements

### Requirement: Outside
Body.

## Requirements

### Requirement: Inside
Body.

### Requirement: Inside
Duplicate.
";
        let issues = find_main_spec_structure_issues(content);
        assert!(issues.len() >= 3); // delta header + outside + duplicate
    }
}
