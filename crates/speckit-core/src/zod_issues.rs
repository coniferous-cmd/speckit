use colored::Colorize;

/// Format Zod-style validation issues into human-readable messages.
/// This is the Rust equivalent of the TypeScript formatZodIssues function.
pub fn format_issues(issues: &[ValidationIssue]) -> Vec<String> {
    issues
        .iter()
        .map(|issue| {
            let level_str = match issue.level {
                ValidationLevel::Error => "ERROR".red().bold(),
                ValidationLevel::Warning => "WARNING".yellow().bold(),
                ValidationLevel::Info => "INFO".cyan().bold(),
            };
            format!("{} [{}]: {}", level_str, issue.path, issue.message)
        })
        .collect()
}

/// Validation level enum
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationLevel {
    Error,
    Warning,
    Info,
}

/// Validation issue struct
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub level: ValidationLevel,
    pub path: String,
    pub message: String,
}

impl ValidationIssue {
    pub fn error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            level: ValidationLevel::Error,
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn warning(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            level: ValidationLevel::Warning,
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn info(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            level: ValidationLevel::Info,
            path: path.into(),
            message: message.into(),
        }
    }
}
