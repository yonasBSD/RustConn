---
inclusion: fileMatch
fileMatchPattern: "rustconn/src/window/**/*.rs"
---

# Window / Sessions — Development Rules

You are editing a file in `rustconn/src/window/`.

## State Management

- `SharedAppState = Rc<RefCell<AppState>>` — pass as `&SharedAppState`
- NEVER hold a borrow across async boundaries or GTK callbacks
- Use `with_state()` / `with_state_mut()` helpers instead of direct `.borrow()`
- For callbacks with RefCell → take-invoke-restore pattern (as in `handle_ironrdp_error`)

## Sidebar

- Statuses: yellow = connecting, green = connected, red = failed, gray = disconnected
- Reconnect → reuse existing tab (don't create a new one)
- Context menu → GNOME HIG order: primary action at top, destructive at bottom

## Toasts

- `adw::ToastOverlay` with severity icons
- Use `i18n_f()` with `{}` placeholders for dynamic values

## Tabs

- Tab Overview → `AdwTabOverview`, terminals always inside `TabPage`
- Split view → layout lives inside TabPage, not in a global container

## Auto-Reconnect

- Uses `poll_until_online_with_backoff()` from `rustconn-core/src/host_check.rs`
- Exponential backoff via `RetryConfig` / `RetryState` (per-connection or default)
- Runs in background thread via `spawn_blocking_with_callback`
- Cancel token registered per session — closing tab cancels polling
- Never use `.expect()` for `Runtime::new()` — use `.map_err(HostCheckError::Io)?`
- Skip reconnect if: SSH auth failure, rapid crash (<5s), or retry disabled
