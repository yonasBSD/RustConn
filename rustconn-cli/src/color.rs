//! Terminal color support with `NO_COLOR` / `--no-color` awareness.
//!
//! Colors are disabled when:
//! - `--no-color` flag is passed (or `NO_COLOR` env var is set)
//! - stdout is not a terminal (piped output)

use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag set once at startup from `--no-color` / `NO_COLOR`.
static NO_COLOR: AtomicBool = AtomicBool::new(false);

/// Call once from `main()` to propagate the `--no-color` flag.
pub fn init(no_color: bool) {
    NO_COLOR.store(no_color, Ordering::Relaxed);
}

/// Returns `true` when colored output should be used.
#[must_use]
pub fn enabled() -> bool {
    !NO_COLOR.load(Ordering::Relaxed) && std::io::stdout().is_terminal()
}

// ANSI escape sequences — return empty strings when color is disabled.

#[must_use]
pub fn green() -> &'static str {
    if enabled() { "\x1b[32m" } else { "" }
}

#[must_use]
pub fn red() -> &'static str {
    if enabled() { "\x1b[31m" } else { "" }
}

#[must_use]
pub fn yellow() -> &'static str {
    if enabled() { "\x1b[33m" } else { "" }
}

#[must_use]
pub fn cyan() -> &'static str {
    if enabled() { "\x1b[36m" } else { "" }
}

#[must_use]
pub fn bold() -> &'static str {
    if enabled() { "\x1b[1m" } else { "" }
}

#[must_use]
pub fn reset() -> &'static str {
    if enabled() { "\x1b[0m" } else { "" }
}
