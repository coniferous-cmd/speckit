//! Delta-spec validation for `speckit validate <change>`.
//!
//! A change's `specs/` directory is the only part of it that archive merges
//! into the main specs, so it is the part whose mistakes cost the most: a
//! requirement that parses under the wrong header is silently dropped, a
//! MODIFIED block that names nothing in the main spec fails only at archive
//! time, and a zero-delta change validates green while promising no behavior
//! at all. Before this module those cases were reported, if at all, as a
//! `No specs/ directory found` warning from the command layer, while the
//! parsers that could have caught them (`parse_delta_spec`,
//! `contains_shall_or_must`, `count_scenarios`) sat unused on the validate
//! path and archive was the first thing to refuse the change.
//!
//! Severity follows the OpenSpec validator so the two CLIs reach the same
//! verdict on the same change: structural breakage that archive would refuse
//! is ERROR, quality guidance is WARNING, and information the reader discarded
//! on its own is INFO.

use std::path::{Path, PathBuf};

use super::constants::{
    CHANGE_SKIP_SPECS_ACCEPTED, CHANGE_SKIP_SPECS_CONFLICT, CHANGE_SKIP_SPECS_INVALID_METADATA,
    MAX_DELTAS_PER_CHANGE, REQUIREMENT_NO_SCENARIOS, REQUIREMENT_NO_SHALL, change_too_many_deltas,
};
use super::types::{ValidationIssue, ValidationLevel};
use crate::change_metadata::{METADATA_FILENAME, read_skip_specs_marker};
use crate::parsers::requirement_text::{contains_shall_or_must, count_scenarios};
use crate::parsers::{
    RequirementBlock, extract_requirements_section, fold_requirement_name,
    normalize_requirement_name, parse_delta_spec,
};
use crate::utils::spec_discovery::{discover_spec_files, has_any_file_under};

/// The four headers the delta reader applies. A `## ... Requirements` header
/// outside this set reads as a delta operation to a human and is dropped by the
/// reader, which is why an orphan under it is an ERROR rather than a warning.
const CANONICAL_SECTIONS: [(&str, &str); 4] = [
    ("ADDED", "ADDED Requirements"),
    ("MODIFIED", "MODIFIED Requirements"),
    ("REMOVED", "REMOVED Requirements"),
    ("RENAMED", "RENAMED Requirements"),
];

/// Inputs for one delta validation pass.
#[derive(Debug, Clone, Default)]
pub struct DeltaValidationOptions {
    /// `<root>/speckit/specs`, when a planning root is resolvable. Without it
    /// the MODIFIED-vs-main-spec checks are skipped: `validate` must not claim
    /// a MODIFIED block is wrong just because the main spec could not be read.
    pub main_specs_dir: Option<PathBuf>,
}

/// Validate every delta spec in a change directory.
///
/// `Ok` carries the findings; an error means the change directory could not be
/// read at all, which the command layer reports as a failure rather than as a
/// clean report.
pub fn validate_change_deltas(
    change_dir: &Path,
    options: &DeltaValidationOptions,
) -> anyhow::Result<Vec<ValidationIssue>> {
    let mut issues = Vec::new();
    let specs_dir = change_dir.join("specs");

    let files = if specs_dir.exists() {
        discover_spec_files(&specs_dir)?
    } else {
        Vec::new()
    };

    let mut total_deltas = 0usize;
    let mut saw_parseable_delta = false;

    for path in &files {
        let entry_path = display_entry(change_dir, path);
        let content = std::fs::read_to_string(path)?;
        let plan = parse_delta_spec(&content);

        let count =
            plan.added.len() + plan.modified.len() + plan.removed.len() + plan.renamed.len();
        total_deltas += count;
        saw_parseable_delta |= count > 0;

        // A non-canonical `###` header the reader stepped over is not a
        // requirement, and the author needs to learn that rather than watch it
        // pass validation and vanish at archive.
        for stray in &plan.skipped_headers {
            let nameless = stray.header.trim().eq_ignore_ascii_case("requirement")
                || stray.header.trim().eq_ignore_ascii_case("requirement:");
            issues.push(infol(
                &entry_path,
                if nameless {
                    format!(
                        "Header \"### {}\" in {} is missing a requirement name and is ignored by validation. Add a name, e.g. \"### Requirement: <name>\".",
                        stray.header, stray.section
                    )
                } else {
                    format!(
                        "Header \"### {}\" in {} is not a \"### Requirement:\" header and is ignored by validation. Use \"### Requirement: {}\" if it should be validated as a requirement.",
                        stray.header, stray.section, stray.header
                    )
                },
            ));
        }

        // A `## Something Requirements` header reads as a delta operation but is
        // not one of the four, so everything under it is dropped.
        for (line, title) in unrecognized_requirement_sections(&content) {
            issues.push(err(
                &entry_path,
                format!(
                    "Unrecognized delta section \"## {title}\" at line {line}. Only \"## ADDED Requirements\", \"## MODIFIED Requirements\", \"## REMOVED Requirements\" and \"## RENAMED Requirements\" are applied; requirements under any other header are ignored."
                ),
            ));
        }

        let mut added_names = Vec::new();
        let mut modified_names = Vec::new();
        let mut removed_names = Vec::new();

        for block in &plan.added {
            let key = normalize_requirement_name(&block.name);
            if added_names.contains(&key) {
                issues.push(err(
                    &entry_path,
                    format!("Duplicate requirement in ADDED: \"{}\"", block.name),
                ));
            } else {
                added_names.push(key);
            }
            issues.extend(validate_block("ADDED", block, &entry_path));
        }

        for block in &plan.modified {
            let key = normalize_requirement_name(&block.name);
            if modified_names.contains(&key) {
                issues.push(err(
                    &entry_path,
                    format!("Duplicate requirement in MODIFIED: \"{}\"", block.name),
                ));
            } else {
                modified_names.push(key.clone());
            }
            issues.extend(validate_block("MODIFIED", block, &entry_path));
            if let Some(dir) = &options.main_specs_dir {
                issues.extend(validate_modified_against_main_spec(block, dir, &entry_path));
            }
        }

        for name in &plan.removed {
            let key = normalize_requirement_name(name);
            if removed_names.contains(&key) {
                issues.push(err(
                    &entry_path,
                    format!("Duplicate requirement in REMOVED: \"{name}\""),
                ));
            } else {
                removed_names.push(key);
            }
        }

        let mut renamed_from = Vec::new();
        let mut renamed_to = Vec::new();
        for pair in &plan.renamed {
            let from_key = normalize_requirement_name(&pair.from);
            let to_key = normalize_requirement_name(&pair.to);
            if renamed_from.contains(&from_key) {
                issues.push(err(
                    &entry_path,
                    format!("Duplicate FROM in RENAMED: \"{}\"", pair.from),
                ));
            } else {
                renamed_from.push(from_key.clone());
            }
            if renamed_to.contains(&to_key) {
                issues.push(err(
                    &entry_path,
                    format!("Duplicate TO in RENAMED: \"{}\"", pair.to),
                ));
            } else {
                renamed_to.push(to_key.clone());
            }

            // A rename whose old name is also modified, or whose new name is
            // also added, has two contradictory instructions for archive.
            if modified_names
                .iter()
                .any(|m| fold_requirement_name(m) == fold_requirement_name(&from_key))
            {
                issues.push(err(
                    &entry_path,
                    format!(
                        "MODIFIED references old name from RENAMED. Use new header for \"{}\"",
                        pair.to
                    ),
                ));
            }
            if added_names
                .iter()
                .any(|a| fold_requirement_name(a) == fold_requirement_name(&to_key))
            {
                issues.push(err(
                    &entry_path,
                    format!("RENAMED TO collides with ADDED for \"{}\"", pair.to),
                ));
            }
            if let Some(matched) = removed_names
                .iter()
                .find(|r| fold_requirement_name(r) == fold_requirement_name(&from_key))
            {
                let spelling = if matched == &from_key {
                    String::new()
                } else {
                    format!(" (REMOVED spells it \"{matched}\")")
                };
                issues.push(err(
                    &entry_path,
                    format!(
                        "Requirement present in both RENAMED and REMOVED: \"{}\"{spelling}",
                        pair.from
                    ),
                ));
            }
        }

        // Cross-section conflicts inside one delta file. Archive would apply
        // whichever branch it reaches last, so this is never author intent.
        for name in &modified_names {
            if removed_names.iter().any(|r| r == name) {
                issues.push(err(
                    &entry_path,
                    format!("Requirement present in both MODIFIED and REMOVED: \"{name}\""),
                ));
            }
            if added_names.iter().any(|a| a == name) {
                issues.push(err(
                    &entry_path,
                    format!("Requirement present in both MODIFIED and ADDED: \"{name}\""),
                ));
            }
        }
        for name in &added_names {
            if removed_names.iter().any(|r| r == name) {
                issues.push(err(
                    &entry_path,
                    format!("Requirement present in both ADDED and REMOVED: \"{name}\""),
                ));
            }
        }

        let presence = &plan.section_presence;
        let declared: Vec<String> = CANONICAL_SECTIONS
            .iter()
            .filter(|(key, _)| presence_of(presence, key))
            .map(|(_, label)| format!("## {label}"))
            .collect();
        if count == 0 {
            if !declared.is_empty() {
                issues.push(err(
                    &entry_path,
                    format!(
                        "Delta sections {} were found, but no requirement entries parsed. Ensure each section includes at least one \"### Requirement:\" block (REMOVED may use bullet list syntax).",
                        declared.join(", ")
                    ),
                ));
            } else {
                issues.push(err(
                    &entry_path,
                    "No delta sections found. Add headers such as \"## ADDED Requirements\" or move non-delta notes outside specs/.",
                ));
            }
        }
    }

    // A Markdown file beside spec.md is not on the merge path, but the
    // artifact graph's `specs/**/*.md` glob counts it as a written delta, so the
    // change looks complete while archive ignores the file.
    for misplaced in
        crate::utils::spec_discovery::find_misplaced_delta_specs(change_dir).unwrap_or_default()
    {
        issues.push(err(
            &format!("specs/{}", misplaced.to_string_lossy().replace('\\', "/")),
            format!(
                "Delta spec found at specs/{}. Delta specs must be a spec.md inside a capability folder - this file is ignored when the change is applied or archived. Move its requirements into specs/{}/spec.md.",
                misplaced.to_string_lossy().replace('\\', "/"),
                misplaced.parent().map_or_else(String::new, |p| p.to_string_lossy().replace('\\', "/"))
            ),
        ));
    }

    // The marker decides whether zero deltas is a promise kept or a hole.
    let marker = match read_skip_specs_marker(change_dir) {
        Ok(declared) => declared,
        Err(error) => {
            issues.push(err(
                METADATA_FILENAME,
                format!("{CHANGE_SKIP_SPECS_INVALID_METADATA}. {error}"),
            ));
            false
        }
    };

    if marker && has_any_file_under(&specs_dir) {
        issues.push(err("file", CHANGE_SKIP_SPECS_CONFLICT.to_string()));
    }

    if total_deltas == 0 {
        if marker {
            issues.push(infol("file", CHANGE_SKIP_SPECS_ACCEPTED));
        } else if !saw_parseable_delta {
            issues.push(err("file", "Change must have at least one delta"));
        }
    }

    if total_deltas > MAX_DELTAS_PER_CHANGE {
        issues.push(warn("file", change_too_many_deltas()));
    }

    Ok(issues)
}

/// Requirement-level checks shared by ADDED and MODIFIED.
fn validate_block(
    section: &str,
    block: &RequirementBlock,
    entry_path: &str,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let label = format!("{section} \"{}\"", block.name);

    let body: Vec<String> = block.raw.lines().skip(1).map(str::to_string).collect();
    let text = crate::parsers::extract_requirement_body(&body);

    if text.trim().is_empty() {
        // A header that happens to contain SHALL/MUST is not a requirement that
        // was written; the body is what is missing.
        issues.push(err(
            entry_path,
            if contains_shall_or_must(&block.name) {
                format!("{}: {REQUIREMENT_NO_SHALL}", label)
            } else {
                format!("{} is missing requirement text", label)
            },
        ));
    } else if !contains_shall_or_must(&text) {
        issues.push(warn(
            entry_path,
            format!("{}: {REQUIREMENT_NO_SHALL}", label),
        ));
    }

    if !text.trim().is_empty()
        && text.chars().count() > super::constants::MAX_REQUIREMENT_TEXT_LENGTH
    {
        issues.push(warn(
            entry_path,
            format!("{}: {}", label, super::constants::requirement_too_long()),
        ));
    }

    if count_scenarios(&body) < 1 {
        issues.push(err(
            entry_path,
            format!("{label} must include at least one scenario. {REQUIREMENT_NO_SCENARIOS}"),
        ));
    }

    issues.extend(scenario_step_issues(&body, &label, entry_path));
    issues
}

/// A scenario with no trigger or no outcome can never fail: nothing exercises it
/// and nothing asserts. Reported as WARNING because the block does count as a
/// scenario, so a normal run still passes; `--strict` turns it into a failure.
fn scenario_step_issues(body: &[String], label: &str, entry_path: &str) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    // Walk the body in document order, collecting each `#### Scenario:` block's
    // own lines. Anything before the first scenario belongs to the requirement
    // description and is not a scenario.
    let mut scenarios: Vec<(String, Vec<String>)> = Vec::new();
    for line in body {
        match scenario_header_title(line) {
            Some(name) => scenarios.push((name, Vec::new())),
            None => {
                if let Some((_, collected)) = scenarios.last_mut() {
                    collected.push(line.clone());
                }
            }
        }
    }

    for (name, collected) in scenarios {
        let joined = collected.join("\n").to_ascii_uppercase();
        let has_trigger = joined.contains("**WHEN**") || joined.contains("**GIVEN**");
        let has_outcome = joined.contains("**THEN**");
        if has_trigger || has_outcome {
            continue;
        }
        issues.push(warn(
            entry_path,
            format!(
                "{label}: scenario \"{name}\" is missing a WHEN/GIVEN step and a THEN step. A scenario needs a trigger and an outcome, e.g. \"- **WHEN** an invoice is voided\" and \"- **THEN** a credit note is issued\"."
            ),
        ));
    }

    issues
}

/// The name in a `#### Scenario: <name>` header, if this line is one.
///
/// Any level-4 child counts as a scenario for progress purposes, but only a
/// `Scenario:`-labeled block is checked for its steps: a schema that nests
/// other level-4 content under a requirement must not have that content
/// judged against scenario syntax it was never written in.
fn scenario_header_title(line: &str) -> Option<String> {
    let rest = line.trim().strip_prefix("#### ")?.trim();
    let (prefix, name) = rest.split_once(':')?;
    if !prefix.eq_ignore_ascii_case("Scenario") || name.trim().is_empty() {
        return None;
    }
    Some(name.trim().to_string())
}

/// MODIFIED replaces a block that must already exist. Catching the miss here
/// rather than at archive turns "archive failed" into "this line is wrong".
fn validate_modified_against_main_spec(
    block: &RequirementBlock,
    main_specs_dir: &Path,
    entry_path: &str,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    // The delta lives at <change>/specs/<capability-path>/spec.md, so the
    // capability path is whatever follows `specs/` in its entry path.
    let Some(capability) = capability_path_from_entry(entry_path) else {
        return issues;
    };
    let main_spec = main_specs_dir.join(&capability).join("spec.md");

    let Ok(content) = std::fs::read_to_string(&main_spec) else {
        issues.push(err(
            entry_path,
            format!(
                "MODIFIED \"{}\" targets capability \"{capability}\", but no main spec exists at {} to modify. Use ADDED for a new capability.",
                block.name,
                main_spec.display()
            ),
        ));
        return issues;
    };

    let main_blocks = extract_requirements_section(&content).body_blocks;
    let key = normalize_requirement_name(&block.name);
    let Some(existing) = main_blocks.iter().find(|b| {
        fold_requirement_name(&normalize_requirement_name(&b.name)) == fold_requirement_name(&key)
    }) else {
        issues.push(err(
            entry_path,
            format!(
                "MODIFIED \"{}\" does not match a requirement in {capability}. Copy the entire requirement block from the main spec and edit it; a name that only resembles it is not a match.",
                block.name
            ),
        ));
        return issues;
    };

    // Archive replaces the whole block, so a MODIFIED rewrite that carries
    // fewer scenarios than the main spec silently retires those behaviors.
    let scenario_count =
        |raw: &str| count_scenarios(&raw.lines().skip(1).map(str::to_string).collect::<Vec<_>>());
    if scenario_count(&block.raw) < scenario_count(&existing.raw) {
        issues.push(err(
            entry_path,
            format!(
                "MODIFIED \"{}\" drops scenarios that exist in the main spec. Archive replaces the whole block, so those behaviors would disappear with it.",
                block.name
            ),
        ));
    }

    issues
}

/// `specs/<capability-path>/spec.md` -> `<capability-path>`.
fn capability_path_from_entry(entry_path: &str) -> Option<String> {
    let rest = entry_path.strip_prefix("specs/")?;
    let capability = rest.strip_suffix("/spec.md")?;
    if capability.is_empty() {
        None
    } else {
        Some(capability.to_string())
    }
}

fn presence_of(presence: &crate::parsers::SectionPresence, key: &str) -> bool {
    match key {
        "ADDED" => presence.added,
        "MODIFIED" => presence.modified,
        "REMOVED" => presence.removed,
        _ => presence.renamed,
    }
}

/// Level-2 headers ending in `Requirements` that are not one of the four.
/// Fenced blocks are masked first: a delta that quotes the format it is
/// describing would otherwise be reported as a malformed section.
fn unrecognized_requirement_sections(content: &str) -> Vec<(usize, String)> {
    let lines: Vec<String> = content.lines().map(str::to_string).collect();
    let mask = crate::parsers::build_code_fence_mask(&lines);
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if mask.get(index).is_some_and(|m| *m) {
            continue;
        }
        let line = line.as_str();
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("## ") else {
            continue;
        };
        let title = rest.trim();
        if !title.to_ascii_lowercase().ends_with("requirements") {
            continue;
        }
        let canonical = CANONICAL_SECTIONS
            .iter()
            .any(|(_, label)| label.to_ascii_lowercase() == title.to_ascii_lowercase());
        if !canonical {
            found.push((index + 1, title.to_string()));
        }
    }
    found
}

fn display_entry(change_dir: &Path, path: &Path) -> String {
    path.strip_prefix(change_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn err(path: &str, message: impl Into<String>) -> ValidationIssue {
    issue(ValidationLevel::Error, path, message)
}

fn warn(path: &str, message: impl Into<String>) -> ValidationIssue {
    issue(ValidationLevel::Warning, path, message)
}

fn infol(path: &str, message: impl Into<String>) -> ValidationIssue {
    issue(ValidationLevel::Info, path, message)
}

fn issue(level: ValidationLevel, path: &str, message: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        level,
        path: path.to_string(),
        message: message.into(),
        line: None,
        column: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// A scratch planning root: `<tmp>/speckit/changes/<change>` for the delta
    /// and `<tmp>/speckit/specs/<capability>` for the main specs a MODIFIED
    /// block is compared against.
    struct Fixture {
        root: TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let root = TempDir::new().expect("temp root");
            fs::create_dir_all(root.path().join("speckit").join("changes")).unwrap();
            Fixture { root }
        }

        fn change_dir(&self, name: &str) -> PathBuf {
            let dir = self.root.path().join("speckit").join("changes").join(name);
            fs::create_dir_all(dir.join("specs")).unwrap();
            dir
        }

        fn write(&self, dir: &Path, relative: &str, content: &str) {
            let path = dir.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, content).unwrap();
        }

        fn main_spec(&self, capability: &str, content: &str) {
            self.write(
                &self.root.path().join("speckit").join("specs"),
                &format!("{capability}/spec.md"),
                content,
            );
        }

        fn validate(&self, change_dir: &Path) -> Vec<ValidationIssue> {
            validate_change_deltas(
                change_dir,
                &DeltaValidationOptions {
                    main_specs_dir: Some(self.root.path().join("speckit").join("specs")),
                },
            )
            .expect("delta validation runs")
        }
    }

    fn levels(issues: &[ValidationIssue], level: ValidationLevel) -> Vec<String> {
        issues
            .iter()
            .filter(|i| i.level == level)
            .map(|i| i.message.clone())
            .collect()
    }

    fn has(issues: &[ValidationIssue], level: ValidationLevel, needle: &str) -> bool {
        levels(issues, level)
            .iter()
            .any(|message| message.contains(needle))
    }

    fn good_added() -> &'static str {
        "## ADDED Requirements\n\n### Requirement: Export works\nThe system SHALL allow CSV export.\n\n#### Scenario: Export succeeds\n- **WHEN** a user requests export\n- **THEN** a CSV file is produced\n"
    }

    #[test]
    fn a_well_formed_delta_passes_clean() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("add-export");
        fixture.write(&change, "specs/export/spec.md", good_added());

        let issues = fixture.validate(&change);
        assert!(
            levels(&issues, ValidationLevel::Error).is_empty(),
            "unexpected errors: {:?}",
            levels(&issues, ValidationLevel::Error)
        );
    }

    #[test]
    fn zero_deltas_without_the_marker_is_an_error() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("refactor-internals");

        let issues = fixture.validate(&change);
        assert!(
            has(&issues, ValidationLevel::Error, "at least one delta"),
            "{issues:?}"
        );
    }

    #[test]
    fn zero_deltas_declared_by_skip_specs_is_accepted_not_rejected() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("docs-only");
        fixture.write(&change, METADATA_FILENAME, "skip_specs: true\n");

        let issues = fixture.validate(&change);
        assert!(
            !issues.iter().any(|i| i.level == ValidationLevel::Error),
            "skip_specs must not be contradicted: {issues:?}"
        );
        assert!(
            has(&issues, ValidationLevel::Info, CHANGE_SKIP_SPECS_ACCEPTED),
            "{issues:?}"
        );
    }

    #[test]
    fn skip_specs_with_delta_files_is_a_conflict() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("contradictory");
        fixture.write(&change, METADATA_FILENAME, "skip_specs: true\n");
        fixture.write(&change, "specs/export/spec.md", good_added());

        let issues = fixture.validate(&change);
        assert!(
            has(&issues, ValidationLevel::Error, CHANGE_SKIP_SPECS_CONFLICT),
            "{issues:?}"
        );
    }

    #[test]
    fn unreadable_metadata_fails_closed_instead_of_silently_dropping_the_marker() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("broken-metadata");
        fixture.write(&change, METADATA_FILENAME, "skip_specs: [not, a, bool]\n");

        let issues = fixture.validate(&change);
        assert!(
            has(
                &issues,
                ValidationLevel::Error,
                CHANGE_SKIP_SPECS_INVALID_METADATA
            ),
            "{issues:?}"
        );
    }

    #[test]
    fn a_delta_section_with_no_entries_is_rejected() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("empty-section");
        fixture.write(&change, "specs/export/spec.md", "## ADDED Requirements\n");

        let issues = fixture.validate(&change);
        assert!(
            has(
                &issues,
                ValidationLevel::Error,
                "no requirement entries parsed"
            ),
            "{issues:?}"
        );
    }

    #[test]
    fn a_spec_file_with_no_delta_sections_at_all_is_rejected() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("prose-only");
        fixture.write(
            &change,
            "specs/export/spec.md",
            "# Notes\n\nThis is background, not a delta.\n",
        );

        let issues = fixture.validate(&change);
        assert!(
            has(&issues, ValidationLevel::Error, "No delta sections found"),
            "{issues:?}"
        );
    }

    #[test]
    fn duplicate_requirement_names_are_rejected_per_section() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("duplicates");
        fixture.write(
            &change,
            "specs/export/spec.md",
            "## ADDED Requirements\n\n### Requirement: Export works\nThe system SHALL export.\n\n#### Scenario: One\n- **WHEN** requested\n- **THEN** exported\n\n### Requirement: Export works\nThe system SHALL also export again.\n\n#### Scenario: Two\n- **WHEN** requested twice\n- **THEN** exported twice\n",
        );

        let issues = fixture.validate(&change);
        assert!(
            has(
                &issues,
                ValidationLevel::Error,
                "Duplicate requirement in ADDED"
            ),
            "{issues:?}"
        );
    }

    #[test]
    fn a_requirement_both_added_and_modified_is_a_conflict() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("self-conflict");
        fixture.write(
            &change,
            "specs/export/spec.md",
            "## ADDED Requirements\n\n### Requirement: Export works\nThe system SHALL export.\n\n#### Scenario: One\n- **WHEN** requested\n- **THEN** exported\n\n## MODIFIED Requirements\n\n### Requirement: Export works\nThe system SHALL export differently.\n\n#### Scenario: Two\n- **WHEN** requested\n- **THEN** exported differently\n",
        );

        let issues = fixture.validate(&change);
        assert!(
            has(
                &issues,
                ValidationLevel::Error,
                "Requirement present in both MODIFIED and ADDED"
            ),
            "{issues:?}"
        );
    }

    #[test]
    fn a_renamed_target_colliding_with_an_added_name_is_rejected() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("rename-collision");
        fixture.write(
            &change,
            "specs/export/spec.md",
            "## RENAMED Requirements\n- FROM: `### Requirement: Old name`\n- TO: `### Requirement: Export works`\n\n## ADDED Requirements\n\n### Requirement: Export works\nThe system SHALL export.\n\n#### Scenario: One\n- **WHEN** requested\n- **THEN** exported\n",
        );

        let issues = fixture.validate(&change);
        assert!(
            has(
                &issues,
                ValidationLevel::Error,
                "RENAMED TO collides with ADDED"
            ),
            "{issues:?}"
        );
    }

    #[test]
    fn a_requirement_without_normative_keyword_is_a_warning_not_an_error() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("no-shall");
        fixture.write(
            &change,
            "specs/export/spec.md",
            "## ADDED Requirements\n\n### Requirement: Export works\nThe system should export when asked.\n\n#### Scenario: One\n- **WHEN** requested\n- **THEN** exported\n",
        );

        let issues = fixture.validate(&change);
        assert!(
            has(&issues, ValidationLevel::Warning, REQUIREMENT_NO_SHALL),
            "{issues:?}"
        );
        assert!(levels(&issues, ValidationLevel::Error).is_empty());
    }

    #[test]
    fn a_requirement_with_no_scenario_is_rejected() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("no-scenario");
        fixture.write(
            &change,
            "specs/export/spec.md",
            "## ADDED Requirements\n\n### Requirement: Export works\nThe system SHALL export.\n",
        );

        let issues = fixture.validate(&change);
        assert!(
            has(&issues, ValidationLevel::Error, "at least one scenario"),
            "{issues:?}"
        );
    }

    #[test]
    fn a_scenario_without_trigger_or_outcome_is_warned() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("hollow-scenario");
        fixture.write(
            &change,
            "specs/export/spec.md",
            "## ADDED Requirements\n\n### Requirement: Export works\nThe system SHALL export.\n\n#### Scenario: Export succeeds\nIt just works.\n",
        );

        let issues = fixture.validate(&change);
        assert!(
            has(
                &issues,
                ValidationLevel::Warning,
                "missing a WHEN/GIVEN step"
            ),
            "{issues:?}"
        );
    }

    #[test]
    fn an_overlong_requirement_description_is_warned() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("verbose-requirement");
        let filler = "The system SHALL export data on request. ".repeat(20);
        fixture.write(
            &change,
            "specs/export/spec.md",
            &format!(
                "## ADDED Requirements\n\n### Requirement: Export works\n{filler}\n\n#### Scenario: One\n- **WHEN** requested\n- **THEN** exported\n"
            ),
        );

        let issues = fixture.validate(&change);
        assert!(
            has(
                &issues,
                ValidationLevel::Warning,
                "Requirement text is very long"
            ),
            "{issues:?}"
        );
    }

    #[test]
    fn a_non_delta_requirements_header_is_rejected_with_its_line() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("wrong-header");
        fixture.write(
            &change,
            "specs/export/spec.md",
            "## EXTRA Requirements\n\n### Requirement: Export works\nThe system SHALL export.\n\n#### Scenario: One\n- **WHEN** requested\n- **THEN** exported\n",
        );

        let issues = fixture.validate(&change);
        assert!(
            has(
                &issues,
                ValidationLevel::Error,
                "Unrecognized delta section"
            ),
            "{issues:?}"
        );
    }

    #[test]
    fn a_header_quoted_inside_a_code_fence_is_not_mistaken_for_a_section() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("fenced-example");
        fixture.write(
            &change,
            "specs/export/spec.md",
            &format!(
                "{}\n\nExample of a section you should not write:\n\n```\n## EXTRA Requirements\n```\n",
                good_added()
            ),
        );

        let issues = fixture.validate(&change);
        assert!(
            !has(
                &issues,
                ValidationLevel::Error,
                "Unrecognized delta section"
            ),
            "fenced example was read as real structure: {issues:?}"
        );
    }

    #[test]
    fn a_stray_level_four_header_is_reported_as_ignored() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("stray-header");
        fixture.write(
            &change,
            "specs/export/spec.md",
            &format!(
                "{}\n### Documentation Requirements\nSome prose the reader skips.\n",
                good_added()
            ),
        );

        let issues = fixture.validate(&change);
        assert!(
            has(
                &issues,
                ValidationLevel::Info,
                "is not a \"### Requirement:\" header"
            ),
            "{issues:?}"
        );
    }

    #[test]
    fn modified_naming_nothing_in_the_main_spec_is_rejected_before_archive() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("phantom-modify");
        fixture.main_spec(
            "export",
            "# Spec\n\n## Purpose\n\nExports data.\n\n## Requirements\n\n### Requirement: Export works\nThe system SHALL export.\n\n#### Scenario: One\n- **WHEN** requested\n- **THEN** exported\n",
        );
        fixture.write(
            &change,
            "specs/export/spec.md",
            "## MODIFIED Requirements\n\n### Requirement: Export is fast\nThe system SHALL export quickly.\n\n#### Scenario: One\n- **WHEN** requested\n- **THEN** exported fast\n",
        );

        let issues = fixture.validate(&change);
        assert!(
            has(
                &issues,
                ValidationLevel::Error,
                "does not match a requirement in export"
            ),
            "{issues:?}"
        );
    }

    #[test]
    fn modified_that_drops_a_scenario_the_main_spec_has_is_rejected() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("scenario-loss");
        fixture.main_spec(
            "export",
            "# Spec\n\n## Purpose\n\nExports data.\n\n## Requirements\n\n### Requirement: Export works\nThe system SHALL export.\n\n#### Scenario: One\n- **WHEN** requested\n- **THEN** exported\n\n#### Scenario: Two\n- **WHEN** requested again\n- **THEN** exported again\n",
        );
        fixture.write(
            &change,
            "specs/export/spec.md",
            "## MODIFIED Requirements\n\n### Requirement: Export works\nThe system SHALL export in CSV.\n\n#### Scenario: One\n- **WHEN** requested\n- **THEN** a CSV is exported\n",
        );

        let issues = fixture.validate(&change);
        assert!(
            has(
                &issues,
                ValidationLevel::Error,
                "drops scenarios that exist in the main spec"
            ),
            "{issues:?}"
        );
    }

    #[test]
    fn main_spec_checks_are_skipped_when_no_planning_root_is_known() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("no-root");
        fixture.write(&change, "specs/export/spec.md", "## ADDED Requirements\n\n### Requirement: Export works\nThe system SHALL export.\n\n#### Scenario: One\n- **WHEN** requested\n- **THEN** exported\n");

        let issues = validate_change_deltas(&change, &DeltaValidationOptions::default())
            .expect("delta validation runs");
        assert!(
            levels(&issues, ValidationLevel::Error).is_empty(),
            "{issues:?}"
        );
    }

    #[test]
    fn a_delta_beside_spec_md_is_named_rather_than_dropped() {
        let fixture = Fixture::new();
        let change = fixture.change_dir("stray-file");
        fixture.write(&change, "specs/export.md", good_added());

        let issues = fixture.validate(&change);
        assert!(
            has(
                &issues,
                ValidationLevel::Error,
                "must be a spec.md inside a capability folder"
            ),
            "{issues:?}"
        );
    }
}
