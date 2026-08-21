/// Change proposal parser.
///
/// Extends [`MarkdownParser`] semantics for change proposals by adding support
/// for delta-formatted spec files under a `specs/` subdirectory.  When delta
/// spec files are present their structured deltas take precedence over the
/// simple bullet-list format in the "What Changes" section.
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{Result, bail};
use regex::Regex;
use walkdir::WalkDir;

use crate::schemas::{
    Change, ChangeMetadata, Delta, DeltaOperation, RenameDescriptor, Requirement,
};

use super::markdown::{MarkdownParser, Section};

/// A discovered delta spec file under the `specs/` directory.
struct DiscoveredSpec {
    /// Spec id relative to the specs root, forward-slash separated (e.g.
    /// `"web"` or `"platform/session-layout"`).
    id: String,
    /// Absolute (or specs-root-relative) path to the `spec.md` file.
    spec_file: PathBuf,
}

/// Regex that recognises canonical `### Requirement:` headers (case
/// insensitive).  Used by the overridden `parse_requirements` to skip
/// non-canonical divider headers inside delta sections.
static REQUIREMENT_HEADER_FILTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^Requirement:\s*\S").unwrap());

/// Regex that matches `FROM: ### Requirement: <name>` rename lines.
static RENAME_FROM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*-?\s*FROM:\s*`?###\s*Requirement:\s*(.+?)`?\s*$").unwrap()
});

/// Regex that matches `TO: ### Requirement: <name>` rename lines.
static RENAME_TO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*-?\s*TO:\s*`?###\s*Requirement:\s*(.+?)`?\s*$").unwrap());

/// Change parser with delta-spec awareness.
///
/// Holds a [`MarkdownParser`] for the core section/requirement parsing, plus
/// the filesystem path of the change directory so it can discover and read
/// `specs/` delta files.
pub struct ChangeParser {
    parser: MarkdownParser,
    change_dir: PathBuf,
}

impl ChangeParser {
    /// Create a new change parser.
    ///
    /// * `content` -- raw Markdown of the change proposal (same content you
    ///   would pass to [`MarkdownParser::new`]).
    /// * `change_dir` -- path to the change directory (the parent of the
    ///   `specs/` subdirectory, if any).
    pub fn new(content: &str, change_dir: impl Into<PathBuf>) -> Self {
        Self {
            parser: MarkdownParser::new(content),
            change_dir: change_dir.into(),
        }
    }

    /// Parse the change proposal, including any delta spec files found under
    /// `specs/` within the change directory.
    ///
    /// When delta spec files exist and produce at least one delta they take
    /// precedence over the simple bullet-list format parsed from the
    /// "What Changes" section.
    ///
    /// # Errors
    /// Returns an error if required sections ("Why", "What Changes") are
    /// missing.
    pub async fn parse_change_with_deltas(&self, name: &str) -> Result<Change> {
        let sections = self.parser.parse_sections();
        let why = MarkdownParser::find_section(&sections, "Why")
            .map(|s| s.content.clone())
            .unwrap_or_default();
        let what_changes = MarkdownParser::find_section(&sections, "What Changes")
            .map(|s| s.content.clone())
            .unwrap_or_default();

        if why.is_empty() {
            bail!("Change must have a Why section");
        }
        if what_changes.is_empty() {
            bail!("Change must have a What Changes section");
        }

        // Parse deltas from the What Changes section (simple format).
        let simple_deltas = self.parser.parse_deltas(&what_changes);

        // Check if there are spec files with delta format.
        let specs_dir = self.change_dir.join("specs");
        let delta_deltas = self.parse_delta_specs(&specs_dir).await;

        // Combine both types, preferring delta format when available.
        let deltas = if delta_deltas.is_empty() {
            simple_deltas
        } else {
            delta_deltas
        };

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
    // Requirement parsing override
    // ------------------------------------------------------------------

    /// Parse requirements from a delta section, ignoring headers that are not
    /// `### Requirement: <name>`.
    ///
    /// A delta section often carries divider headers such as
    /// `### Documentation Requirements`.  The base parser treats every child
    /// header as a requirement, which invents a scenario-less requirement that
    /// does not exist.  This override keeps only canonical `### Requirement:`
    /// children.
    fn parse_requirements(&self, section: &Section) -> Vec<Requirement> {
        let filtered = Section {
            level: section.level,
            title: section.title.clone(),
            content: section.content.clone(),
            children: section
                .children
                .iter()
                .filter(|child| REQUIREMENT_HEADER_FILTER.is_match(child.title.trim()))
                .cloned()
                .collect(),
        };
        self.parser.parse_requirements(&filtered)
    }

    // ------------------------------------------------------------------
    // Delta spec file parsing
    // ------------------------------------------------------------------

    /// Walk `specs_dir` recursively, read every `spec.md`, and extract deltas
    /// from each one.
    async fn parse_delta_specs(&self, specs_dir: &Path) -> Vec<Delta> {
        let mut deltas: Vec<Delta> = Vec::new();
        let spec_files = discover_spec_files(specs_dir);

        for discovered in spec_files {
            match tokio::fs::read_to_string(&discovered.spec_file).await {
                Ok(content) => {
                    let spec_deltas = self.parse_spec_deltas(&discovered.id, &content);
                    deltas.extend(spec_deltas);
                }
                Err(_) => {
                    // Spec file might not be readable, which is okay.
                    continue;
                }
            }
        }

        deltas
    }

    /// Parse a single delta spec file into [`Delta`] entries.
    ///
    /// Recognises `## ADDED Requirements`, `## MODIFIED Requirements`,
    /// `## REMOVED Requirements`, and `## RENAMED Requirements` sections.
    fn parse_spec_deltas(&self, spec_name: &str, content: &str) -> Vec<Delta> {
        let mut deltas: Vec<Delta> = Vec::new();
        let sections = Self::parse_sections_from_content(content);

        // ADDED requirements
        if let Some(added_section) = MarkdownParser::find_section(&sections, "ADDED Requirements") {
            for req in self.parse_requirements(added_section) {
                deltas.push(Delta {
                    spec: spec_name.to_string(),
                    operation: DeltaOperation::Added,
                    description: format!("Add requirement: {}", req.text),
                    requirement: Some(req.clone()),
                    requirements: vec![req],
                    rename: None,
                });
            }
        }

        // MODIFIED requirements
        if let Some(modified_section) =
            MarkdownParser::find_section(&sections, "MODIFIED Requirements")
        {
            for req in self.parse_requirements(modified_section) {
                deltas.push(Delta {
                    spec: spec_name.to_string(),
                    operation: DeltaOperation::Modified,
                    description: format!("Modify requirement: {}", req.text),
                    requirement: Some(req.clone()),
                    requirements: vec![req],
                    rename: None,
                });
            }
        }

        // REMOVED requirements
        if let Some(removed_section) =
            MarkdownParser::find_section(&sections, "REMOVED Requirements")
        {
            for req in self.parse_requirements(removed_section) {
                deltas.push(Delta {
                    spec: spec_name.to_string(),
                    operation: DeltaOperation::Removed,
                    description: format!("Remove requirement: {}", req.text),
                    requirement: Some(req.clone()),
                    requirements: vec![req],
                    rename: None,
                });
            }
        }

        // RENAMED requirements
        if let Some(renamed_section) =
            MarkdownParser::find_section(&sections, "RENAMED Requirements")
        {
            for rename in Self::parse_renames(&renamed_section.content) {
                deltas.push(Delta {
                    spec: spec_name.to_string(),
                    operation: DeltaOperation::Renamed,
                    description: format!(
                        "Rename requirement from \"{}\" to \"{}\"",
                        rename.from, rename.to
                    ),
                    requirement: None,
                    requirements: Vec::new(),
                    rename: Some(rename),
                });
            }
        }

        deltas
    }

    /// Parse `FROM:` / `TO:` rename pairs from a "RENAMED Requirements"
    /// section body.
    fn parse_renames(content: &str) -> Vec<RenameDescriptor> {
        let normalized = MarkdownParser::normalize_content(content);
        let mut renames: Vec<RenameDescriptor> = Vec::new();
        let mut current_from: Option<String> = None;

        for line in normalized.split('\n') {
            if let Some(caps) = RENAME_FROM.captures(line) {
                current_from = Some(caps[1].trim().to_string());
            } else if let Some(caps) = RENAME_TO.captures(line) {
                let to = caps[1].trim().to_string();
                if let Some(from) = current_from.take() {
                    renames.push(RenameDescriptor { from, to });
                }
            }
        }

        renames
    }

    /// Parse sections from an arbitrary content string (used to read delta
    /// spec files that are not the main change document).
    fn parse_sections_from_content(content: &str) -> Vec<Section> {
        MarkdownParser::new(content).parse_sections()
    }
}

/// Walk `specs_root` recursively and discover every `spec.md` file.
///
/// Returns a list of discovered specs with their id (the relative directory
/// path, e.g. `"web"` or `"platform/session-layout"`) and file path.
fn discover_spec_files(specs_root: &Path) -> Vec<DiscoveredSpec> {
    let mut result = Vec::new();

    if !specs_root.is_dir() {
        return result;
    }

    for entry in WalkDir::new(specs_root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name() != "spec.md" {
            continue;
        }

        let spec_file = entry.path().to_path_buf();
        let relative = entry
            .path()
            .strip_prefix(specs_root)
            .unwrap_or(entry.path());
        let id = relative
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        result.push(DiscoveredSpec { id, spec_file });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_renames_basic() {
        let content = "\
- FROM: `### Requirement: Old Name`
- TO: `### Requirement: New Name`";
        let renames = ChangeParser::parse_renames(content);
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].from, "Old Name");
        assert_eq!(renames[0].to, "New Name");
    }

    #[test]
    fn parse_renames_multiple() {
        let content = "\
- FROM: ### Requirement: Alpha
- TO: ### Requirement: Beta
- FROM: ### Requirement: Gamma
- TO: ### Requirement: Delta";
        let renames = ChangeParser::parse_renames(content);
        assert_eq!(renames.len(), 2);
        assert_eq!(renames[0].from, "Alpha");
        assert_eq!(renames[0].to, "Beta");
        assert_eq!(renames[1].from, "Gamma");
        assert_eq!(renames[1].to, "Delta");
    }

    #[test]
    fn parse_sections_from_content_creates_new_parser() {
        let content = "## A\nHello\n## B\nWorld";
        let sections = ChangeParser::parse_sections_from_content(content);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "A");
        assert_eq!(sections[1].title, "B");
    }

    #[test]
    fn discover_spec_files_returns_empty_for_missing_dir() {
        let result = discover_spec_files(Path::new("/nonexistent/path"));
        assert!(result.is_empty());
    }
}
