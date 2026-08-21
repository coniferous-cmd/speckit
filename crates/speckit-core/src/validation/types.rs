use serde::{Deserialize, Serialize};

/// Severity level of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ValidationLevel {
    Error,
    Warning,
    Info,
}

/// A single validation finding attached to a path inside the document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub level: ValidationLevel,
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
}

/// Aggregated result of a validation pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
    pub summary: ValidationSummary,
}

/// Counts of issues by severity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
}

impl ValidationReport {
    /// Create a report from a list of issues.  When `strict_mode` is `true`,
    /// any warning also makes the report invalid; otherwise only errors do.
    pub fn from_issues(issues: Vec<ValidationIssue>, strict_mode: bool) -> Self {
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

        let valid = if strict_mode {
            errors == 0 && warnings == 0
        } else {
            errors == 0
        };

        Self {
            valid,
            issues,
            summary: ValidationSummary {
                errors,
                warnings,
                info,
            },
        }
    }

    /// Convenience: returns `self.valid`.
    pub fn is_valid(&self) -> bool {
        self.valid
    }
}
