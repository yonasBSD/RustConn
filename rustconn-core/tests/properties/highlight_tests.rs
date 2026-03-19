//! Property-based tests for highlight rules: regex validation, serde round-trip,
//! and match-position correctness.
//!
//! **Validates: Requirements 7.8, 11.6**

use proptest::prelude::*;
use rustconn_core::highlight::{CompiledHighlightRules, builtin_defaults};
use rustconn_core::models::HighlightRule;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

/// Generates a syntactically valid regex pattern (simple literal or character class).
fn valid_regex_pattern() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple word literals — always valid regex
        "[a-zA-Z]{1,12}",
        // Word boundary patterns like \bWORD\b
        "[a-zA-Z]{1,8}".prop_map(|w| format!(r"\b{w}\b")),
        // Character class patterns
        Just("[a-z]+".to_string()),
        Just("[0-9]+".to_string()),
        Just("(?i)error".to_string()),
        Just(r"\d{1,5}".to_string()),
    ]
}

/// Generates an invalid regex pattern that will fail `Regex::new()`.
fn invalid_regex_pattern() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("(unclosed".to_string()),
        Just("[unclosed".to_string()),
        Just("*leading_star".to_string()),
        Just(r"\p{BadUnicode}".to_string()),
        Just(")bad".to_string()),
    ]
}

/// Optional CSS hex color (#RRGGBB).
fn optional_hex_color() -> impl Strategy<Value = Option<String>> {
    prop::option::of(
        prop::collection::vec(
            prop::sample::select(vec![
                '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
            ]),
            6..=6,
        )
        .prop_map(|chars| format!("#{}", chars.into_iter().collect::<String>())),
    )
}

/// Generates a `HighlightRule` with a valid regex pattern.
fn valid_highlight_rule() -> impl Strategy<Value = HighlightRule> {
    (
        "[a-zA-Z0-9 _-]{1,20}", // name
        valid_regex_pattern(),
        optional_hex_color(),
        optional_hex_color(),
        any::<bool>(),
    )
        .prop_map(|(name, pattern, fg, bg, enabled)| HighlightRule {
            id: Uuid::new_v4(),
            name,
            pattern,
            foreground_color: fg,
            background_color: bg,
            enabled,
        })
}

// ---------------------------------------------------------------------------
// Proptest 14: Valid regex passes validate_pattern(), invalid does not
// **Validates: Requirements 7.8, 11.6**
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn valid_regex_passes_validate_pattern(pattern in valid_regex_pattern()) {
        let rule = HighlightRule {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            pattern,
            foreground_color: None,
            background_color: None,
            enabled: true,
        };
        prop_assert!(rule.validate_pattern().is_ok());
    }

    #[test]
    fn invalid_regex_fails_validate_pattern(pattern in invalid_regex_pattern()) {
        let rule = HighlightRule {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            pattern,
            foreground_color: None,
            background_color: None,
            enabled: true,
        };
        prop_assert!(rule.validate_pattern().is_err());
    }
}

// ---------------------------------------------------------------------------
// Proptest 15: Serde round-trip for HighlightRule
// **Validates: Requirements 7.8, 11.6**
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn serde_roundtrip_preserves_highlight_rule(rule in valid_highlight_rule()) {
        let json = serde_json::to_string(&rule).map_err(|e| {
            TestCaseError::fail(format!("serialization failed: {e}"))
        })?;
        let restored: HighlightRule = serde_json::from_str(&json).map_err(|e| {
            TestCaseError::fail(format!("deserialization failed: {e}"))
        })?;
        prop_assert_eq!(&rule.id, &restored.id);
        prop_assert_eq!(&rule.name, &restored.name);
        prop_assert_eq!(&rule.pattern, &restored.pattern);
        prop_assert_eq!(&rule.foreground_color, &restored.foreground_color);
        prop_assert_eq!(&rule.background_color, &restored.background_color);
        prop_assert_eq!(rule.enabled, restored.enabled);
    }
}

// ---------------------------------------------------------------------------
// Proptest 16: Matching positions are correct for known patterns
// **Validates: Requirements 7.8, 11.6**
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn matching_positions_correct_for_literal(
        prefix in "[a-z]{0,10}",
        keyword in "[A-Z]{1,6}",
        suffix in "[a-z]{0,10}",
    ) {
        let line = format!("{prefix}{keyword}{suffix}");

        let rule = HighlightRule {
            id: Uuid::new_v4(),
            name: "literal".to_string(),
            pattern: keyword.clone(),
            foreground_color: Some("#FF0000".to_string()),
            background_color: None,
            enabled: true,
        };

        let compiled = CompiledHighlightRules::compile(&[], &[rule]);
        let matches = compiled.find_matches(&line);

        // The keyword must appear at least once (it's a literal substring)
        prop_assert!(!matches.is_empty(), "expected at least one match in '{line}'");

        for m in &matches {
            // Byte offsets must be within the line
            prop_assert!(m.start <= m.end);
            prop_assert!(m.end <= line.len());
            // The matched slice must equal the keyword
            prop_assert_eq!(&line[m.start..m.end], keyword.as_str());
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[test]
fn builtin_defaults_all_have_valid_patterns() {
    let defaults = builtin_defaults();
    for rule in &defaults {
        assert!(
            rule.validate_pattern().is_ok(),
            "built-in rule '{}' has invalid pattern '{}'",
            rule.name,
            rule.pattern,
        );
    }
}

#[test]
fn disabled_rule_produces_no_matches() {
    let rule = HighlightRule {
        id: Uuid::new_v4(),
        name: "disabled".to_string(),
        pattern: "XYZZY".to_string(),
        foreground_color: Some("#FF0000".to_string()),
        background_color: None,
        enabled: false,
    };
    let compiled = CompiledHighlightRules::compile(&[], &[rule]);
    let matches = compiled.find_matches("this line has XYZZY in it");
    // Only built-in defaults should match; none of them match "XYZZY"
    assert!(
        matches.iter().all(|m| {
            let slice = &"this line has XYZZY in it"[m.start..m.end];
            slice != "XYZZY"
        }),
        "disabled rule should produce no matches for its pattern",
    );
}

#[test]
fn invalid_regex_rule_is_skipped_in_compile() {
    let bad_rule = HighlightRule {
        id: Uuid::new_v4(),
        name: "bad".to_string(),
        pattern: "(unclosed".to_string(),
        foreground_color: None,
        background_color: None,
        enabled: true,
    };
    let good_rule = HighlightRule {
        id: Uuid::new_v4(),
        name: "good".to_string(),
        pattern: "OK".to_string(),
        foreground_color: Some("#00FF00".to_string()),
        background_color: None,
        enabled: true,
    };
    let compiled = CompiledHighlightRules::compile(&[bad_rule, good_rule], &[]);
    let matches = compiled.find_matches("OK here");
    // The good rule should still match despite the bad rule being skipped
    assert!(!matches.is_empty());
    assert_eq!(matches[0].start, 0);
    assert_eq!(matches[0].end, 2);
}
