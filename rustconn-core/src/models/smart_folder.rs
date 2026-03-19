//! Smart folder model for dynamic connection grouping based on filter criteria.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::protocol::ProtocolType;

/// A saved filter that dynamically groups connections.
///
/// Smart folders evaluate their filter criteria against all connections
/// using AND logic — a connection must match ALL active filters to appear
/// in the folder's results. Empty filter criteria yield an empty result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartFolder {
    /// Unique identifier for the smart folder
    pub id: Uuid,
    /// Human-readable name (e.g. "Prod SSH Servers")
    pub name: String,
    /// Filter by protocol type (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_protocol: Option<ProtocolType>,
    /// Filter by tags — connection must have ALL listed tags
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filter_tags: Vec<String>,
    /// Filter by host glob pattern (e.g. "*.prod.example.com")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_host_pattern: Option<String>,
    /// Filter by parent group ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_group_id: Option<Uuid>,
    /// Display order in sidebar
    #[serde(default)]
    pub sort_order: i32,
}
