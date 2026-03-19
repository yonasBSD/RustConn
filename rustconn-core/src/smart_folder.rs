//! Smart folder manager for evaluating dynamic connection filters.

use glob::Pattern;
use uuid::Uuid;

use crate::models::{Connection, SmartFolder};

/// Manages a collection of smart folders and evaluates their filters
/// against connections.
#[derive(Debug, Default)]
pub struct SmartFolderManager {
    folders: Vec<SmartFolder>,
}

impl SmartFolderManager {
    /// Creates a new empty `SmartFolderManager`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            folders: Vec::new(),
        }
    }

    /// Adds a smart folder to the manager.
    pub fn add(&mut self, folder: SmartFolder) {
        self.folders.push(folder);
    }

    /// Removes a smart folder by ID. Returns `true` if found and removed.
    pub fn remove(&mut self, id: &Uuid) -> bool {
        let len_before = self.folders.len();
        self.folders.retain(|f| f.id != *id);
        self.folders.len() < len_before
    }

    /// Returns a reference to a smart folder by ID, if it exists.
    #[must_use]
    pub fn get(&self, id: &Uuid) -> Option<&SmartFolder> {
        self.folders.iter().find(|f| f.id == *id)
    }

    /// Returns a slice of all managed smart folders.
    #[must_use]
    pub fn list(&self) -> &[SmartFolder] {
        &self.folders
    }

    /// Evaluate a smart folder against a list of connections.
    ///
    /// Returns connections matching **all** active filter criteria (AND logic).
    /// If no filter criteria are set, returns an empty vector.
    #[must_use]
    pub fn evaluate<'a>(
        &self,
        folder: &SmartFolder,
        connections: &'a [Connection],
    ) -> Vec<&'a Connection> {
        // Empty filter criteria → empty result
        if !has_any_filter(folder) {
            return Vec::new();
        }

        // Pre-compile the glob pattern once (if present and valid)
        let compiled_pattern = folder
            .filter_host_pattern
            .as_ref()
            .and_then(|p| Pattern::new(p).ok());

        connections
            .iter()
            .filter(|conn| matches_all(folder, conn, compiled_pattern.as_ref()))
            .collect()
    }
}

/// Returns `true` if the folder has at least one active filter criterion.
fn has_any_filter(folder: &SmartFolder) -> bool {
    folder.filter_protocol.is_some()
        || !folder.filter_tags.is_empty()
        || folder.filter_host_pattern.is_some()
        || folder.filter_group_id.is_some()
}

/// Returns `true` if the connection matches **all** active filters in the folder.
fn matches_all(
    folder: &SmartFolder,
    conn: &Connection,
    compiled_pattern: Option<&Pattern>,
) -> bool {
    // Protocol filter
    if let Some(ref proto) = folder.filter_protocol
        && conn.protocol != *proto
    {
        return false;
    }

    // Tags filter — every tag in the filter must be present in the connection
    if !folder.filter_tags.is_empty()
        && !folder.filter_tags.iter().all(|tag| conn.tags.contains(tag))
    {
        return false;
    }

    // Host glob pattern filter
    if folder.filter_host_pattern.is_some() {
        match compiled_pattern {
            Some(pattern) => {
                if !pattern.matches(&conn.host) {
                    return false;
                }
            }
            // Invalid glob pattern → treat as non-matching
            None => return false,
        }
    }

    // Group ID filter
    if let Some(ref group_id) = folder.filter_group_id
        && conn.group_id.as_ref() != Some(group_id)
    {
        return false;
    }

    true
}
