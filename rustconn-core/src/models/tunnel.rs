//! Standalone SSH tunnel model
//!
//! Represents an SSH port-forwarding tunnel that runs independently of
//! terminal sessions. Each tunnel references an existing SSH connection
//! for host/key/password configuration and defines one or more port
//! forwarding rules (`-L`, `-R`, `-D`).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::PortForward;

/// A standalone SSH tunnel that runs without a terminal session.
///
/// The tunnel uses an existing SSH connection (referenced by `connection_id`)
/// as the SSH host, inheriting its host, port, username, key, jump host,
/// and password source. This avoids duplicating connection configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandaloneTunnel {
    /// Unique identifier
    pub id: Uuid,
    /// Human-readable name (e.g. "MySQL prod", "SOCKS proxy")
    pub name: String,
    /// Reference to an existing SSH connection that provides
    /// host, port, username, identity file, jump host, and credentials
    pub connection_id: Uuid,
    /// Port forwarding rules (one `ssh -N` process handles all of them)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forwards: Vec<PortForward>,
    /// Start this tunnel automatically when the application launches
    #[serde(default)]
    pub auto_start: bool,
    /// Automatically reconnect when the tunnel process exits unexpectedly
    #[serde(default)]
    pub auto_reconnect: bool,
    /// Whether this tunnel is enabled (disabled tunnels are skipped by auto-start)
    #[serde(default = "default_true")]
    pub enabled: bool,
}

const fn default_true() -> bool {
    true
}

impl StandaloneTunnel {
    /// Creates a new tunnel with the given name and connection reference
    #[must_use]
    pub fn new(name: impl Into<String>, connection_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            connection_id,
            forwards: Vec::new(),
            auto_start: false,
            auto_reconnect: false,
            enabled: true,
        }
    }

    /// Adds a port forwarding rule to this tunnel
    #[must_use]
    pub fn with_forward(mut self, forward: PortForward) -> Self {
        self.forwards.push(forward);
        self
    }

    /// Returns a display summary of all forwards (e.g. "L 3306→db:3306, D 1080")
    #[must_use]
    pub fn forwards_summary(&self) -> String {
        self.forwards
            .iter()
            .map(PortForward::display_summary)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Runtime status of a standalone tunnel
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelStatus {
    /// Tunnel is not running
    Stopped,
    /// Tunnel is being started (SSH handshake in progress)
    Starting,
    /// Tunnel is running and healthy
    Running,
    /// Tunnel failed to start or crashed
    Failed(String),
}

impl TunnelStatus {
    /// Returns true if the tunnel is currently running
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Returns true if the tunnel is stopped (not running, not starting)
    #[must_use]
    pub const fn is_stopped(&self) -> bool {
        matches!(self, Self::Stopped)
    }
}

impl std::fmt::Display for TunnelStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "Stopped"),
            Self::Starting => write!(f, "Starting"),
            Self::Running => write!(f, "Running"),
            Self::Failed(msg) => write!(f, "Failed: {msg}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PortForwardDirection;

    #[test]
    fn test_tunnel_creation() {
        let conn_id = Uuid::new_v4();
        let tunnel = StandaloneTunnel::new("Test tunnel", conn_id);
        assert_eq!(tunnel.name, "Test tunnel");
        assert_eq!(tunnel.connection_id, conn_id);
        assert!(tunnel.forwards.is_empty());
        assert!(!tunnel.auto_start);
        assert!(!tunnel.auto_reconnect);
        assert!(tunnel.enabled);
    }

    #[test]
    fn test_tunnel_with_forward() {
        let conn_id = Uuid::new_v4();
        let tunnel = StandaloneTunnel::new("MySQL", conn_id).with_forward(PortForward {
            direction: PortForwardDirection::Local,
            local_port: 3306,
            remote_host: "db.internal".to_string(),
            remote_port: 3306,
        });
        assert_eq!(tunnel.forwards.len(), 1);
        assert_eq!(tunnel.forwards[0].local_port, 3306);
    }

    #[test]
    fn test_forwards_summary() {
        let conn_id = Uuid::new_v4();
        let tunnel = StandaloneTunnel::new("Multi", conn_id)
            .with_forward(PortForward {
                direction: PortForwardDirection::Local,
                local_port: 3306,
                remote_host: "db.internal".to_string(),
                remote_port: 3306,
            })
            .with_forward(PortForward {
                direction: PortForwardDirection::Dynamic,
                local_port: 1080,
                remote_host: String::new(),
                remote_port: 0,
            });
        let summary = tunnel.forwards_summary();
        assert!(summary.contains("3306"));
        assert!(summary.contains("1080"));
    }

    #[test]
    fn test_tunnel_status() {
        assert!(TunnelStatus::Running.is_running());
        assert!(!TunnelStatus::Stopped.is_running());
        assert!(TunnelStatus::Stopped.is_stopped());
        assert!(!TunnelStatus::Running.is_stopped());
    }

    #[test]
    fn test_tunnel_serialization() {
        let conn_id = Uuid::new_v4();
        let tunnel = StandaloneTunnel::new("Test", conn_id).with_forward(PortForward {
            direction: PortForwardDirection::Local,
            local_port: 8080,
            remote_host: "localhost".to_string(),
            remote_port: 80,
        });
        let json = serde_json::to_string(&tunnel).expect("serialize");
        let parsed: StandaloneTunnel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(tunnel, parsed);
    }
}
