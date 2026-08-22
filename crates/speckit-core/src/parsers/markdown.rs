use anyhow::{Result, bail};
use regex::Regex;
/// Markdown parser for Speckit documents.
///
/// Parses Markdown content into a section tree, then extracts structured
/// `Spec` or `Change` documents from it.  Handles BOM stripping, `\r\n`
/// normalization, and nested heading levels.  Code fences are masked so
/// Markdown structure inside fenced blocks is ignored.
use std::sync::LazyLock;

use crate::schemas::{
    Change, ChangeMetadata, Delta, DeltaOperation, Requirement, Scenario, Spec, SpecMetadata,
};

use super::code_fence::build_code_fence_mask;
use super::requirement_text::extract_requirement_text;

/// Matches a Markdown ATX header: one to six `#` characters, whitespace, then
/// the title text.  Capture group 1 is the `#` run, group 2 is the title.
static HEADER_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.+)$").unwrap());

/// Matches just the `#` prefix and trailing whitespace of a header (used to
/// detect section boundaries without capturing the title).  Capture group 1
/// is the leading `#` run so the caller can read the level without scanning
/// the whole match.
static HEADER_LEVEL_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#{1,6})\s+").unwrap());

/// A parsed Markdown section (heading + body + nested children).
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub level: usize,
    pub title: String,
    pub content: String,
    pub children: Vec<Section>,
}

/// Stateful Markdown parser that splits content into lines, masks fenced code
/// blocks, and builds a hierarchical section tree.
pub struct MarkdownParser {
    lines: Vec<String>,
    code_fence_line_mask: Vec<bool>,
}

impl MarkdownParser {
    /// Create a new parser from raw Markdown content.  The content is
    /// normalized (BOM stripped, `\r\n` -> `\n`) before processing.
    pub fn new(content: &str) -> Self {
        let normalized = Self::normalize_content(content);
        let lines: Vec<String> = normalized.split('\n').map(String::from).collect();
        let code_fence_line_mask = build_code_fence_mask(&lines);
        Self {
            lines,
            code_fence_line_mask,
        }
    }

    /// Strip a UTF-8 BOM so a header on the first line still matches, and
    /// normalize line endings to `\n`.
    pub fn normalize_content(content: &str) -> String {
        let stripped = content.strip_prefix('\u{FEFF}').unwrap_or(content);
        stripped.replace("\r\n", "\n").replace('\r', "\n")
    }

    // ------------------------------------------------------------------
    // Section tree construction
    // ------------------------------------------------------------------

    /// Parse the stored content into a tree of [`Section`]s.
    ///
    /// Each Markdown heading (`#` through `######`) becomes a section node.
    /// Children are headings with a greater `#` count that appear before the
    /// next heading at the same or shallower level.
    pub fn parse_sections(&self) -> Vec<Section> {
        // First pass: collect every header as a flat (level, title, content) tuple.
        let mut flat: Vec<(usize, String, String)> = Vec::new();
        for i in 0..self.lines.len() {
            if self.code_fence_line_mask[i] {
                continue;
            }
            if let Some(caps) = HEADER_REGEX.captures(&self.lines[i]) {
                let level = caps[1].len();
                let title = caps[2].trim().to_string();
                let content = self.get_content_until_next_header(i + 1, level);
                flat.push((level, title, content));
            }
        }

        // Second pass: build the tree using a stack.
        Self::build_section_tree(flat)
    }

    /// Assemble a flat list of `(level, title, content)` tuples into a
    /// hierarchical section tree.
    fn build_section_tree(flat: Vec<(usize, String, String)>) -> Vec<Section> {
        let mut root: Vec<Section> = Vec::new();
        let mut stack: Vec<Section> = Vec::new();

        for (level, title, content) in flat {
            // Pop and nest any sections at the same or deeper level.
            while stack.last().is_some_and(|s| s.level >= level) {
                let finished = stack.pop().unwrap();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(finished);
                } else {
                    root.push(finished);
                }
            }

            stack.push(Section {
                level,
                title,
                content,
                children: Vec::new(),
            });
        }

        // Flush the remaining stack.
        while let Some(section) = stack.pop() {
            if let Some(parent) = stack.last_mut() {
                parent.children.push(section);
            } else {
                root.push(section);
            }
        }

        root
    }

    /// Collect the body lines that follow a header, stopping when the next
    /// header at the same or shallower level is reached.  Lines inside fenced
    /// code blocks are included as ordinary content.
    fn get_content_until_next_header(&self, start_line: usize, current_level: usize) -> String {
        let mut content_lines: Vec<&str> = Vec::new();

        for i in start_line..self.lines.len() {
            let is_header = if self.code_fence_line_mask[i] {
                false
            } else {
                HEADER_LEVEL_REGEX
                    .captures(&self.lines[i])
                    .is_some_and(|caps| caps[1].len() <= current_level)
            };

            if is_header {
                break;
            }

            content_lines.push(&self.lines[i]);
        }

        content_lines.join("\n").trim().to_string()
    }

    // ------------------------------------------------------------------
    // Section lookup
    // ------------------------------------------------------------------

    /// Recursively search `sections` (depth-first) for the first section
    /// whose title matches `title` (case-insensitive).
    pub fn find_section<'a>(sections: &'a [Section], title: &str) -> Option<&'a Section> {
        let target = title.to_lowercase();
        for section in sections {
            if section.title.to_lowercase() == target {
                return Some(section);
            }
            if let Some(found) = Self::find_section(&section.children, title) {
                return Some(found);
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Spec parsing
    // ------------------------------------------------------------------

    /// Parse the stored content as an Speckit specification document.
    ///
    /// # Errors
    /// Returns an error if the document is missing a `## Purpose` or
    /// `## Requirements` section.
    pub fn parse_spec(&self, name: &str) -> Result<Spec> {
        let sections = self.parse_sections();
        let purpose = Self::find_section(&sections, "Purpose")
            .map(|s| s.content.clone())
            .unwrap_or_default();
        let requirements_section = Self::find_section(&sections, "Requirements");

        if purpose.is_empty() {
            bail!("Spec must have a Purpose section");
        }
        let requirements_section = match requirements_section {
            Some(s) => s,
            None => bail!("Spec must have a Requirements section"),
        };

        let requirements = self.parse_requirements(requirements_section);

        Ok(Spec {
            name: name.to_string(),
            overview: purpose.trim().to_string(),
            requirements,
            metadata: Some(SpecMetadata {
                version: "1.0.0".to_string(),
                format: "speckit".to_string(),
                source_path: None,
            }),
        })
    }

    // ------------------------------------------------------------------
    // Change parsing (simple format, no delta spec files)
    // ------------------------------------------------------------------

    /// Parse the stored content as an Speckit change document.
    ///
    /// # Errors
    /// Returns an error if the document is missing a `## Why` or
    /// `## What Changes` section.
    pub fn parse_change(&self, name: &str) -> Result<Change> {
        let sections = self.parse_sections();
        let why = Self::find_section(&sections, "Why")
            .map(|s| s.content.clone())
            .unwrap_or_default();
        let what_changes = Self::find_section(&sections, "What Changes")
            .map(|s| s.content.clone())
            .unwrap_or_default();

        if why.is_empty() {
            bail!("Change must have a Why section");
        }
        if what_changes.is_empty() {
            bail!("Change must have a What Changes section");
        }

        let deltas = self.parse_deltas(&what_changes);

        Ok(Change {
            name: name.to_string(),
            why: why.trim().to_string(),
            what_changes: what_changes.trim().to_string(),
            deltas,
            metadata: Some(ChangeMetadata {
                version: "1.0.0".to_string(),
                format: "speckit-change".to_string(),
                source_path: None,
            }),
        })
    }

    // ------------------------------------------------------------------
    // Requirement / scenario helpers
    // ------------------------------------------------------------------

    /// Parse requirements from a `## Requirements` section.  Every direct
    /// child heading is treated as a requirement.
    pub(crate) fn parse_requirements(&self, section: &Section) -> Vec<Requirement> {
        section
            .children
            .iter()
            .map(|child| {
                let body_lines: Vec<String> = child.content.split('\n').map(String::from).collect();
                let text = extract_requirement_text(&child.title, &body_lines);
                let scenarios = self.parse_scenarios(child);
                Requirement {
                    name: child.title.clone(),
                    text,
                    scenarios,
                }
            })
            .collect()
    }

    /// Parse scenarios from a requirement section.  Each direct child whose
    /// content is non-empty becomes a [`Scenario`].
    fn parse_scenarios(&self, requirement_section: &Section) -> Vec<Scenario> {
        requirement_section
            .children
            .iter()
            .filter(|child| !child.content.trim().is_empty())
            .map(|child| Scenario {
                name: child.title.clone(),
                raw_text: child.content.clone(),
            })
            .collect()
    }

    // ------------------------------------------------------------------
    // Delta parsing (simple bullet-list format in "What Changes")
    // ------------------------------------------------------------------

    /// Parse delta entries from the "What Changes" section body.
    ///
    /// Recognizes bullet lines of the form:
    /// - `**spec_name:** description`
    /// - `**spec_name**: description`
    ///
    /// The operation (ADDED / MODIFIED / REMOVED / RENAMED) is inferred from
    /// keywords in the description.
    pub(crate) fn parse_deltas(&self, content: &str) -> Vec<Delta> {
        let delta_line_re = Regex::new(r"(?m)^\s*-\s*\*\*([^*:]+)(?::\*\*|\*\*:)\s*(.+)$").unwrap();
        let renamed_re =
            Regex::new(r"(?i)\brename(?:s|d|ing)?\b|\brenamed\s+(?:to|from)\b").unwrap();
        let added_re =
            Regex::new(r"(?i)\badd(?:s|ed|ing)?\b|\bcreate(?:s|d|ing)?\b|\bnew\b").unwrap();
        let removed_re = Regex::new(r"(?i)\bremove(?:s|d|ing)?\b|\bdelete(?:s|d|ing)?\b").unwrap();

        let mut deltas: Vec<Delta> = Vec::new();

        for caps in delta_line_re.captures_iter(content) {
            let spec_name = caps[1].trim().to_string();
            let description = caps[2].trim().to_string();
            let lower_desc = description.to_lowercase();

            // Check RENAMED first since it's more specific than patterns
            // containing "new" (which would otherwise match ADDED).
            let operation = if renamed_re.is_match(&lower_desc) {
                DeltaOperation::Renamed
            } else if added_re.is_match(&lower_desc) {
                DeltaOperation::Added
            } else if removed_re.is_match(&lower_desc) {
                DeltaOperation::Removed
            } else {
                DeltaOperation::Modified
            };

            deltas.push(Delta {
                spec: spec_name,
                operation,
                description,
                requirement: None,
                requirements: Vec::new(),
                rename: None,
            });
        }

        deltas
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_bom() {
        let input = "\u{FEFF}# Hello";
        assert_eq!(MarkdownParser::normalize_content(input), "# Hello");
    }

    #[test]
    fn normalize_crlf() {
        let input = "line1\r\nline2\rline3\nline4";
        assert_eq!(
            MarkdownParser::normalize_content(input),
            "line1\nline2\nline3\nline4"
        );
    }

    #[test]
    fn parse_sections_flat() {
        // Two siblings at the same heading level remain top-level sections.
        let parser = MarkdownParser::new("## Title\nBody\n## Second\nMore");
        let sections = parser.parse_sections();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "Title");
        assert_eq!(sections[0].level, 2);
        assert_eq!(sections[1].title, "Second");
        assert_eq!(sections[1].level, 2);
    }

    #[test]
    fn parse_sections_nested() {
        let content = "## A\nbody A\n### B\nbody B\n### C\nbody C\n## D\nbody D";
        let parser = MarkdownParser::new(content);
        let sections = parser.parse_sections();
        assert_eq!(sections.len(), 2); // A and D
        assert_eq!(sections[0].title, "A");
        assert_eq!(sections[0].children.len(), 2); // B and C
        assert_eq!(sections[0].children[0].title, "B");
        assert_eq!(sections[0].children[1].title, "C");
        assert_eq!(sections[1].title, "D");
    }

    #[test]
    fn find_section_case_insensitive() {
        let content = "## Purpose\nHello\n## Requirements\n### Req\n...";
        let parser = MarkdownParser::new(content);
        let sections = parser.parse_sections();
        assert!(MarkdownParser::find_section(&sections, "purpose").is_some());
        assert!(MarkdownParser::find_section(&sections, "PURPOSE").is_some());
        assert!(MarkdownParser::find_section(&sections, "Requirements").is_some());
        assert!(MarkdownParser::find_section(&sections, "missing").is_none());
    }

    #[test]
    fn code_fences_not_parsed_as_headers() {
        let content = "## Real\n```\n## Fake\n```\n## Also Real";
        let parser = MarkdownParser::new(content);
        let sections = parser.parse_sections();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "Real");
        assert_eq!(sections[1].title, "Also Real");
    }

    #[test]
    fn parse_spec_requires_purpose_and_requirements() {
        let content = "## Purpose\nThe purpose.\n## Requirements\n### Requirement: A\nThe system SHALL do A.\n#### Scenario: S1\nWHEN X\nTHEN Y";
        let parser = MarkdownParser::new(content);
        let spec = parser.parse_spec("test-spec").unwrap();
        assert_eq!(spec.name, "test-spec");
        assert_eq!(spec.overview, "The purpose.");
        assert_eq!(spec.requirements.len(), 1);
    }

    #[test]
    fn parse_spec_fails_without_purpose() {
        let content = "## Requirements\n### Req\n...";
        let parser = MarkdownParser::new(content);
        assert!(parser.parse_spec("test").is_err());
    }

    #[test]
    fn parse_change_basic() {
        let content = "## Why\nWe need this change because it is important for the system.\n## What Changes\n- **auth-spec:** Add new OAuth requirement";
        let parser = MarkdownParser::new(content);
        let change = parser.parse_change("test-change").unwrap();
        assert_eq!(change.name, "test-change");
        assert!(change.why.contains("important"));
        assert_eq!(change.deltas.len(), 1);
        assert_eq!(change.deltas[0].spec, "auth-spec");
    }

    #[test]
    fn parse_deltas_operations() {
        let parser = MarkdownParser::new("");
        let content = "\
- **spec-a:** Add new authentication
- **spec-b:** Remove old endpoint
- **spec-c:** Rename the requirement
- **spec-d:** Update the config";
        let deltas = parser.parse_deltas(content);
        assert_eq!(deltas.len(), 4);
        assert_eq!(deltas[0].operation, DeltaOperation::Added);
        assert_eq!(deltas[1].operation, DeltaOperation::Removed);
        assert_eq!(deltas[2].operation, DeltaOperation::Renamed);
        assert_eq!(deltas[3].operation, DeltaOperation::Modified);
    }

    #[test]
    fn parse_deltas_colon_inside_bold() {
        let parser = MarkdownParser::new("");
        let content = "- **spec-name:** Some description";
        let deltas = parser.parse_deltas(content);
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].spec, "spec-name");
    }
}
