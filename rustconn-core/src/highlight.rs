//! Compiled highlight-rule engine for regex-based terminal text highlighting.
//!
//! [`CompiledHighlightRules`] merges global and per-connection
//! [`HighlightRule`](crate::models::HighlightRule) sets, compiles their regex
//! patterns once, and exposes [`find_matches`](CompiledHighlightRules::find_matches)
//! to locate all matching regions in a line of terminal output.

use regex::{Regex, RegexSet};
use tracing::warn;
use uuid::Uuid;

use crate::models::HighlightRule;

// ---------------------------------------------------------------------------
// HighlightMatch
// ---------------------------------------------------------------------------

/// A single highlighted region within a line of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightMatch {
    /// Byte offset of the match start within the line.
    pub start: usize,
    /// Byte offset of the match end (exclusive) within the line.
    pub end: usize,
    /// Optional foreground (text) colour in CSS hex format (`#RRGGBB`).
    pub foreground_color: Option<String>,
    /// Optional background colour in CSS hex format (`#RRGGBB`).
    pub background_color: Option<String>,
}

// ---------------------------------------------------------------------------
// CompiledRule (internal)
// ---------------------------------------------------------------------------

/// A single rule whose regex has been successfully compiled.
struct CompiledRule {
    regex: Regex,
    name: String,
    pattern: String,
    foreground_color: Option<String>,
    background_color: Option<String>,
}

// ---------------------------------------------------------------------------
// CompiledHighlightRules
// ---------------------------------------------------------------------------

/// Pre-compiled set of highlight rules ready for matching.
///
/// Created via [`compile`](Self::compile) which merges global and per-connection
/// rule lists.  Per-connection rules take priority: if a per-connection rule
/// shares the same `id` as a global rule, the per-connection version wins.
pub struct CompiledHighlightRules {
    rules: Vec<CompiledRule>,
    /// Pre-compiled `RegexSet` used to quickly determine which rules match a
    /// given line before running the individual (heavier) `Regex` objects.
    regex_set: RegexSet,
}

impl CompiledHighlightRules {
    /// Compiles global and per-connection highlight rules into a single set.
    ///
    /// Per-connection rules take priority: when a per-connection rule has the
    /// same `id` as a global rule, only the per-connection version is kept.
    /// Disabled rules and rules with invalid regex patterns are silently
    /// skipped (invalid patterns produce a `tracing::warn!`).
    ///
    /// Built-in default rules (ERROR, WARNING, CRITICAL, FATAL) are always
    /// prepended to the global set so they apply unless overridden.
    #[must_use]
    pub fn compile(global_rules: &[HighlightRule], per_conn_rules: &[HighlightRule]) -> Self {
        // Start with built-in defaults, then append user-supplied globals.
        let mut merged: Vec<&HighlightRule> = Vec::new();

        let defaults = builtin_defaults();
        for rule in &defaults {
            merged.push(rule);
        }
        for rule in global_rules {
            merged.push(rule);
        }

        // Per-connection rules override globals with the same id.
        let per_conn_ids: std::collections::HashSet<Uuid> =
            per_conn_rules.iter().map(|r| r.id).collect();

        merged.retain(|r| !per_conn_ids.contains(&r.id));

        for rule in per_conn_rules {
            merged.push(rule);
        }

        // Compile enabled rules; skip disabled or invalid-regex ones.
        let mut compiled = Vec::new();
        for rule in &merged {
            if !rule.enabled {
                continue;
            }
            match Regex::new(&rule.pattern) {
                Ok(regex) => {
                    compiled.push(CompiledRule {
                        regex,
                        name: rule.name.clone(),
                        pattern: rule.pattern.clone(),
                        foreground_color: rule.foreground_color.clone(),
                        background_color: rule.background_color.clone(),
                    });
                }
                Err(e) => {
                    warn!(
                        rule_name = %rule.name,
                        pattern = %rule.pattern,
                        "Skipping highlight rule with invalid regex: {e}"
                    );
                }
            }
        }

        // Build a RegexSet from the compiled patterns for fast initial filtering.
        let regex_set = RegexSet::new(compiled.iter().map(|r| r.pattern.as_str()))
            .unwrap_or_else(|_| RegexSet::empty());

        Self {
            rules: compiled,
            regex_set,
        }
    }

    /// Finds all highlight matches in the given `line`.
    ///
    /// Returns a [`Vec<HighlightMatch>`] sorted by start position.  When
    /// multiple rules match the same region the later rule in the compiled
    /// list wins (per-connection rules appear after globals).
    #[must_use]
    pub fn find_matches(&self, line: &str) -> Vec<HighlightMatch> {
        let mut matches = Vec::new();
        // Use RegexSet to quickly determine which rules match this line,
        // then only run the individual regexes for those rules.
        for idx in self.regex_set.matches(line) {
            let rule = &self.rules[idx];
            for m in rule.regex.find_iter(line) {
                matches.push(HighlightMatch {
                    start: m.start(),
                    end: m.end(),
                    foreground_color: rule.foreground_color.clone(),
                    background_color: rule.background_color.clone(),
                });
            }
        }
        matches.sort_by_key(|m| m.start);
        matches
    }

    /// Returns the source pattern strings and names of all compiled rules.
    ///
    /// Useful for registering patterns with external regex engines (e.g. VTE
    /// PCRE2) that cannot reuse the Rust [`Regex`] objects directly.
    #[must_use]
    pub fn source_patterns(&self) -> Vec<SourcePattern<'_>> {
        self.rules
            .iter()
            .map(|r| SourcePattern {
                name: &r.name,
                pattern: &r.pattern,
            })
            .collect()
    }
}

/// A borrowed view of a compiled rule's name and regex pattern string.
#[derive(Debug)]
pub struct SourcePattern<'a> {
    /// Human-readable rule name.
    pub name: &'a str,
    /// The regex pattern string.
    pub pattern: &'a str,
}

// ---------------------------------------------------------------------------
// Built-in default rules
// ---------------------------------------------------------------------------

/// Returns the built-in default highlight rules.
///
/// - `ERROR`    — red foreground
/// - `WARNING`  — yellow foreground
/// - `CRITICAL` — red background
/// - `FATAL`    — red background
#[must_use]
pub fn builtin_defaults() -> Vec<HighlightRule> {
    // Deterministic UUIDs so the defaults are stable across restarts and can
    // be overridden by per-connection rules with the same id.
    let error_id = Uuid::from_bytes([
        0xBD, 0x01, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01,
    ]);
    let warning_id = Uuid::from_bytes([
        0xBD, 0x01, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02,
    ]);
    let critical_id = Uuid::from_bytes([
        0xBD, 0x01, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x03,
    ]);
    let fatal_id = Uuid::from_bytes([
        0xBD, 0x01, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x04,
    ]);

    vec![
        HighlightRule {
            id: error_id,
            name: "ERROR".to_string(),
            pattern: r"(?i)\bERROR\b".to_string(),
            foreground_color: Some("#FF0000".to_string()),
            background_color: None,
            enabled: true,
        },
        HighlightRule {
            id: warning_id,
            name: "WARNING".to_string(),
            pattern: r"(?i)\bWARNING\b".to_string(),
            foreground_color: Some("#FFFF00".to_string()),
            background_color: None,
            enabled: true,
        },
        HighlightRule {
            id: critical_id,
            name: "CRITICAL".to_string(),
            pattern: r"(?i)\bCRITICAL\b".to_string(),
            foreground_color: None,
            background_color: Some("#FF0000".to_string()),
            enabled: true,
        },
        HighlightRule {
            id: fatal_id,
            name: "FATAL".to_string(),
            pattern: r"(?i)\bFATAL\b".to_string(),
            foreground_color: None,
            background_color: Some("#FF0000".to_string()),
            enabled: true,
        },
    ]
}
