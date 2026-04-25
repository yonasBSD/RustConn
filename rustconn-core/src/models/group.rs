//! Connection group model for hierarchical organization.

use std::path::PathBuf;

use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::PasswordSource;
use super::SshAuthMethod;
use crate::sync::SyncMode;

/// A hierarchical group for organizing connections
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionGroup {
    /// Unique identifier for the group
    pub id: Uuid,
    /// Human-readable name for the group
    pub name: String,
    /// Parent group ID (None for root-level groups)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
    /// Whether the group is expanded in the UI
    #[serde(default)]
    pub expanded: bool,
    /// Timestamp when the group was created
    pub created_at: DateTime<Utc>,
    /// Sort order for manual ordering (lower values appear first)
    #[serde(default)]
    pub sort_order: i32,
    /// Username for inheritance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Domain for inheritance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Password source and config for inheritance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_source: Option<PasswordSource>,
    /// Optional description/notes for the group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Custom icon for the group (emoji/unicode character or GTK icon name)
    ///
    /// When `None`, the default folder icon is used.
    /// Examples: `"🇺🇦"`, `"🏢"`, `"starred-symbolic"`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// SSH authentication method for inheritance by child connections
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_auth_method: Option<SshAuthMethod>,
    /// SSH key path for inheritance (LOCAL-ONLY, not synced)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_key_path: Option<PathBuf>,
    /// SSH ProxyJump for inheritance
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_proxy_jump: Option<String>,
    /// ID of an SSH connection to use as jump host for all connections in this group.
    /// Takes precedence over `ssh_proxy_jump` text field. (LOCAL-ONLY, not synced)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_jump_host_id: Option<Uuid>,
    /// SSH agent socket override for inheritance (LOCAL-ONLY)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_agent_socket: Option<String>,
    /// Cloud Sync mode for this group (None, Master, or Import).
    #[serde(default)]
    pub sync_mode: SyncMode,
    /// Filename in the sync directory (e.g., `"production-servers.rcn"`).
    /// Fixed at first export and never changes even if the group is renamed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_file: Option<String>,
    /// Timestamp of the last successful sync operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_synced_at: Option<DateTime<Utc>>,
}

impl ConnectionGroup {
    /// Creates a new root-level group
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            parent_id: None,
            expanded: true,
            created_at: Utc::now(),
            sort_order: 0,
            username: None,
            domain: None,
            password_source: None,
            description: None,
            icon: None,
            ssh_auth_method: None,
            ssh_key_path: None,
            ssh_proxy_jump: None,
            ssh_jump_host_id: None,
            ssh_agent_socket: None,
            sync_mode: SyncMode::None,
            sync_file: None,
            last_synced_at: None,
        }
    }

    /// Creates a new group with a parent
    #[must_use]
    pub fn with_parent(name: String, parent_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            parent_id: Some(parent_id),
            expanded: true,
            created_at: Utc::now(),
            sort_order: 0,
            username: None,
            domain: None,
            password_source: None,
            description: None,
            icon: None,
            ssh_auth_method: None,
            ssh_key_path: None,
            ssh_proxy_jump: None,
            ssh_jump_host_id: None,
            ssh_agent_socket: None,
            sync_mode: SyncMode::None,
            sync_file: None,
            last_synced_at: None,
        }
    }

    /// Returns true if this is a root-level group
    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.parent_id.is_none()
    }
}
