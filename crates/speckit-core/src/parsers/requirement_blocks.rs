/// Delta-spec and requirement-block parsing.
///
/// Provides the structured delta reader used by `validate <change>` and
/// `archive`.  Parses `## ADDED/MODIFIED/REMOVED/RENAMED Requirements`
/// sections, extracts `### Requirement:` blocks with their full raw content,
/// and performs scenario-loss analysis for MODIFIED requirements.
use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use super::code_fence::build_code_fence_mask;
use super::requirement_text::SCENARIO_HEADER;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single requirement block parsed from a `### Requirement:` header,
/// including its header line, normalised name, and the full raw content
/// (header + body + scenarios).
#[derive(Debug, Clone, PartialEq)]
pub struct RequirementBlock {
    /// The raw header line, e.g. `"### Requirement: Something"`.
    pub header_line: String,
    /// The normalised requirement name, e.g. `"Something"`.
    pub name: String,
    /// Full block text including `header_line` and all following content.
    pub raw: String,
}

/// Parsed structure of a `## Requirements` section.
#[derive(Debug, Clone)]
pub struct RequirementsSectionParts {
    /// Content before the `## Requirements` header.
    pub before: String,
    /// The `"## Requirements"` header line itself.
    pub header_line: String,
    /// Content between the header and the first requirement block.
    pub preamble: String,
    /// Parsed requirement blocks in document order.
    pub body_blocks: Vec<RequirementBlock>,
    /// Content after the requirements section.
    pub after: String,
}

/// A level-3 header inside a delta section that is not a canonical
/// `### Requirement:` header, recorded at the moment the delta reader skips
/// over it.  Surfaced as an INFO note by `validate <change>`.
#[derive(Debug, Clone, PartialEq)]
pub struct SkippedHeader {
    /// Header text without the leading `###`.
    pub header: String,
    /// The `##` section title as written.
    pub section: String,
    /// 1-based line number in the delta file.
    pub line: usize,
}

/// Parsed delta plan from a delta-formatted spec change file.
#[derive(Debug, Clone)]
pub struct DeltaPlan {
    pub added: Vec<RequirementBlock>,
    pub modified: Vec<RequirementBlock>,
    /// Requirement names from the REMOVED section.
    pub removed: Vec<String>,
    pub renamed: Vec<RenamePair>,
    /// Non-canonical `###` headers the reader skipped.
    pub skipped_headers: Vec<SkippedHeader>,
    /// Which delta sections were present in the document.
    pub section_presence: SectionPresence,
}

/// A rename pair from the RENAMED section.
#[derive(Debug, Clone, PartialEq)]
pub struct RenamePair {
    pub from: String,
    pub to: String,
}

/// Tracks which `## ... Requirements` sections were found.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionPresence {
    pub added: bool,
    pub modified: bool,
    pub removed: bool,
    pub renamed: bool,
}

// ---------------------------------------------------------------------------
// Regex constants
// ---------------------------------------------------------------------------

/// Canonical `### Requirement: <name>` header.
static REQUIREMENT_HEADER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^###\s*Requirement:\s*(.+)\s*$").unwrap());

/// Any level-2 header (`## ...`).
static TOP_LEVEL_SECTION_HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^##\s+").unwrap());

/// Bullet-list format of a requirement header in a REMOVED section:
/// `- ### Requirement: <name>`
static BULLET_REQUIREMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*-\s*`?###\s*Requirement:\s*(.+?)`?\s*$").unwrap());

/// Matches `FROM: ### Requirement: <name>`.
static RENAME_FROM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*-?\s*FROM:\s*`?###\s*Requirement:\s*(.+?)`?\s*$").unwrap()
});

/// Matches `TO: ### Requirement: <name>`.
static RENAME_TO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*-?\s*TO:\s*`?###\s*Requirement:\s*(.+?)`?\s*$").unwrap());

/// Matches an `## ADDED/MODIFIED/REMOVED/RENAMED Requirements` header.
static DELTA_SECTION_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^##\s+(ADDED|MODIFIED|REMOVED|RENAMED)\s+Requirements\s*$").unwrap()
});

/// Matches any `## ` header (top-level section boundary).
static H2_HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^##\s+").unwrap());

/// Matches a `### ...` header (level 3).
static H3_HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^###\s+(.+?)\s*$").unwrap());

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

/// Normalize a requirement name: trim leading/trailing whitespace.
pub fn normalize_requirement_name(name: &str) -> String {
    name.trim().to_string()
}

/// Case- and whitespace-insensitive fold of a requirement name.  Used for
/// typo detection -- near-miss REMOVED headers and the RENAMED+REMOVED
/// cross-section conflict.
pub fn fold_requirement_name(name: &str) -> String {
    normalize_requirement_name(name)
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Requirements section extraction
// ---------------------------------------------------------------------------

/// Extract the `## Requirements` section from a spec file and parse its
/// requirement blocks.
pub fn extract_requirements_section(content: &str) -> RequirementsSectionParts {
    let normalized = normalize_line_endings(content);
    let lines: Vec<String> = normalized.split('\n').map(String::from).collect();
    let fence_mask = build_code_fence_mask(&lines);

    // Find the `## Requirements` header (case-insensitive).
    let req_header_index = lines.iter().enumerate().find_map(|(i, l)| {
        if !fence_mask[i]
            && Regex::new(r"(?i)^##\s+Requirements\s*$")
                .unwrap()
                .is_match(l)
        {
            Some(i)
        } else {
            None
        }
    });

    let Some(req_header_index) = req_header_index else {
        // No requirements section; create an empty one at the end.
        let before = content.trim_end().to_string();
        let header_line = "## Requirements".to_string();
        let prefix = if before.is_empty() {
            String::new()
        } else {
            format!("{}\n\n", before)
        };
        return RequirementsSectionParts {
            before: prefix,
            header_line,
            preamble: String::new(),
            body_blocks: Vec::new(),
            after: "\n".to_string(),
        };
    };

    // Find end of this section: next line that starts with '## ' at same or
    // higher level.
    let mut end_index = lines.len();
    for i in (req_header_index + 1)..lines.len() {
        if !fence_mask[i] && TOP_LEVEL_SECTION_HEADER.is_match(&lines[i]) {
            end_index = i;
            break;
        }
    }

    let before_lines = &lines[..req_header_index];
    let before = before_lines.join("\n");
    let header_line = lines[req_header_index].clone();
    let section_body_lines = &lines[(req_header_index + 1)..end_index];
    let section_body_mask = &fence_mask[(req_header_index + 1)..end_index];

    // Parse requirement blocks within the section body.
    let mut blocks: Vec<RequirementBlock> = Vec::new();
    let mut cursor = 0;
    let mut preamble_lines: Vec<&str> = Vec::new();

    // Collect preamble lines until first requirement header.
    while cursor < section_body_lines.len() {
        if is_requirement_header(cursor, section_body_lines, section_body_mask) {
            break;
        }
        preamble_lines.push(&section_body_lines[cursor]);
        cursor += 1;
    }

    while cursor < section_body_lines.len() {
        if !is_requirement_header(cursor, section_body_lines, section_body_mask) {
            cursor += 1;
            continue;
        }
        let header_line_candidate = section_body_lines[cursor].clone();
        let name = REQUIREMENT_HEADER_REGEX
            .captures(&header_line_candidate)
            .map(|caps| normalize_requirement_name(&caps[1]))
            .unwrap_or_default();
        cursor += 1;

        // Gather lines until next requirement header or top-level header.
        let mut body_lines: Vec<String> = vec![header_line_candidate.clone()];
        while cursor < section_body_lines.len() {
            if is_requirement_header(cursor, section_body_lines, section_body_mask)
                || is_top_level_header(cursor, section_body_lines, section_body_mask)
            {
                break;
            }
            body_lines.push(section_body_lines[cursor].clone());
            cursor += 1;
        }
        let raw = body_lines.join("\n").trim_end().to_string();
        blocks.push(RequirementBlock {
            header_line: header_line_candidate,
            name,
            raw,
        });
    }

    let after_lines = &lines[end_index..];
    let after_raw = after_lines.join("\n");
    let after = if after_raw.starts_with('\n') {
        after_raw
    } else {
        format!("\n{}", after_raw)
    };
    let preamble = preamble_lines.join("\n").trim().to_string();
    let before_out = if before.trim_end().is_empty() {
        before
    } else {
        format!("{}\n", before)
    };

    RequirementsSectionParts {
        before: before_out,
        header_line,
        preamble,
        body_blocks: blocks,
        after,
    }
}

// ---------------------------------------------------------------------------
// Delta spec parsing
// ---------------------------------------------------------------------------

/// Parse a delta-formatted spec change file content into a [`DeltaPlan`].
pub fn parse_delta_spec(content: &str) -> DeltaPlan {
    let normalized = normalize_line_endings(content);
    let lines: Vec<String> = normalized.split('\n').map(String::from).collect();
    let fence_mask = build_code_fence_mask(&lines);
    let sections = split_top_level_sections(&lines, &fence_mask);

    let added_lookup = get_section_case_insensitive(&sections, "ADDED Requirements");
    let modified_lookup = get_section_case_insensitive(&sections, "MODIFIED Requirements");
    let removed_lookup = get_section_case_insensitive(&sections, "REMOVED Requirements");
    let renamed_lookup = get_section_case_insensitive(&sections, "RENAMED Requirements");

    let mut skipped_headers: Vec<SkippedHeader> = Vec::new();

    let added = parse_requirement_blocks_from_section(
        &added_lookup.body_lines,
        &added_lookup.body_mask,
        &mut skipped_headers,
        &added_lookup.title,
        added_lookup.body_start_line,
    );
    let modified = parse_requirement_blocks_from_section(
        &modified_lookup.body_lines,
        &modified_lookup.body_mask,
        &mut skipped_headers,
        &modified_lookup.title,
        modified_lookup.body_start_line,
    );
    let removed_names = parse_removed_names(&removed_lookup.body_lines, &removed_lookup.body_mask);
    let renamed_pairs = parse_renamed_pairs(&renamed_lookup.body_lines, &renamed_lookup.body_mask);
    skipped_headers.sort_by_key(|s| s.line);

    DeltaPlan {
        added,
        modified,
        removed: removed_names,
        renamed: renamed_pairs,
        skipped_headers,
        section_presence: SectionPresence {
            added: added_lookup.found,
            modified: modified_lookup.found,
            removed: removed_lookup.found,
            renamed: renamed_lookup.found,
        },
    }
}

/// Scenario names that the current requirement block has and the incoming
/// (MODIFIED) block does not.  A MODIFIED requirement replaces the whole
/// block, so every name reported here would be dropped from the main spec.
///
/// Multiplicity-aware: a name present N times in current and M times in
/// incoming means `max(0, N - M)` instances are missing.
pub fn find_missing_current_scenarios(
    current: &RequirementBlock,
    incoming: &RequirementBlock,
) -> Vec<String> {
    // Count incoming scenario names.
    let mut remaining_incoming: HashMap<String, usize> = HashMap::new();
    for scenario in parse_scenario_blocks(&incoming.raw) {
        *remaining_incoming.entry(scenario.name).or_insert(0) += 1;
    }

    // Walk current scenarios and report those that have no incoming counterpart.
    let mut missing: Vec<String> = Vec::new();
    for scenario in parse_scenario_blocks(&current.raw) {
        let entry = remaining_incoming.entry(scenario.name.clone()).or_insert(0);
        if *entry > 0 {
            *entry -= 1;
        } else {
            missing.push(scenario.name);
        }
    }

    missing
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

struct ScenarioBlock {
    name: String,
    #[allow(dead_code)]
    raw: String,
}

/// A slice of a document: its lines plus a parallel fence mask.
struct SectionBody {
    lines: Vec<String>,
    fence_mask: Vec<bool>,
    body_start_line: usize,
}

/// Result of looking up a top-level section by title.
struct SectionLookup {
    title: String,
    body_lines: Vec<String>,
    body_mask: Vec<bool>,
    body_start_line: usize,
    found: bool,
}

fn normalize_line_endings(content: &str) -> String {
    let stripped = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    stripped.replace("\r\n", "\n").replace('\r', "\n")
}

fn is_requirement_header(cursor: usize, lines: &[String], mask: &[bool]) -> bool {
    !mask[cursor] && REQUIREMENT_HEADER_REGEX.is_match(&lines[cursor])
}

fn is_top_level_header(cursor: usize, lines: &[String], mask: &[bool]) -> bool {
    !mask[cursor] && TOP_LEVEL_SECTION_HEADER.is_match(&lines[cursor])
}

/// Split the document into top-level (`##`) sections, each represented as a
/// [`SectionBody`] containing its body lines (everything after the header up
/// to the next `##` header or end of document).
fn split_top_level_sections(lines: &[String], fence_mask: &[bool]) -> HashMap<String, SectionBody> {
    let mut indices: Vec<(String, usize)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if fence_mask[i] {
            continue;
        }
        if let Some(caps) = Regex::new(r"^##\s+(.+)$").unwrap().captures(line) {
            indices.push((caps[1].trim().to_string(), i));
        }
    }

    let mut result: HashMap<String, SectionBody> = HashMap::new();
    for (idx, (title, start)) in indices.iter().enumerate() {
        let end = indices.get(idx + 1).map(|(_, i)| *i).unwrap_or(lines.len());
        result.insert(
            title.clone(),
            SectionBody {
                lines: lines[(start + 1)..end].to_vec(),
                fence_mask: fence_mask[(start + 1)..end].to_vec(),
                body_start_line: start + 2, // 1-based
            },
        );
    }

    result
}

fn get_section_case_insensitive(
    sections: &HashMap<String, SectionBody>,
    desired: &str,
) -> SectionLookup {
    let target = desired.to_lowercase();
    for (title, body) in sections {
        if title.to_lowercase() == target {
            return SectionLookup {
                title: title.clone(),
                body_lines: body.lines.clone(),
                body_mask: body.fence_mask.clone(),
                body_start_line: body.body_start_line,
                found: true,
            };
        }
    }
    SectionLookup {
        title: desired.to_string(),
        body_lines: Vec::new(),
        body_mask: Vec::new(),
        body_start_line: 0,
        found: false,
    }
}

/// Parse `### Requirement:` blocks from a delta section body.
///
/// Non-canonical `###` headers are recorded in `skipped_headers` so the
/// validator can surface them as INFO notes.
fn parse_requirement_blocks_from_section(
    lines: &[String],
    fence_mask: &[bool],
    skipped_headers: &mut Vec<SkippedHeader>,
    section_name: &str,
    body_start_line: usize,
) -> Vec<RequirementBlock> {
    if lines.is_empty() {
        return Vec::new();
    }

    let mut blocks: Vec<RequirementBlock> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        // Seek next requirement header, recording any skipped `###` headers.
        while i < lines.len() && !is_requirement_header(i, lines, fence_mask) {
            record_if_skipped_header(
                i,
                lines,
                fence_mask,
                skipped_headers,
                section_name,
                body_start_line,
            );
            i += 1;
        }
        if i >= lines.len() {
            break;
        }

        let header_line = lines[i].clone();
        let name = REQUIREMENT_HEADER_REGEX
            .captures(&header_line)
            .map(|caps| normalize_requirement_name(&caps[1]))
            .unwrap_or_default();

        let mut buf: Vec<String> = vec![header_line.clone()];
        i += 1;

        while i < lines.len() {
            if is_requirement_header(i, lines, fence_mask)
                || is_top_level_header(i, lines, fence_mask)
            {
                break;
            }
            record_if_skipped_header(
                i,
                lines,
                fence_mask,
                skipped_headers,
                section_name,
                body_start_line,
            );
            buf.push(lines[i].clone());
            i += 1;
        }

        blocks.push(RequirementBlock {
            header_line,
            name,
            raw: buf.join("\n").trim_end().to_string(),
        });
    }

    blocks
}

/// If line `index` is a non-canonical `###` header (not `### Requirement:`),
/// record it in the skipped-headers vector.
fn record_if_skipped_header(
    index: usize,
    lines: &[String],
    fence_mask: &[bool],
    skipped_headers: &mut Vec<SkippedHeader>,
    section_name: &str,
    body_start_line: usize,
) {
    if fence_mask[index] {
        return;
    }
    if let Some(caps) = H3_HEADER.captures(&lines[index])
        && !REQUIREMENT_HEADER_REGEX.is_match(&lines[index]) {
            skipped_headers.push(SkippedHeader {
                header: caps[1].trim().to_string(),
                section: section_name.to_string(),
                line: body_start_line + index,
            });
        }
}

/// Parse requirement names from a REMOVED section.  Supports both canonical
/// `### Requirement:` headers and bullet-list format (`- ### Requirement:`).
fn parse_removed_names(lines: &[String], fence_mask: &[bool]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if fence_mask[i] {
            continue;
        }
        if let Some(caps) = REQUIREMENT_HEADER_REGEX.captures(line) {
            names.push(normalize_requirement_name(&caps[1]));
            continue;
        }
        if let Some(caps) = BULLET_REQUIREMENT.captures(line) {
            names.push(normalize_requirement_name(&caps[1]));
        }
    }
    names
}

/// Parse `FROM:` / `TO:` rename pairs from a RENAMED section.
fn parse_renamed_pairs(lines: &[String], fence_mask: &[bool]) -> Vec<RenamePair> {
    let mut pairs: Vec<RenamePair> = Vec::new();
    let mut current_from: Option<String> = None;

    for (i, line) in lines.iter().enumerate() {
        if fence_mask[i] {
            continue;
        }
        if let Some(caps) = RENAME_FROM.captures(line) {
            current_from = Some(normalize_requirement_name(&caps[1]));
        } else if let Some(caps) = RENAME_TO.captures(line) {
            let to = normalize_requirement_name(&caps[1]);
            if let Some(from) = current_from.take() {
                pairs.push(RenamePair { from, to });
            }
        }
    }

    pairs
}

/// Parse scenario blocks from a requirement's raw content.  A scenario is
/// any non-fenced `####` header -- matching the spec path's `SCENARIO_HEADER`
/// / `count_scenarios` exactly.
fn parse_scenario_blocks(requirement_raw: &str) -> Vec<ScenarioBlock> {
    let normalized = requirement_raw.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<String> = normalized.split('\n').map(String::from).collect();
    let mask = build_code_fence_mask(&lines);
    let mut scenarios: Vec<ScenarioBlock> = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        if mask[index] || !SCENARIO_HEADER.is_match(&lines[index]) {
            index += 1;
            continue;
        }

        let start = index;
        let name = scenario_name_at(&lines[index]);
        index += 1;

        while index < lines.len() {
            if !mask[index] && SCENARIO_HEADER.is_match(&lines[index]) {
                break;
            }
            index += 1;
        }

        scenarios.push(ScenarioBlock {
            name,
            raw: lines[start..index].join("\n").trim_end().to_string(),
        });
    }

    scenarios
}

/// Extract the scenario name from a `####` header line, stripping the
/// leading `####`, optional ATX closing `#` runs, and an optional
/// `Scenario:` prefix.
fn scenario_name_at(line: &str) -> String {
    // Strip leading `#### `.
    let without_header = SCENARIO_HEADER.replace(line, "");
    // Strip optional ATX closing sequence (` ###` or `\t###`).
    let atx_re = Regex::new(r"[ \t]+#+[ \t]*$").unwrap();
    let without_atx = atx_re.replace(&without_header, "");
    // Strip optional `Scenario:` prefix.
    let scenario_prefix = Regex::new(r"(?i)^Scenario:\s*").unwrap();
    scenario_prefix.replace(&without_atx, "").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_requirement_name_trims() {
        assert_eq!(normalize_requirement_name("  Foo Bar  "), "Foo Bar");
    }

    #[test]
    fn fold_requirement_name_case_and_whitespace() {
        assert_eq!(fold_requirement_name("  Foo  Bar  "), "foo bar");
        assert_eq!(fold_requirement_name("FOO BAR"), "foo bar");
    }

    #[test]
    fn extract_requirements_section_basic() {
        let content = "\
## Purpose
Some purpose.

## Requirements

### Requirement: Auth
The system SHALL authenticate users.

#### Scenario: Valid credentials
WHEN valid credentials are provided
THEN the user is authenticated

### Requirement: Logging
The system MUST log all events.
";
        let parts = extract_requirements_section(content);
        assert_eq!(parts.body_blocks.len(), 2);
        assert_eq!(parts.body_blocks[0].name, "Auth");
        assert_eq!(parts.body_blocks[1].name, "Logging");
    }

    #[test]
    fn extract_requirements_section_missing() {
        let content = "## Purpose\nJust a purpose.\n";
        let parts = extract_requirements_section(content);
        assert!(parts.body_blocks.is_empty());
        assert_eq!(parts.header_line, "## Requirements");
    }

    #[test]
    fn extract_requirements_section_with_preamble() {
        let content = "\
## Requirements

Some preamble text.

### Requirement: First
Body.
";
        let parts = extract_requirements_section(content);
        assert_eq!(parts.preamble, "Some preamble text.");
        assert_eq!(parts.body_blocks.len(), 1);
    }

    #[test]
    fn parse_delta_spec_added() {
        let content = "\
## ADDED Requirements

### Requirement: New Feature
The system SHALL support new feature.

#### Scenario: Happy path
WHEN used
THEN works
";
        let plan = parse_delta_spec(content);
        assert!(plan.section_presence.added);
        assert_eq!(plan.added.len(), 1);
        assert_eq!(plan.added[0].name, "New Feature");
    }

    #[test]
    fn parse_delta_spec_all_sections() {
        let content = "\
## ADDED Requirements

### Requirement: Added Req
Body.

## MODIFIED Requirements

### Requirement: Modified Req
Body.

## REMOVED Requirements

### Requirement: Removed Req

## RENAMED Requirements

- FROM: ### Requirement: Old Name
- TO: ### Requirement: New Name
";
        let plan = parse_delta_spec(content);
        assert!(plan.section_presence.added);
        assert!(plan.section_presence.modified);
        assert!(plan.section_presence.removed);
        assert!(plan.section_presence.renamed);
        assert_eq!(plan.added.len(), 1);
        assert_eq!(plan.modified.len(), 1);
        assert_eq!(plan.removed.len(), 1);
        assert_eq!(plan.removed[0], "Removed Req");
        assert_eq!(plan.renamed.len(), 1);
        assert_eq!(plan.renamed[0].from, "Old Name");
        assert_eq!(plan.renamed[0].to, "New Name");
    }

    #[test]
    fn parse_delta_spec_skips_non_canonical_headers() {
        let content = "\
## ADDED Requirements

### Documentation Requirements

### Requirement: Real Req
Body.
";
        let plan = parse_delta_spec(content);
        assert_eq!(plan.added.len(), 1);
        assert_eq!(plan.added[0].name, "Real Req");
        assert_eq!(plan.skipped_headers.len(), 1);
        assert_eq!(plan.skipped_headers[0].header, "Documentation Requirements");
        assert_eq!(plan.skipped_headers[0].section, "ADDED Requirements");
    }

    #[test]
    fn find_missing_current_scenarios_basic() {
        let current = RequirementBlock {
            header_line: "### Requirement: R".to_string(),
            name: "R".to_string(),
            raw: "### Requirement: R\n#### Scenario: A\nWHEN X\n#### Scenario: B\nWHEN Y"
                .to_string(),
        };
        let incoming = RequirementBlock {
            header_line: "### Requirement: R".to_string(),
            name: "R".to_string(),
            raw: "### Requirement: R\n#### Scenario: A\nWHEN X".to_string(),
        };
        let missing = find_missing_current_scenarios(&current, &incoming);
        assert_eq!(missing, vec!["B"]);
    }

    #[test]
    fn find_missing_current_scenarios_multiplicity() {
        let current = RequirementBlock {
            header_line: "### Requirement: R".to_string(),
            name: "R".to_string(),
            raw: "\
### Requirement: R
#### Scenario: A
WHEN X
#### Scenario: A
WHEN Y
#### Scenario: A
WHEN Z"
                .to_string(),
        };
        let incoming = RequirementBlock {
            header_line: "### Requirement: R".to_string(),
            name: "R".to_string(),
            raw: "\
### Requirement: R
#### Scenario: A
WHEN X
#### Scenario: A
WHEN Y"
                .to_string(),
        };
        let missing = find_missing_current_scenarios(&current, &incoming);
        assert_eq!(missing, vec!["A"]); // 3 current - 2 incoming = 1 missing
    }

    #[test]
    fn find_missing_current_scenarios_none_missing() {
        let current = RequirementBlock {
            header_line: "### Requirement: R".to_string(),
            name: "R".to_string(),
            raw: "### Requirement: R\n#### Scenario: A\nWHEN X".to_string(),
        };
        let incoming = RequirementBlock {
            header_line: "### Requirement: R".to_string(),
            name: "R".to_string(),
            raw: "### Requirement: R\n#### Scenario: A\nWHEN X\n#### Scenario: B\nWHEN Y"
                .to_string(),
        };
        let missing = find_missing_current_scenarios(&current, &incoming);
        assert!(missing.is_empty());
    }

    #[test]
    fn scenario_name_atx_stripping() {
        assert_eq!(scenario_name_at("#### Foo ####"), "Foo");
        assert_eq!(scenario_name_at("#### Scenario: Bar ####"), "Bar");
        assert_eq!(scenario_name_at("#### Baz"), "Baz");
    }

    #[test]
    fn parse_removed_names_bullet_format() {
        let lines: Vec<String> = vec![
            "- ### Requirement: Old Req".to_string(),
            "- `### Requirement: Another`".to_string(),
        ];
        let mask = vec![false, false];
        let names = parse_removed_names(&lines, &mask);
        assert_eq!(names, vec!["Old Req", "Another"]);
    }

    #[test]
    fn code_fences_ignored_in_delta_sections() {
        let content = "\
## ADDED Requirements

```
### Requirement: Fake
```

### Requirement: Real
Body.
";
        let plan = parse_delta_spec(content);
        assert_eq!(plan.added.len(), 1);
        assert_eq!(plan.added[0].name, "Real");
    }
}
