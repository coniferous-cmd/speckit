use anyhow::Result;
use std::fs;
use std::path::Path;

use super::constants::*;
use super::types::{ValidationIssue, ValidationLevel, ValidationReport, ValidationSummary};
use crate::parsers::MarkdownParser;
use crate::schemas::{Change, Spec};

/// Validator for Speckit specs and changes.
pub struct Validator {
    strict_mode: bool,
}

impl Validator {
    /// Create a new validator.
    pub fn new(strict_mode: bool) -> Self {
        Self { strict_mode }
    }

    /// Create a validation issue with default line/column.
    fn issue(
        level: ValidationLevel,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> ValidationIssue {
        ValidationIssue {
            level,
            path: path.into(),
            message: message.into(),
            line: None,
            column: None,
        }
    }

    /// Create a report from issues.
    fn create_report(&self, issues: Vec<ValidationIssue>) -> ValidationReport {
        let errors = issues
            .iter()
            .filter(|i| i.level == ValidationLevel::Error)
            .count();
        let warnings = issues
            .iter()
            .filter(|i| i.level == ValidationLevel::Warning)
            .count();
        let info = issues
            .iter()
            .filter(|i| i.level == ValidationLevel::Info)
            .count();

        let valid = if self.strict_mode {
            errors == 0 && warnings == 0
        } else {
            errors == 0
        };

        ValidationReport {
            valid,
            issues,
            summary: ValidationSummary {
                errors,
                warnings,
                info,
            },
        }
    }

    /// Validate a spec file.
    pub fn validate_spec(&self, file_path: &Path) -> Result<ValidationReport> {
        let mut issues = Vec::new();
        let spec_name = Self::extract_name_from_path(file_path);

        match fs::read_to_string(file_path) {
            Ok(content) => match self.validate_spec_content(&spec_name, &content) {
                Ok(report) => issues.extend(report.issues),
                Err(e) => issues.push(Self::issue(ValidationLevel::Error, "file", e.to_string())),
            },
            Err(e) => {
                issues.push(Self::issue(
                    ValidationLevel::Error,
                    "file",
                    format!("Cannot read file: {}", e),
                ));
            }
        }

        Ok(self.create_report(issues))
    }

    /// Validate spec content from a string.
    pub fn validate_spec_content(
        &self,
        spec_name: &str,
        content: &str,
    ) -> Result<ValidationReport> {
        let mut issues = Vec::new();

        let parser = MarkdownParser::new(content);

        match parser.parse_spec(spec_name) {
            Ok(spec) => {
                // Validate using schema rules
                issues.extend(self.apply_spec_rules(&spec, content));
            }
            Err(e) => {
                issues.push(Self::issue(ValidationLevel::Error, "file", e.to_string()));
            }
        }

        Ok(self.create_report(issues))
    }

    /// Apply spec validation rules.
    fn apply_spec_rules(&self, spec: &Spec, content: &str) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        // Check spec name
        if spec.name.is_empty() {
            issues.push(Self::issue(ValidationLevel::Error, "name", SPEC_NAME_EMPTY));
        }

        // Check overview/purpose
        if spec.overview.is_empty() {
            issues.push(Self::issue(
                ValidationLevel::Error,
                "overview",
                SPEC_PURPOSE_EMPTY,
            ));
        } else if self.strict_mode && spec.overview.len() < MIN_PURPOSE_LENGTH {
            issues.push(Self::issue(
                ValidationLevel::Warning,
                "overview",
                purpose_too_brief(),
            ));
        }

        // Check requirements
        if spec.requirements.is_empty() {
            issues.push(Self::issue(
                ValidationLevel::Error,
                "requirements",
                SPEC_NO_REQUIREMENTS,
            ));
        }

        // Validate each requirement
        for (i, req) in spec.requirements.iter().enumerate() {
            if req.name.is_empty() {
                issues.push(Self::issue(
                    ValidationLevel::Error,
                    format!("requirements[{}].name", i),
                    REQUIREMENT_EMPTY,
                ));
            }

            if req.scenarios.is_empty() {
                issues.push(Self::issue(
                    ValidationLevel::Error,
                    format!("requirements[{}].scenarios", i),
                    REQUIREMENT_NO_SCENARIOS,
                ));
            }

            for (j, scenario) in req.scenarios.iter().enumerate() {
                if scenario.name.is_empty() {
                    issues.push(Self::issue(
                        ValidationLevel::Error,
                        format!("requirements[{}].scenarios[{}].name", i, j),
                        SCENARIO_EMPTY,
                    ));
                }
            }
        }

        issues
    }

    /// Validate a change file.
    pub fn validate_change(&self, file_path: &Path) -> Result<ValidationReport> {
        let mut issues = Vec::new();
        let change_name = Self::extract_name_from_path(file_path);

        match fs::read_to_string(file_path) {
            Ok(content) => {
                let parser = MarkdownParser::new(&content);

                match parser.parse_change(&change_name) {
                    Ok(change) => {
                        issues.extend(self.apply_change_rules(&change, &content));
                    }
                    Err(e) => {
                        issues.push(Self::issue(ValidationLevel::Error, "file", e.to_string()));
                    }
                }
            }
            Err(e) => {
                issues.push(Self::issue(
                    ValidationLevel::Error,
                    "file",
                    format!("Cannot read file: {}", e),
                ));
            }
        }

        Ok(self.create_report(issues))
    }

    /// Apply change validation rules.
    fn apply_change_rules(&self, change: &Change, content: &str) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        // Check change name
        if change.name.is_empty() {
            issues.push(Self::issue(
                ValidationLevel::Error,
                "name",
                CHANGE_NAME_EMPTY,
            ));
        }

        // Check why section
        if change.why.is_empty() {
            issues.push(Self::issue(
                ValidationLevel::Error,
                "why",
                "Why section cannot be empty",
            ));
        } else if change.why.len() < MIN_WHY_SECTION_LENGTH {
            issues.push(Self::issue(
                ValidationLevel::Error,
                "why",
                change_why_too_short(),
            ));
        } else if change.why.len() > MAX_WHY_SECTION_LENGTH {
            issues.push(Self::issue(
                ValidationLevel::Warning,
                "why",
                change_why_too_long(),
            ));
        }

        // Check what changes section
        if change.what_changes.is_empty() {
            issues.push(Self::issue(
                ValidationLevel::Error,
                "whatChanges",
                CHANGE_WHAT_EMPTY,
            ));
        }

        issues
    }

    /// Extract name from a file path.
    fn extract_name_from_path(file_path: &Path) -> String {
        let components: Vec<&str> = file_path
            .components()
            .map(|c| c.as_os_str().to_str().unwrap_or(""))
            .collect();

        for i in 0..components.len() {
            if (components[i] == "specs" || components[i] == "changes") && i + 1 < components.len()
            {
                return components[i + 1].to_string();
            }
        }

        let file_name = components.last().unwrap_or(&"");
        match file_name.rfind('.') {
            Some(idx) => file_name[..idx].to_string(),
            None => file_name.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_validate_valid_spec() {
        let content = r#"## Purpose
This is a valid purpose section with enough content to pass validation checks.

## Requirements
### Requirement: Test Requirement
The system SHALL work correctly.

#### Scenario: Basic functionality
- WHEN the user performs an action
- THEN the system responds correctly
"#;

        let validator = Validator::new(false);
        let report = validator.validate_spec_content("test", content).unwrap();
        assert!(report.valid);
    }

    #[test]
    fn test_validate_missing_purpose() {
        let content = r#"## Requirements
### Requirement: Test
Text
"#;

        let validator = Validator::new(false);
        let report = validator.validate_spec_content("test", content).unwrap();
        assert!(!report.valid);
    }

    #[test]
    fn test_extract_name_from_path() {
        let path = PathBuf::from("speckit/specs/user-auth/spec.md");
        assert_eq!(Validator::extract_name_from_path(&path), "user-auth");
    }
}
