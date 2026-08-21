use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StoreDiagnosticSeverity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for StoreDiagnosticSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Info => write!(f, "info"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreDiagnostic {
    pub severity: StoreDiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

/// Creates a [`StoreDiagnostic`] with the given severity, code, and message.
pub fn make_store_diagnostic(
    severity: StoreDiagnosticSeverity,
    code: impl Into<String>,
    message: impl Into<String>,
    target: Option<String>,
    fix: Option<String>,
) -> StoreDiagnostic {
    StoreDiagnostic {
        severity,
        code: code.into(),
        message: message.into(),
        target,
        fix,
    }
}

/// Core store error type, carrying a human-readable diagnostic.
#[derive(Debug)]
pub struct StoreError {
    pub diagnostic: StoreDiagnostic,
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.diagnostic.message)
    }
}

impl std::error::Error for StoreError {}

impl StoreError {
    pub fn new(
        message: impl Into<String>,
        code: impl Into<String>,
        options: StoreErrorOptions,
    ) -> Self {
        let message = message.into();
        Self {
            diagnostic: StoreDiagnostic {
                severity: StoreDiagnosticSeverity::Error,
                code: code.into(),
                message: message.clone(),
                target: options.target,
                fix: options.fix,
            },
        }
    }

    pub fn code(&self) -> &str {
        &self.diagnostic.code
    }

    pub fn target(&self) -> Option<&str> {
        self.diagnostic.target.as_deref()
    }

    pub fn fix(&self) -> Option<&str> {
        self.diagnostic.fix.as_deref()
    }
}

#[derive(Debug, Clone, Default)]
pub struct StoreErrorOptions {
    pub target: Option<String>,
    pub fix: Option<String>,
}

/// A root-selection error, parallel to [`StoreError`] but scoped to the
/// root resolution module.
#[derive(Debug)]
pub struct RootSelectionError {
    pub diagnostic: StoreDiagnostic,
}

impl std::fmt::Display for RootSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.diagnostic.message)
    }
}

impl std::error::Error for RootSelectionError {}

impl RootSelectionError {
    pub fn new(
        message: impl Into<String>,
        code: impl Into<String>,
        options: StoreErrorOptions,
    ) -> Self {
        let message = message.into();
        Self {
            diagnostic: StoreDiagnostic {
                severity: StoreDiagnosticSeverity::Error,
                code: code.into(),
                message: message.clone(),
                target: options.target,
                fix: options.fix,
            },
        }
    }

    pub fn code(&self) -> &str {
        &self.diagnostic.code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_error_display_uses_message() {
        let err = StoreError::new(
            "Something went wrong",
            "test_code",
            StoreErrorOptions {
                target: Some("store.id".into()),
                fix: Some("Fix it.".into()),
            },
        );
        assert_eq!(err.to_string(), "Something went wrong");
        assert_eq!(err.code(), "test_code");
        assert_eq!(err.target(), Some("store.id"));
        assert_eq!(err.fix(), Some("Fix it."));
    }

    #[test]
    fn diagnostic_serializes_with_optional_fields() {
        let diag = make_store_diagnostic(
            StoreDiagnosticSeverity::Warning,
            "test_warn",
            "warning message",
            None,
            Some("fix hint".into()),
        );
        let json = serde_json::to_string(&diag).unwrap();
        assert!(json.contains("\"severity\":\"warning\""));
        assert!(!json.contains("\"target\""));
        assert!(json.contains("\"fix\":\"fix hint\""));
    }
}
