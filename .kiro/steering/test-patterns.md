---
inclusion: fileMatch
fileMatchPattern: "rustconn-core/tests/**/*.rs"
---

# Test File Patterns

You are editing a test file in `rustconn-core/tests/`.

## Property tests (`tests/properties/`)
- Use `proptest` 1.10 with `proptest!` macro
- New modules MUST be registered in `tests/properties/mod.rs`
- Entry point: `tests/property_tests.rs` (carries `#![allow(...)]` blocks)
- Temp files: always use `tempfile` crate
- Full suite runs ~2 min (argon2 tests are slow in debug) — this is normal

## Integration tests (`tests/integration/`)
- New modules MUST be registered in `tests/integration/mod.rs`
- Entry point: `tests/integration_tests.rs`

## Fixtures (`tests/fixtures/`)
- Shared test data and helpers

## Key rules
- No `unwrap()`/`expect()` except provably impossible states
- No GUI imports (`gtk4`, `adw`, `vte4`) — this is the core crate
- Use `SecretString` for any credential test data
