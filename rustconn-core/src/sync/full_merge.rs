//! UUID-based bidirectional merge engine for Simple Sync.
//!
//! [`FullMergeEngine`] implements the merge algorithm described in the design
//! doc: entities are matched by UUID, `updated_at` determines the winner,
//! tombstones track deletions, and expired tombstones are cleaned up.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use super::full_export::FullSyncExport;
use super::tombstone::{SyncEntityType, Tombstone};

/// Local device state used as input to the merge algorithm.
#[derive(Debug, Clone)]
pub struct LocalState {
    /// This device's unique identifier.
    pub device_id: Uuid,
    /// Local connection IDs with their `updated_at` timestamps.
    pub connections: HashMap<Uuid, DateTime<Utc>>,
    /// Local group IDs with their `updated_at` timestamps.
    pub groups: HashMap<Uuid, DateTime<Utc>>,
    /// Local template IDs with their `updated_at` timestamps.
    pub templates: HashMap<Uuid, DateTime<Utc>>,
    /// Local snippet IDs with their `updated_at` timestamps.
    pub snippets: HashMap<Uuid, DateTime<Utc>>,
    /// Local cluster IDs (use export time as proxy).
    pub clusters: HashMap<Uuid, DateTime<Utc>>,
    /// Local tombstones.
    pub tombstones: Vec<Tombstone>,
    /// Tombstone retention in days.
    pub retention_days: u32,
}

/// Result of a full bidirectional merge operation.
#[derive(Debug, Clone, Default)]
pub struct FullMergeResult {
    /// Entity IDs to create locally (new from remote).
    pub to_create: Vec<MergeAction>,
    /// Entity IDs to update locally (remote is newer).
    pub to_update: Vec<MergeAction>,
    /// Entity IDs to delete locally (remote tombstone wins).
    pub to_delete: Vec<MergeAction>,
    /// Tombstones to remove (expired past retention_days).
    pub tombstones_to_remove: Vec<Tombstone>,
}

/// A single merge action identifying the entity type and ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeAction {
    /// Type of entity affected.
    pub entity_type: SyncEntityType,
    /// UUID of the entity.
    pub id: Uuid,
}

/// UUID-based bidirectional merge engine for Simple Sync.
///
/// Implements the algorithm from the design doc:
/// - Remote entities are matched to local by UUID
/// - `updated_at` determines which version wins (newer wins)
/// - Tombstones override entities when `deleted_at > updated_at`
/// - Expired tombstones (older than `retention_days`) are cleaned up
pub struct FullMergeEngine;

impl FullMergeEngine {
    /// Performs a bidirectional merge between local state and a remote export.
    ///
    /// # Preconditions
    ///
    /// - `remote.device_id != local.device_id` (caller must check)
    /// - `remote.sync_type == "full"`
    ///
    /// # Algorithm
    ///
    /// For each entity type (connections, groups, templates, snippets, clusters):
    ///
    /// 1. **Remote → Local**: For each remote entity:
    ///    - If local tombstone exists with `deleted_at > entity.updated_at` → skip
    ///    - If entity exists locally with `local.updated_at >= remote.updated_at` → skip
    ///    - If entity exists locally with `remote.updated_at > local.updated_at` → update
    ///    - If entity doesn't exist locally and no tombstone → create
    ///
    /// 2. **Remote tombstones → Local**: For each remote tombstone:
    ///    - If entity exists locally with `updated_at < tombstone.deleted_at` → delete
    ///
    /// 3. **Cleanup**: Remove tombstones older than `retention_days`
    #[must_use]
    pub fn merge(local: &LocalState, remote: &FullSyncExport) -> FullMergeResult {
        let mut result = FullMergeResult::default();

        // Build tombstone lookup maps
        let local_tombstone_map = build_tombstone_map(&local.tombstones);
        let remote_tombstone_map = build_tombstone_map(&remote.tombstones);

        // Process each entity type
        Self::merge_entity_type(
            SyncEntityType::Connection,
            &local.connections,
            &remote
                .connections
                .iter()
                .map(|c| (c.id, c.updated_at))
                .collect(),
            &local_tombstone_map,
            &remote_tombstone_map,
            &mut result,
        );

        Self::merge_entity_type(
            SyncEntityType::Group,
            &local.groups,
            &remote.groups.iter().map(|g| (g.id, g.created_at)).collect(),
            &local_tombstone_map,
            &remote_tombstone_map,
            &mut result,
        );

        Self::merge_entity_type(
            SyncEntityType::Template,
            &local.templates,
            &remote
                .templates
                .iter()
                .map(|t| (t.id, t.updated_at))
                .collect(),
            &local_tombstone_map,
            &remote_tombstone_map,
            &mut result,
        );

        Self::merge_entity_type(
            SyncEntityType::Snippet,
            &local.snippets,
            &remote
                .snippets
                .iter()
                .map(|s| (s.id, s.updated_at))
                .collect(),
            &local_tombstone_map,
            &remote_tombstone_map,
            &mut result,
        );

        Self::merge_entity_type(
            SyncEntityType::Cluster,
            &local.clusters,
            &remote
                .clusters
                .iter()
                .map(|c| (c.id, remote.exported_at))
                .collect(),
            &local_tombstone_map,
            &remote_tombstone_map,
            &mut result,
        );

        // Cleanup expired tombstones.
        //
        // Only remove tombstones that are expired by the LOCAL retention policy.
        // Remote tombstones are NOT cleaned up here — they are managed by the
        // remote device's own retention policy. If we cleaned remote tombstones
        // using our (potentially shorter) retention, a deleted entity could
        // "resurrect" on our device when the remote re-exports it.
        let now = Utc::now();
        let cutoff = now - Duration::days(i64::from(local.retention_days));
        for tombstone in &local.tombstones {
            if tombstone.deleted_at < cutoff {
                result.tombstones_to_remove.push(tombstone.clone());
            }
        }

        result
    }

    /// Merges a single entity type between local and remote.
    fn merge_entity_type(
        entity_type: SyncEntityType,
        local_entities: &HashMap<Uuid, DateTime<Utc>>,
        remote_entities: &HashMap<Uuid, DateTime<Utc>>,
        local_tombstones: &HashMap<(SyncEntityType, Uuid), DateTime<Utc>>,
        remote_tombstones: &HashMap<(SyncEntityType, Uuid), DateTime<Utc>>,
        result: &mut FullMergeResult,
    ) {
        // Remote → Local reconciliation
        for (&remote_id, &remote_updated_at) in remote_entities {
            // Check if locally tombstoned
            if let Some(&deleted_at) = local_tombstones.get(&(entity_type, remote_id))
                && deleted_at > remote_updated_at
            {
                continue; // Deleted locally after last remote update
            }

            if let Some(&local_updated_at) = local_entities.get(&remote_id) {
                if local_updated_at >= remote_updated_at {
                    continue; // Local is same or newer
                }
                // Remote is newer → update
                result.to_update.push(MergeAction {
                    entity_type,
                    id: remote_id,
                });
            } else {
                // Not in local → create
                result.to_create.push(MergeAction {
                    entity_type,
                    id: remote_id,
                });
            }
        }

        // Apply remote tombstones locally
        for (&(tomb_type, tomb_id), &deleted_at) in remote_tombstones {
            if tomb_type != entity_type {
                continue;
            }
            if let Some(&local_updated_at) = local_entities.get(&tomb_id)
                && deleted_at > local_updated_at
            {
                result.to_delete.push(MergeAction {
                    entity_type,
                    id: tomb_id,
                });
            }
        }
    }
}

/// Builds a lookup map from tombstones: `(entity_type, id) → deleted_at`.
///
/// If multiple tombstones exist for the same entity, the latest `deleted_at` wins.
fn build_tombstone_map(tombstones: &[Tombstone]) -> HashMap<(SyncEntityType, Uuid), DateTime<Utc>> {
    let mut map = HashMap::new();
    for t in tombstones {
        let key = (t.entity_type, t.id);
        let entry = map.entry(key).or_insert(t.deleted_at);
        if t.deleted_at > *entry {
            *entry = t.deleted_at;
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_remote(device_id: Uuid) -> FullSyncExport {
        FullSyncExport {
            sync_version: 1,
            sync_type: "full".to_owned(),
            exported_at: Utc::now(),
            app_version: "0.12.0".to_owned(),
            device_id,
            device_name: "remote".to_owned(),
            connections: Vec::new(),
            groups: Vec::new(),
            templates: Vec::new(),
            snippets: Vec::new(),
            clusters: Vec::new(),
            variables: Vec::new(),
            tombstones: Vec::new(),
        }
    }

    fn empty_local() -> LocalState {
        LocalState {
            device_id: Uuid::new_v4(),
            connections: HashMap::new(),
            groups: HashMap::new(),
            templates: HashMap::new(),
            snippets: HashMap::new(),
            clusters: HashMap::new(),
            tombstones: Vec::new(),
            retention_days: 30,
        }
    }

    #[test]
    fn empty_merge_produces_no_actions() {
        let local = empty_local();
        let remote = empty_remote(Uuid::new_v4());
        let result = FullMergeEngine::merge(&local, &remote);

        assert!(result.to_create.is_empty());
        assert!(result.to_update.is_empty());
        assert!(result.to_delete.is_empty());
        assert!(result.tombstones_to_remove.is_empty());
    }

    #[test]
    fn new_remote_connection_creates_locally() {
        let local = empty_local();
        let mut remote = empty_remote(Uuid::new_v4());

        let conn = crate::models::Connection::new_ssh("test".to_owned(), "host".to_owned(), 22);
        let conn_id = conn.id;
        remote.connections.push(conn);

        let result = FullMergeEngine::merge(&local, &remote);

        assert_eq!(result.to_create.len(), 1);
        assert_eq!(result.to_create[0].entity_type, SyncEntityType::Connection);
        assert_eq!(result.to_create[0].id, conn_id);
    }

    #[test]
    fn newer_remote_updates_local() {
        let now = Utc::now();
        let mut local = empty_local();
        let conn_id = Uuid::new_v4();
        local.connections.insert(conn_id, now - Duration::hours(1));

        let mut remote = empty_remote(Uuid::new_v4());
        let mut conn = crate::models::Connection::new_ssh("test".to_owned(), "host".to_owned(), 22);
        conn.id = conn_id;
        conn.updated_at = now;
        remote.connections.push(conn);

        let result = FullMergeEngine::merge(&local, &remote);

        assert_eq!(result.to_update.len(), 1);
        assert_eq!(result.to_update[0].id, conn_id);
    }

    #[test]
    fn older_remote_keeps_local() {
        let now = Utc::now();
        let mut local = empty_local();
        let conn_id = Uuid::new_v4();
        local.connections.insert(conn_id, now);

        let mut remote = empty_remote(Uuid::new_v4());
        let mut conn = crate::models::Connection::new_ssh("test".to_owned(), "host".to_owned(), 22);
        conn.id = conn_id;
        conn.updated_at = now - Duration::hours(1);
        remote.connections.push(conn);

        let result = FullMergeEngine::merge(&local, &remote);

        assert!(result.to_update.is_empty());
        assert!(result.to_create.is_empty());
    }

    #[test]
    fn local_tombstone_blocks_remote_create() {
        let now = Utc::now();
        let conn_id = Uuid::new_v4();

        let mut local = empty_local();
        local.tombstones.push(Tombstone::with_deleted_at(
            SyncEntityType::Connection,
            conn_id,
            now,
        ));

        let mut remote = empty_remote(Uuid::new_v4());
        let mut conn = crate::models::Connection::new_ssh("test".to_owned(), "host".to_owned(), 22);
        conn.id = conn_id;
        conn.updated_at = now - Duration::hours(1);
        remote.connections.push(conn);

        let result = FullMergeEngine::merge(&local, &remote);
        assert!(result.to_create.is_empty());
    }

    #[test]
    fn remote_tombstone_deletes_local() {
        let now = Utc::now();
        let conn_id = Uuid::new_v4();

        let mut local = empty_local();
        local.connections.insert(conn_id, now - Duration::hours(1));

        let mut remote = empty_remote(Uuid::new_v4());
        remote.tombstones.push(Tombstone::with_deleted_at(
            SyncEntityType::Connection,
            conn_id,
            now,
        ));

        let result = FullMergeEngine::merge(&local, &remote);

        assert_eq!(result.to_delete.len(), 1);
        assert_eq!(result.to_delete[0].id, conn_id);
    }

    #[test]
    fn remote_tombstone_does_not_delete_newer_local() {
        let now = Utc::now();
        let conn_id = Uuid::new_v4();

        let mut local = empty_local();
        local.connections.insert(conn_id, now);

        let mut remote = empty_remote(Uuid::new_v4());
        remote.tombstones.push(Tombstone::with_deleted_at(
            SyncEntityType::Connection,
            conn_id,
            now - Duration::hours(1),
        ));

        let result = FullMergeEngine::merge(&local, &remote);
        assert!(result.to_delete.is_empty());
    }

    #[test]
    fn expired_tombstones_are_cleaned_up() {
        let now = Utc::now();
        let mut local = empty_local();
        local.retention_days = 30;
        local.tombstones.push(Tombstone::with_deleted_at(
            SyncEntityType::Connection,
            Uuid::new_v4(),
            now - Duration::days(31),
        ));
        local.tombstones.push(Tombstone::with_deleted_at(
            SyncEntityType::Group,
            Uuid::new_v4(),
            now - Duration::days(5),
        ));

        let remote = empty_remote(Uuid::new_v4());
        let result = FullMergeEngine::merge(&local, &remote);

        assert_eq!(result.tombstones_to_remove.len(), 1);
        assert_eq!(
            result.tombstones_to_remove[0].entity_type,
            SyncEntityType::Connection
        );
    }
}
