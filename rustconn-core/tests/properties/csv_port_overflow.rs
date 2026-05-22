//! Property-based tests for CSV port overflow handling.
//!
//! **Validates: TEST-1 — CSV with port > u16::MAX must produce a skipped entry, never panic.**

use proptest::prelude::*;
use rustconn_core::import::CsvImporter;

/// Strategy for generating port values that overflow u16 (> 65535).
fn overflow_port_strategy() -> impl Strategy<Value = u64> {
    65536u64..=u64::MAX / 2
}

/// Strategy for generating port values that are zero (invalid).
fn zero_port_strategy() -> impl Strategy<Value = u64> {
    Just(0u64)
}

/// Strategy for generating non-numeric port strings that are clearly invalid.
fn garbage_port_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("abc".to_string()),
        Just("-1".to_string()),
        Just("99999999999999999999".to_string()),
        Just("3.14".to_string()),
        Just("0x16".to_string()),
        "[a-z]{2,10}".prop_map(|s| s),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// CSV row with port > 65535 must be skipped (not panic, not import with truncated port).
    #[test]
    fn csv_port_overflow_is_skipped(port in overflow_port_strategy()) {
        let csv = format!("name,host,port,protocol\nserver1,10.0.0.1,{port},ssh\n");
        let importer = CsvImporter::new();
        let result = importer.parse_csv(&csv);

        // Connection must NOT be imported
        prop_assert_eq!(
            result.connections.len(), 0,
        );
        // Must appear in skipped entries
        prop_assert!(
            !result.skipped.is_empty(),
            "Port overflow should produce a skipped entry"
        );
    }

    /// CSV row with port = 0 must be skipped.
    #[test]
    fn csv_port_zero_is_skipped(port in zero_port_strategy()) {
        let csv = format!("name,host,port,protocol\nserver1,10.0.0.1,{port},ssh\n");
        let importer = CsvImporter::new();
        let result = importer.parse_csv(&csv);

        prop_assert_eq!(
            result.connections.len(), 0,
        );
        prop_assert!(!result.skipped.is_empty());
    }

    /// CSV row with non-numeric port must be skipped.
    #[test]
    fn csv_port_garbage_is_skipped(port in garbage_port_strategy()) {
        let csv = format!("name,host,port,protocol\nserver1,10.0.0.1,{port},ssh\n");
        let importer = CsvImporter::new();
        let result = importer.parse_csv(&csv);

        prop_assert_eq!(
            result.connections.len(), 0,
        );
        prop_assert!(!result.skipped.is_empty());
    }

    /// Valid port (1..=65535) must be imported successfully.
    #[test]
    fn csv_valid_port_is_imported(port in 1u16..=65535) {
        let csv = format!("name,host,port,protocol\nserver1,10.0.0.1,{port},ssh\n");
        let importer = CsvImporter::new();
        let result = importer.parse_csv(&csv);

        prop_assert_eq!(
            result.connections.len(), 1,
        );
        prop_assert_eq!(result.connections[0].port, port);
    }
}
