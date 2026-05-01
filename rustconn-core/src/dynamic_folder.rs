//! Dynamic folder executor for script-generated connections.
//!
//! Executes a user-defined script and parses the JSON output into
//! [`DynamicConnectionEntry`] objects that become read-only connections
//! inside a group.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::process::Stdio;
use std::time::Instant;

use thiserror::Error;
use uuid::Uuid;

use crate::models::{
    Connection, DynamicConnectionEntry, DynamicConnectionId, DynamicFolderConfig,
    DynamicFolderResult, ProtocolType,
};

/// Errors from dynamic folder operations
#[derive(Debug, Error)]
pub enum DynamicFolderError {
    /// Script execution failed
    #[error("Script execution failed: {0}")]
    ExecutionFailed(String),

    /// Script timed out
    #[error("Script timed out after {0} seconds")]
    Timeout(u64),

    /// Script returned non-zero exit code
    #[error("Script exited with code {code}: {stderr}")]
    NonZeroExit {
        /// Exit code
        code: i32,
        /// Stderr output
        stderr: String,
    },

    /// Failed to parse script output as JSON
    #[error("Failed to parse script output: {0}")]
    ParseError(String),

    /// Script produced empty output
    #[error("Script produced no output")]
    EmptyOutput,

    /// I/O error
    #[error("I/O error: {0}")]
    Io(String),
}

/// Result type for dynamic folder operations
pub type DynamicFolderResult2 = Result<DynamicFolderResult, DynamicFolderError>;

/// Executes a dynamic folder script and parses the output.
///
/// The script is run via `sh -c` and must output a JSON array of
/// [`DynamicConnectionEntry`] objects to stdout.
///
/// # Errors
///
/// Returns an error if the script fails, times out, or produces invalid output.
pub async fn execute_script(config: &DynamicFolderConfig) -> DynamicFolderResult2 {
    let start = Instant::now();

    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c").arg(&config.script);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    // Prevent the script from inheriting stdin
    cmd.stdin(Stdio::null());

    if let Some(ref dir) = config.working_directory {
        cmd.current_dir(dir);
    }

    let child = cmd
        .spawn()
        .map_err(|e| DynamicFolderError::Io(e.to_string()))?;

    let output = tokio::time::timeout(config.timeout(), child.wait_with_output())
        .await
        .map_err(|_| DynamicFolderError::Timeout(config.timeout_secs))?
        .map_err(|e| DynamicFolderError::ExecutionFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(-1);
        return Err(DynamicFolderError::NonZeroExit { code, stderr });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();

    if stdout.is_empty() {
        return Err(DynamicFolderError::EmptyOutput);
    }

    let (entries, warnings) = parse_entries(stdout)?;
    let duration = start.elapsed();

    Ok(DynamicFolderResult {
        entries,
        warnings,
        duration,
    })
}

/// Parses JSON output into connection entries with validation warnings.
fn parse_entries(
    json: &str,
) -> Result<(Vec<DynamicConnectionEntry>, Vec<String>), DynamicFolderError> {
    let raw_entries: Vec<DynamicConnectionEntry> =
        serde_json::from_str(json).map_err(|e| DynamicFolderError::ParseError(e.to_string()))?;

    let mut entries = Vec::with_capacity(raw_entries.len());
    let mut warnings = Vec::new();

    for (i, entry) in raw_entries.into_iter().enumerate() {
        if entry.name.trim().is_empty() {
            warnings.push(format!("Entry {i}: skipped — empty name"));
            continue;
        }
        if entry.host.trim().is_empty() {
            warnings.push(format!("Entry {i} ({}): skipped — empty host", entry.name));
            continue;
        }
        entries.push(entry);
    }

    Ok((entries, warnings))
}

/// Converts a [`DynamicConnectionEntry`] into a [`Connection`] assigned to the given group.
///
/// The connection gets a deterministic UUID derived from the group ID and entry content,
/// so repeated refreshes produce stable IDs for the same entries.
#[must_use]
pub fn entry_to_connection(entry: &DynamicConnectionEntry, group_id: Uuid) -> Connection {
    let id = stable_connection_id(group_id, entry);

    let protocol_type = match entry.protocol.to_lowercase().as_str() {
        "ssh" => ProtocolType::Ssh,
        "rdp" => ProtocolType::Rdp,
        "vnc" => ProtocolType::Vnc,
        "spice" => ProtocolType::Spice,
        "telnet" => ProtocolType::Telnet,
        "mosh" => ProtocolType::Mosh,
        _ => ProtocolType::Ssh,
    };

    let default_port = match protocol_type {
        ProtocolType::Ssh | ProtocolType::Sftp | ProtocolType::Mosh => 22,
        ProtocolType::Rdp => 3389,
        ProtocolType::Vnc | ProtocolType::Spice => 5900,
        ProtocolType::Telnet => 23,
        _ => 22,
    };
    let port = entry.port.unwrap_or(default_port);

    let mut conn = match protocol_type {
        ProtocolType::Ssh => Connection::new_ssh(entry.name.clone(), entry.host.clone(), port),
        ProtocolType::Rdp => Connection::new_rdp(entry.name.clone(), entry.host.clone(), port),
        ProtocolType::Vnc => Connection::new_vnc(entry.name.clone(), entry.host.clone(), port),
        ProtocolType::Spice => Connection::new_spice(entry.name.clone(), entry.host.clone(), port),
        ProtocolType::Telnet => {
            Connection::new_telnet(entry.name.clone(), entry.host.clone(), port)
        }
        ProtocolType::Mosh => Connection::new_mosh(entry.name.clone(), entry.host.clone(), port),
        _ => Connection::new_ssh(entry.name.clone(), entry.host.clone(), port),
    };

    conn.id = id;
    conn.group_id = Some(group_id);
    conn.username = entry.username.clone();
    conn.tags = entry.tags.clone();
    conn.description = entry.description.clone();
    conn.is_dynamic = true;

    conn
}

/// Generates a stable UUID for a dynamic connection entry.
///
/// The UUID is deterministic based on group_id + name + host + protocol,
/// so the same entry always gets the same ID across refreshes.
#[must_use]
pub fn stable_connection_id(group_id: Uuid, entry: &DynamicConnectionEntry) -> Uuid {
    let dynamic_id = DynamicConnectionId {
        group_id,
        entry_hash: compute_entry_hash(entry),
    };

    // Create a deterministic UUID v5 from the hash
    let mut bytes = [0u8; 16];
    let group_bytes = group_id.as_bytes();
    let hash_bytes = dynamic_id.entry_hash.to_le_bytes();
    bytes[..8].copy_from_slice(&group_bytes[..8]);
    bytes[8..16].copy_from_slice(&hash_bytes);
    // Set version 4 bits to make it a valid UUID
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// Computes a hash for an entry based on name + host + protocol.
fn compute_entry_hash(entry: &DynamicConnectionEntry) -> u64 {
    let mut hasher = DefaultHasher::new();
    entry.name.hash(&mut hasher);
    entry.host.hash(&mut hasher);
    entry.protocol.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_entries_valid() {
        let json = r#"[
            {"name": "web-01", "host": "10.0.0.1"},
            {"name": "web-02", "host": "10.0.0.2", "port": 2222, "username": "admin"}
        ]"#;

        let (entries, warnings) = parse_entries(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(warnings.is_empty());
        assert_eq!(entries[0].name, "web-01");
        assert_eq!(entries[0].protocol, "ssh");
        assert_eq!(entries[1].port, Some(2222));
        assert_eq!(entries[1].username.as_deref(), Some("admin"));
    }

    #[test]
    fn test_parse_entries_skips_invalid() {
        let json = r#"[
            {"name": "", "host": "10.0.0.1"},
            {"name": "valid", "host": ""},
            {"name": "good", "host": "10.0.0.3"}
        ]"#;

        let (entries, warnings) = parse_entries(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "good");
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn test_parse_entries_invalid_json() {
        let result = parse_entries("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_stable_id_deterministic() {
        let group_id = Uuid::new_v4();
        let entry = DynamicConnectionEntry {
            name: "test".to_string(),
            host: "10.0.0.1".to_string(),
            port: None,
            protocol: "ssh".to_string(),
            username: None,
            group: None,
            tags: Vec::new(),
            description: None,
        };

        let id1 = stable_connection_id(group_id, &entry);
        let id2 = stable_connection_id(group_id, &entry);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_stable_id_differs_for_different_entries() {
        let group_id = Uuid::new_v4();
        let entry1 = DynamicConnectionEntry {
            name: "web-01".to_string(),
            host: "10.0.0.1".to_string(),
            port: None,
            protocol: "ssh".to_string(),
            username: None,
            group: None,
            tags: Vec::new(),
            description: None,
        };
        let entry2 = DynamicConnectionEntry {
            name: "web-02".to_string(),
            host: "10.0.0.2".to_string(),
            port: None,
            protocol: "ssh".to_string(),
            username: None,
            group: None,
            tags: Vec::new(),
            description: None,
        };

        let id1 = stable_connection_id(group_id, &entry1);
        let id2 = stable_connection_id(group_id, &entry2);
        assert_ne!(id1, id2);
    }
}
