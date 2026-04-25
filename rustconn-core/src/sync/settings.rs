//! Sync settings and mode configuration for Cloud Sync.
//!
//! Provides [`SyncSettings`] for global sync configuration and [`SyncMode`]
//! for per-group sync role (None, Master, Import).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Global synchronization settings.
///
/// Controls the sync directory, device identity, and timing parameters
/// for both Group Sync and Simple Sync modes.
///
/// # Validation Rules
///
/// - `sync_dir`: Must exist and be writable when set
/// - `device_name`: Non-empty, defaults to hostname
/// - `export_debounce_secs`: Range 1..=60, default 5
/// - `tombstone_retention_days`: Range 1..=365, default 30
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncSettings {
    /// Path to the shared cloud sync directory (Google Drive, Syncthing, etc.).
    /// `None` means sync is not configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_dir: Option<PathBuf>,

    /// Unique identifier for this device, used to prevent self-sync.
    pub device_id: Uuid,

    /// Human-readable device name shown in sync metadata.
    pub device_name: String,

    /// Whether to auto-import changes from Import groups on application start.
    #[serde(default = "default_auto_import")]
    pub auto_import_on_start: bool,

    /// Debounce interval in seconds for Master group exports.
    /// Range: 1..=60, default: 5.
    #[serde(default = "default_export_debounce_secs")]
    pub export_debounce_secs: u32,

    /// Number of days to retain tombstones in Simple Sync before cleanup.
    /// Range: 1..=365, default: 30.
    #[serde(default = "default_tombstone_retention_days")]
    pub tombstone_retention_days: u32,

    /// Whether Simple Sync (bidirectional full sync) is enabled.
    #[serde(default)]
    pub simple_sync_enabled: bool,
}

fn default_auto_import() -> bool {
    true
}

fn default_export_debounce_secs() -> u32 {
    5
}

fn default_tombstone_retention_days() -> u32 {
    30
}

/// Returns the system hostname, falling back to `"unknown"`.
fn get_device_name() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            sync_dir: None,
            device_id: Uuid::new_v4(),
            device_name: get_device_name(),
            auto_import_on_start: true,
            export_debounce_secs: 5,
            tombstone_retention_days: 30,
            simple_sync_enabled: false,
        }
    }
}

/// Sync role for a [`ConnectionGroup`](crate::models::ConnectionGroup).
///
/// - `None` — group is not synced
/// - `Master` — group exports changes to a `.rcn` file
/// - `Import` — group imports changes from a `.rcn` file (read-only)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SyncMode {
    /// Group is not participating in sync.
    #[default]
    None,
    /// Group is the authoritative source; exports to sync file.
    Master,
    /// Group imports from sync file; synced fields are read-only.
    Import,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_settings_default_values() {
        let settings = SyncSettings::default();
        assert!(settings.sync_dir.is_none());
        assert!(!settings.device_name.is_empty());
        assert!(settings.auto_import_on_start);
        assert_eq!(settings.export_debounce_secs, 5);
        assert_eq!(settings.tombstone_retention_days, 30);
    }

    #[test]
    fn sync_mode_default_is_none() {
        assert_eq!(SyncMode::default(), SyncMode::None);
    }

    #[test]
    fn sync_settings_serialization_round_trip() {
        let settings = SyncSettings {
            sync_dir: Some(PathBuf::from("/tmp/sync")),
            device_id: Uuid::new_v4(),
            device_name: "test-device".to_owned(),
            auto_import_on_start: false,
            export_debounce_secs: 10,
            tombstone_retention_days: 60,
            simple_sync_enabled: false,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: SyncSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(settings, deserialized);
    }

    #[test]
    fn sync_mode_serialization_round_trip() {
        for mode in [SyncMode::None, SyncMode::Master, SyncMode::Import] {
            let json = serde_json::to_string(&mode).unwrap();
            let deserialized: SyncMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, deserialized);
        }
    }

    #[test]
    fn sync_settings_deserialize_with_defaults() {
        // Minimal JSON — missing optional/defaulted fields
        let json = r#"{"device_id":"00000000-0000-0000-0000-000000000001","device_name":"laptop"}"#;
        let settings: SyncSettings = serde_json::from_str(json).unwrap();
        assert!(settings.sync_dir.is_none());
        assert!(settings.auto_import_on_start);
        assert_eq!(settings.export_debounce_secs, 5);
        assert_eq!(settings.tombstone_retention_days, 30);
    }
}
