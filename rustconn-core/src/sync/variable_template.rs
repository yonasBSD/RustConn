//! Variable template model for Cloud Sync exports.
//!
//! Used in sync exports to describe variables referenced by connections.
//! Each [`VariableTemplate`] captures the variable's name, description,
//! whether it holds a secret, and an optional default value (non-secret only).
//! Actual variable values are never synced — only the template metadata
//! travels between devices so each user can configure their own secrets locally.

use serde::{Deserialize, Serialize};

/// Describes a variable referenced by synced connections.
///
/// Included in [`GroupSyncExport`](super::group_export::GroupSyncExport) and
/// [`FullSyncExport`](super::full_export::FullSyncExport) so that importing
/// devices know which variables to prompt the user for on first connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableTemplate {
    /// Variable name used as the lookup key (e.g. `"web_deploy_key"`).
    pub name: String,

    /// Human-readable description shown in the setup dialog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether this variable holds a secret value (password, key passphrase, etc.).
    /// Secret variables never have a `default_value`.
    #[serde(default)]
    pub is_secret: bool,

    /// Default value for non-secret variables.
    /// Always `None` when `is_secret` is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialization_round_trip() {
        let template = VariableTemplate {
            name: "web_deploy_key".to_owned(),
            description: Some("SSH key passphrase for web deployment".to_owned()),
            is_secret: true,
            default_value: None,
        };
        let json = serde_json::to_string(&template).unwrap();
        let deserialized: VariableTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(template, deserialized);
    }

    #[test]
    fn non_secret_with_default_value() {
        let template = VariableTemplate {
            name: "api_endpoint".to_owned(),
            description: Some("API base URL".to_owned()),
            is_secret: false,
            default_value: Some("https://api.example.com".to_owned()),
        };
        let json = serde_json::to_string(&template).unwrap();
        let deserialized: VariableTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(template, deserialized);
    }

    #[test]
    fn optional_fields_skipped_when_none() {
        let template = VariableTemplate {
            name: "simple_var".to_owned(),
            description: None,
            is_secret: false,
            default_value: None,
        };
        let json = serde_json::to_string(&template).unwrap();
        assert!(!json.contains("description"));
        assert!(!json.contains("default_value"));
    }

    #[test]
    fn deserialize_with_defaults() {
        let json = r#"{"name":"minimal"}"#;
        let template: VariableTemplate = serde_json::from_str(json).unwrap();
        assert_eq!(template.name, "minimal");
        assert!(template.description.is_none());
        assert!(!template.is_secret);
        assert!(template.default_value.is_none());
    }

    #[test]
    fn is_secret_defaults_to_false() {
        let json = r#"{"name":"test","description":"desc"}"#;
        let template: VariableTemplate = serde_json::from_str(json).unwrap();
        assert!(!template.is_secret);
    }
}
