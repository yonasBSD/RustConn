//! Property tests for the dynamic inventory sync engine.

use proptest::prelude::*;
use rustconn_core::models::{Connection, ConnectionGroup};
use rustconn_core::sync::{
    Inventory, InventoryEntry, SYNC_TAG_PREFIX, default_port_for_protocol, parse_inventory_json,
    parse_inventory_yaml, sync_inventory, sync_tag,
};

/// Strategy for generating valid inventory entries.
fn arb_inventory_entry() -> impl Strategy<Value = InventoryEntry> {
    (
        "[a-z][a-z0-9-]{1,15}",
        "(10\\.[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3})",
        prop_oneof![
            Just("ssh".to_string()),
            Just("rdp".to_string()),
            Just("vnc".to_string()),
            Just("telnet".to_string()),
        ],
        proptest::option::of(1u16..=65535),
        proptest::option::of("[a-z]{3,8}"),
        proptest::option::of("[A-Z][a-z]{3,10}"),
    )
        .prop_map(
            |(name, host, protocol, port, username, group)| InventoryEntry {
                name,
                host,
                protocol,
                port,
                username,
                group,
                tags: Vec::new(),
                description: None,
                icon: None,
            },
        )
}

fn arb_inventory() -> impl Strategy<Value = Inventory> {
    proptest::collection::vec(arb_inventory_entry(), 1..10)
        .prop_map(|connections| Inventory { connections })
}

proptest! {
    #[test]
    fn sync_adds_all_new_connections(inventory in arb_inventory()) {
        let mut connections: Vec<Connection> = Vec::new();
        let mut groups: Vec<ConnectionGroup> = Vec::new();

        let result = sync_inventory(
            &inventory, "test", &mut connections, &mut groups, false,
        );

        let expected = inventory.connections.iter()
            .filter(|e| !e.name.trim().is_empty() && !e.host.trim().is_empty())
            .count();

        prop_assert_eq!(result.added, expected);
        prop_assert_eq!(result.updated, 0);
        prop_assert_eq!(result.removed, 0);
        prop_assert_eq!(connections.len(), expected);

        // All connections should have the sync tag
        let tag = sync_tag("test");
        for conn in &connections {
            prop_assert!(conn.tags.contains(&tag));
        }
    }

    #[test]
    fn sync_idempotent_no_changes_on_second_run(inventory in arb_inventory()) {
        let mut connections: Vec<Connection> = Vec::new();
        let mut groups: Vec<ConnectionGroup> = Vec::new();

        // First sync
        sync_inventory(&inventory, "src", &mut connections, &mut groups, false);
        let count_after_first = connections.len();

        // Second sync — should produce zero adds/updates
        let result = sync_inventory(
            &inventory, "src", &mut connections, &mut groups, false,
        );

        prop_assert_eq!(result.added, 0);
        prop_assert_eq!(result.updated, 0);
        prop_assert_eq!(result.removed, 0);
        prop_assert_eq!(connections.len(), count_after_first);
    }

    #[test]
    fn sync_remove_stale_cleans_absent_connections(inventory in arb_inventory()) {
        let mut connections: Vec<Connection> = Vec::new();
        let mut groups: Vec<ConnectionGroup> = Vec::new();

        // First sync — populate
        sync_inventory(&inventory, "src", &mut connections, &mut groups, false);
        let initial = connections.len();

        // Second sync with empty inventory + remove_stale
        let empty = Inventory { connections: vec![] };
        let result = sync_inventory(
            &empty, "src", &mut connections, &mut groups, true,
        );

        prop_assert_eq!(result.removed, initial);
        prop_assert_eq!(connections.len(), 0);
    }
}

proptest! {
    #[test]
    fn sync_does_not_touch_unrelated_connections(inventory in arb_inventory()) {
        // Pre-existing connection from a different source
        let mut manual = Connection::new_ssh(
            "manual-server".to_string(),
            "192.168.1.1".to_string(),
            22,
        );
        manual.tags = vec!["manual".to_string()];

        let mut connections = vec![manual];
        let mut groups: Vec<ConnectionGroup> = Vec::new();

        sync_inventory(&inventory, "src", &mut connections, &mut groups, true);

        // The manual connection should still be there
        prop_assert!(connections.iter().any(|c| c.name == "manual-server"));
    }

    #[test]
    fn sync_creates_groups_for_new_group_names(inventory in arb_inventory()) {
        let mut connections: Vec<Connection> = Vec::new();
        let mut groups: Vec<ConnectionGroup> = Vec::new();

        sync_inventory(&inventory, "src", &mut connections, &mut groups, false);

        let expected_groups: std::collections::HashSet<_> = inventory
            .connections
            .iter()
            .filter(|e| !e.name.trim().is_empty() && !e.host.trim().is_empty())
            .filter_map(|e| e.group.as_ref())
            .collect();

        for group_name in &expected_groups {
            prop_assert!(
                groups.iter().any(|g| &g.name == *group_name),
                "Group '{}' should have been created",
                group_name
            );
        }
    }
}

// --- Deterministic unit tests ---

#[test]
fn test_sync_tag_format() {
    assert_eq!(sync_tag("netbox"), "sync:netbox");
    assert_eq!(sync_tag("ansible"), "sync:ansible");
    assert!(sync_tag("x").starts_with(SYNC_TAG_PREFIX));
}

#[test]
fn test_default_port_for_protocol() {
    assert_eq!(default_port_for_protocol("ssh"), 22);
    assert_eq!(default_port_for_protocol("SSH"), 22);
    assert_eq!(default_port_for_protocol("rdp"), 3389);
    assert_eq!(default_port_for_protocol("vnc"), 5900);
    assert_eq!(default_port_for_protocol("telnet"), 23);
    assert_eq!(default_port_for_protocol("unknown"), 22);
}

#[test]
fn test_parse_inventory_json() {
    let json = r#"{
        "connections": [
            {
                "name": "web-01",
                "host": "10.0.1.5",
                "protocol": "ssh",
                "port": 2222,
                "username": "admin",
                "group": "Production"
            },
            {
                "name": "db-01",
                "host": "10.0.2.10"
            }
        ]
    }"#;

    let inv = parse_inventory_json(json).unwrap();
    assert_eq!(inv.connections.len(), 2);
    assert_eq!(inv.connections[0].name, "web-01");
    assert_eq!(inv.connections[0].port, Some(2222));
    assert_eq!(inv.connections[0].username.as_deref(), Some("admin"));
    assert_eq!(inv.connections[0].group.as_deref(), Some("Production"));
    // Second entry uses defaults
    assert_eq!(inv.connections[1].protocol, "ssh");
    assert_eq!(inv.connections[1].port, None);
}

#[test]
fn test_parse_inventory_yaml() {
    let yaml = "connections:\n  - name: web-01\n    host: 10.0.1.5\n    protocol: rdp\n";
    let inv = parse_inventory_yaml(yaml).unwrap();
    assert_eq!(inv.connections.len(), 1);
    assert_eq!(inv.connections[0].protocol, "rdp");
}

#[test]
fn test_sync_skips_empty_name_or_host() {
    let inventory = Inventory {
        connections: vec![
            InventoryEntry {
                name: String::new(),
                host: "10.0.1.1".to_string(),
                protocol: "ssh".to_string(),
                port: None,
                username: None,
                group: None,
                tags: Vec::new(),
                description: None,
                icon: None,
            },
            InventoryEntry {
                name: "valid".to_string(),
                host: "  ".to_string(),
                protocol: "ssh".to_string(),
                port: None,
                username: None,
                group: None,
                tags: Vec::new(),
                description: None,
                icon: None,
            },
        ],
    };

    let mut connections = Vec::new();
    let mut groups = Vec::new();
    let result = sync_inventory(&inventory, "t", &mut connections, &mut groups, false);

    assert_eq!(result.added, 0);
    assert_eq!(result.skipped, 2);
    assert_eq!(result.skip_reasons.len(), 2);
}

#[test]
fn test_sync_skips_unknown_protocol() {
    let inventory = Inventory {
        connections: vec![InventoryEntry {
            name: "srv".to_string(),
            host: "10.0.1.1".to_string(),
            protocol: "ftp".to_string(),
            port: None,
            username: None,
            group: None,
            tags: Vec::new(),
            description: None,
            icon: None,
        }],
    };

    let mut connections = Vec::new();
    let mut groups = Vec::new();
    let result = sync_inventory(&inventory, "t", &mut connections, &mut groups, false);

    assert_eq!(result.skipped, 1);
    assert!(result.skip_reasons[0].contains("ftp"));
}

#[test]
fn test_sync_updates_changed_port() {
    let mk = |port: u16| Inventory {
        connections: vec![InventoryEntry {
            name: "srv".to_string(),
            host: "10.0.1.1".to_string(),
            protocol: "ssh".to_string(),
            port: Some(port),
            username: None,
            group: None,
            tags: Vec::new(),
            description: None,
            icon: None,
        }],
    };

    let mut connections = Vec::new();
    let mut groups = Vec::new();

    sync_inventory(&mk(22), "s", &mut connections, &mut groups, false);
    assert_eq!(connections[0].port, 22);

    let result = sync_inventory(&mk(2222), "s", &mut connections, &mut groups, false);
    assert_eq!(result.updated, 1);
    assert_eq!(result.added, 0);
    assert_eq!(connections[0].port, 2222);
}

#[test]
fn test_sync_multiple_sources_independent() {
    let mk = |name: &str, host: &str, proto: &str| Inventory {
        connections: vec![InventoryEntry {
            name: name.to_string(),
            host: host.to_string(),
            protocol: proto.to_string(),
            port: None,
            username: None,
            group: None,
            tags: Vec::new(),
            description: None,
            icon: None,
        }],
    };

    let mut connections = Vec::new();
    let mut groups = Vec::new();

    sync_inventory(
        &mk("srv-a", "10.0.1.1", "ssh"),
        "source-a",
        &mut connections,
        &mut groups,
        false,
    );
    sync_inventory(
        &mk("srv-b", "10.0.2.1", "rdp"),
        "source-b",
        &mut connections,
        &mut groups,
        false,
    );
    assert_eq!(connections.len(), 2);

    // Remove stale from source-a should not touch source-b
    let empty = Inventory {
        connections: vec![],
    };
    let result = sync_inventory(&empty, "source-a", &mut connections, &mut groups, true);
    assert_eq!(result.removed, 1);
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0].name, "srv-b");
}
