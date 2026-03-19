//! Highlight rule model for regex-based text highlighting in terminals.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ConfigError;

/// A regex-based rule for highlighting text in terminal output.
///
/// Each rule defines a pattern to match and optional foreground/background
/// colors to apply to matching text regions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighlightRule {
    /// Unique identifier for the rule
    pub id: Uuid,
    /// Human-readable name for the rule (e.g. "Error lines")
    pub name: String,
    /// Regex pattern to match against terminal text
    pub pattern: String,
    /// Optional foreground (text) color in CSS hex format (#RRGGBB)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground_color: Option<String>,
    /// Optional background color in CSS hex format (#RRGGBB)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    /// Whether this rule is active
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

/// Default value for `enabled` field during deserialization.
fn default_enabled() -> bool {
    true
}

impl HighlightRule {
    /// Creates a new highlight rule with the given name and pattern.
    #[must_use]
    pub fn new(name: String, pattern: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            pattern,
            foreground_color: None,
            background_color: None,
            enabled: true,
        }
    }

    /// Validates that the `pattern` field is a valid regex.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Validation`] if the pattern cannot be compiled.
    pub fn validate_pattern(&self) -> Result<(), ConfigError> {
        regex::Regex::new(&self.pattern).map_err(|e| ConfigError::Validation {
            field: "pattern".to_string(),
            reason: format!("Invalid regex pattern '{}': {e}", self.pattern),
        })?;
        Ok(())
    }
}
