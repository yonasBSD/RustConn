---
inclusion: fileMatch
fileMatchPattern: "rustconn-core/**/*.rs"
---

# rustconn-core: GUI-Free Crate Rules

You are working in `rustconn-core` — the business logic library. Strict constraints apply:

## FORBIDDEN imports
- `gtk4`, `adw`, `vte4`, `libadwaita` — NEVER import these here
- If you need GTK widgets, the code belongs in `rustconn/` (the GUI crate)

## REQUIRED patterns
- All error types: `#[derive(Debug, thiserror::Error)]`
- All credentials: `secrecy::SecretString`, never plain `String`
- All public items: `///` doc comments (`#![warn(missing_docs)]` is enabled)
- All fallible functions: return `Result<T, Error>`, use domain-specific aliases (`ConfigResult<T>`, `ProtocolResult<T>`, etc.)
- All new public types: re-export through `lib.rs`
- Feature-gated types: use `#[cfg(feature = "...")]` on both definition and re-export

## Logging
- Use `tracing` with structured fields, never `println!`/`eprintln!`

## Testing
- Property tests go in `tests/properties/`, register in `properties/mod.rs`
- Integration tests go in `tests/integration/`, register in `integration/mod.rs`
- Use `tempfile` crate for temporary files, never hardcoded paths
