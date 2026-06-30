# Design Document

Release 0.17.5 — Design.

## Overview

The release closes the root causes of three issues (#194, #191, #197) and adds a native arm64 snap
build. #194/#191 are SSH auto-login (related), #197 is keyboard accelerators, #4 is packaging.

## Architecture

- SSH auto-login (#194, #191) — `rustconn/src/window/protocols_ssh.rs` (GUI layer), backed by
  `rustconn/src/terminal/mod.rs` (VTE) and `rustconn/src/state/mod.rs`
  (`resolve_credentials_blocking`). Command building — `build_ssh_command_args`; ssh helpers —
  `rustconn-core/src/ssh_tunnel.rs`.
- Keys (#197) — registry in `rustconn-core/src/config/keybindings.rs`, applied in `rustconn/src/app.rs`
  (`apply_keybindings`, `set_passthrough`, `install_layout_independent_accels`).
- Snap (#4) — `snap/snapcraft.yaml`, `.github/workflows/release.yml`, `snap.yml`, `docs/`.

Crate boundary: all GUI edits stay in `rustconn/`. `rustconn-core` changes only for:
(a) the pure prompt-detector function (no gtk/vte) and (b) the new settings field. VTE/adw do not
enter core.

## Components and Interfaces

### #194 — Cursor-position-based prompt detection

Problem: `detect_password_prompt` (`protocols_ssh.rs:25`) analyses `.lines().last()` of the full grid
from `get_terminal_text` (`terminal/mod.rs:2536`, rows `0..row_count`). Below the prompt there are
~20 blank rows → `last()` is empty → a valid `Password:` is not matched. On network gear the prompt
is in no-echo with cursor escapes and no `\n`, and `cursor-moved` fires before the glyphs land.

Solution:
1. Pure function in `rustconn-core`: `looks_like_password_prompt(line: &str) -> bool` (move the
   matching logic; no gtk/vte; testable).
2. `terminal/mod.rs` + `TerminalNotebook`: `get_cursor_line_text(session_id) -> Option<String>`
   (the line under the cursor via `get_cursor_position` + `text_range_format`), fallback — the last
   non-empty grid line.
3. `detect_password_prompt` → a thin adapter (cursor line, passphrase cutoff, delegate to core).
4. Idle re-check: a `glib::timeout_add_local_once(~120ms)` re-run of `check_and_inject` when there is
   no match, guarded against double-scheduling; the one-shot `password_sent` is kept.

Flow:
```
spawn ssh → connect_contents_changed + connect_cursor_moved → check_and_inject:
    line = cursor_line_text (fallback: last non-empty grid line)
    if !passphrase && core::looks_like_password_prompt(line):
        send_text(password + "\n"); password_sent = true
    else if !scheduled:
        timeout_once(120ms) → check_and_inject; scheduled = true
```

### #191 — Full bastion password resolution + guard

Problem: in `build_ssh_command_args` (`protocols_ssh.rs:249-301`) the bastion password is resolved
only via cache + `generate_store_key`+vault `Retrieve`. This misses the bastion's
`PasswordSource::Variable` → fallback to `-J` → VTE sends the target's password into the bastion
prompt. The VTE auto-fill has no "bastion vs target" guard.

Solution:
1. A shared connection-password resolver (Vault/Variable/cache) in the style of
   `resolve_credentials_blocking`, applied to `jump_conn`.
2. Guard in VTE auto-fill: a `bastion_handled_out_of_band`/`has_jump_host` flag → the target password
   is injected only when `!has_jump_host || bastion_handled_out_of_band`.
3. string+ref combo: determine the "first real hop" independently of an added string proxy
   (remove the dependency on `is_first_hop = jump_hosts.is_empty()`).

### #197 — Focus-based auto-suspend of single-Ctrl accelerators

Problem: `apply_keybindings` (`app.rs:1072`) registers accelerators at the `adw::Application` level →
the window capture phase intercepts chords before the focused VTE. Conflicting actions: `win.search`,
`win.command-palette`, `win.new-connection`, `win.close-tab`, `win.show-history`,
`win.move-to-group`, `win.import`.

Solution:
1. A constant list of "terminal" actions + `suspend_terminal_accels(app)` /
   `restore_terminal_accels(app, state)` (modeled on `set_passthrough`).
2. A `gtk4::EventControllerFocus` on the VTE (`terminal/mod.rs`): enter→suspend, leave→restore;
   the same on the embedded RDP/VNC/SPICE viewer containers.
3. A field `AppSettings.ui.terminal_passthrough_ctrl: bool` (default `true`) — when `false`, the
   controller does not suspend accelerators.
4. A toggle in the settings tab, text in `i18n()`; reconcile with the Keybindings tab's `suspend_accels`.

### #4 — Native parallel snap arm64

`snap/snapcraft.yaml`:
```yaml
platforms:
  amd64: { build-on: [amd64], build-for: [amd64] }
  arm64: { build-on: [arm64], build-for: [arm64] }
prime:
  - -usr/lib/*/libgtk-4.so*          # was usr/lib/x86_64-linux-gnu/...
  - -usr/lib/*/libgdk_pixbuf-2.0.so*
  # ...the rest of the entries likewise on wildcard
version: '0.17.5'
```

`release.yml` `build-snap` job:
```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - { runner: ubuntu-24.04,     arch: amd64 }
      - { runner: ubuntu-24.04-arm, arch: arm64 }
runs-on: ${{ matrix.runner }}
# artifact: snap-package-${{ matrix.arch }}; publish: snapcraft upload --release candidate
```

## Data Models

- `AppSettings.ui.terminal_passthrough_ctrl: bool` (default `true`) — a new field in `rustconn-core`,
  serde with `#[serde(default = ...)]` for backward compatibility with old configs.
- Auto-fill flow flag (#191): `bastion_handled_out_of_band: bool` / `has_jump_host: bool` —
  passed into the `check_and_inject` closure (not a persistent model, local session state).
- No new persistent models for #194/#4.

## Correctness Properties

### Property 1: prompt detection (#194)
`looks_like_password_prompt` is true for all supported localized prompts and
`pass:`/trailing spaces/no-trailing-space; false for `passphrase for key`.

**Validates: Requirements 1.1, 1.5**

### Property 2: injection idempotence (#194)
The password is injected exactly once per session (`password_sent` monotonically becomes true; the
idle timer is not scheduled twice).

**Validates: Requirements 1.3, 1.4**

### Property 3: guard invariant (#191)
The target password is NOT sent to the VTE until the bastion is handled out-of-band (or while there
is no jump host at all).

**Validates: Requirements 2.2, 2.5**

### Property 4: accelerator determinism (#197)
suspend/restore are idempotent; after any sequence of focus events the set of active accelerators is
deterministic (terminal focused → suspended; otherwise → from settings).

**Validates: Requirements 3.1, 3.2**

## Error Handling

- Bastion Vault/Variable resolution: errors are logged (`tracing`, no secret), fallback — bastion
  without an out-of-band password (the guard prevents leaking the target password).
- `get_cursor_line_text`: `None` → fallback to the grid; never panics.
- Idle timer: one-shot, no accumulation; removed together with the session.
- snap arm: best-effort — `fail-fast: false` + `continue-on-error` on publish; an arm failure does not
  break amd64/the release.
- All errors via `Result`/`Option`, no `unwrap`/`expect` on working paths (M-PANIC-ON-BUG).

## Testing Strategy

- `rustconn-core`: unit/property tests for `looks_like_password_prompt` (localizations, `pass:`,
  trailing spaces, no-trailing, passphrase negative).
- `rustconn-core`/`rustconn`: a test for bastion password resolution from the Variable source.
- #197: a unit test for building the suspended-accelerator list; UI focus — manual check.
- Manual checks: OLT Variable password (exact prompt line from the requester); bastion Variable + target
  Vault and vice versa; prompt-only bastion (target password not leaked); snap arm64 `workflow_dispatch`
  dry-run.
- At the end of the feature — `rust-quality-check` (fmt+clippy+tests).
