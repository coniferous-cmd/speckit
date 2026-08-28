use serde::{Deserialize, Serialize};
use validator::Validate;

/// Custom validator: ensure every scenario in a slice has non-empty raw text.
fn validate_scenarios(scenarios: &[Scenario]) -> Result<(), validator::ValidationError> {
    for s in scenarios {
        if s.raw_text.is_empty() {
            return Err(validator::ValidationError::new("non_empty_scenario"));
        }
    }
    if scenarios.is_empty() {
        return Err(validator::ValidationError::new("non_empty"));
    }
    Ok(())
}

/// A single scenario attached to a requirement.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    /// The scenario name/title.
    #[serde(default)]
    pub name: String,

    /// The full raw text of the scenario.
    #[validate(length(min = 1, message = "Scenario text cannot be empty"))]
    pub raw_text: String,
}

/// A requirement with its associated scenarios.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    /// The requirement name/title.
    #[serde(default)]
    pub name: String,

    /// The requirement text (excluding scenarios).
    #[validate(length(min = 1, message = "Requirement text cannot be empty"))]
    pub text: String,

    /// Associated scenarios.
    #[validate(custom(function = "validate_scenarios"))]
    pub scenarios: Vec<Scenario>,
}
