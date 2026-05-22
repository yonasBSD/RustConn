//! Property-based tests for sync filename path traversal prevention.
//!
//! **Validates: TEST-1 — fuzz sync_file with `..`, absolute paths, directory separators.**
//!
//! `validate_sync_filename` must reject any filename that could escape the sync directory.

use proptest::prelude::*;
use rustconn_core::sync::validate_sync_filename;

/// Strategy for generating path traversal attempts.
fn traversal_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Parent directory traversal
        Just("../secret.rcn".to_string()),
        Just("../../etc/passwd".to_string()),
        Just("..".to_string()),
        Just("foo/../bar.rcn".to_string()),
        // Absolute paths
        Just("/etc/passwd".to_string()),
        Just("/tmp/evil.rcn".to_string()),
        // Directory separators
        Just("subdir/file.rcn".to_string()),
        Just("a/b/c.rcn".to_string()),
        // Generated traversal patterns
        "\\.\\.(/[a-z]{1,5}){1,4}".prop_map(|s| s),
        "/[a-z/]{1,20}".prop_map(|s| s),
    ]
}

/// Strategy for generating valid simple filenames.
fn valid_filename_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_-]{0,20}\\.rcn"
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Any path with `..` components must be rejected.
    #[test]
    fn rejects_parent_traversal(path in traversal_strategy()) {
        let result = validate_sync_filename(&path);
        prop_assert!(
            result.is_err(),
            "Path traversal '{path}' should be rejected but was accepted"
        );
    }

    /// Valid simple filenames (no separators, no traversal) must be accepted.
    #[test]
    fn accepts_valid_filenames(name in valid_filename_strategy()) {
        let result = validate_sync_filename(&name);
        prop_assert!(
            result.is_ok(),
            "Valid filename '{name}' should be accepted but was rejected: {:?}",
            result.err()
        );
    }

    /// Filenames with directory separators must be rejected.
    #[test]
    fn rejects_subdirectory_paths(
        dir in "[a-z]{1,5}",
        file in "[a-z]{1,5}\\.rcn",
    ) {
        let path = format!("{dir}/{file}");
        let result = validate_sync_filename(&path);
        prop_assert!(
            result.is_err(),
            "Subdirectory path '{path}' should be rejected"
        );
    }

    /// Absolute paths must be rejected.
    #[test]
    fn rejects_absolute_paths(path in "/[a-z]{1,10}(/[a-z]{1,5}){0,3}\\.rcn") {
        let result = validate_sync_filename(&path);
        prop_assert!(
            result.is_err(),
            "Absolute path '{path}' should be rejected"
        );
    }

    /// Filenames "." and ".." must be rejected (special path components).
    #[test]
    fn rejects_dot_and_dotdot(_unused in 0u8..2) {
        let result_dot = validate_sync_filename(".");
        prop_assert!(result_dot.is_err(), "'.' should be rejected");

        let result_dotdot = validate_sync_filename("..");
        prop_assert!(result_dotdot.is_err(), "'..' should be rejected");
    }
}
