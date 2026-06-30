# Implementation Plan: Release 0.17.5

## Overview

Order: #194 → #191 → #197 → snap arm64. Each SSH fix is verified with `getDiagnostics`, and at the
end of a feature with `rust-quality-check`. #194 and #191 are related (SSH auto-login); #191 relies
on the resolver helper, so it comes after #194.

## Tasks

### #194 — Variable password / cursor-position prompt detection

- [x] 1.1 Extract a pure detector function into `rustconn-core` (`looks_like_password_prompt(&str) -> bool`),
  moving the matching logic out of `detect_password_prompt`; no gtk/vte.
  _Requirements: 1.1, 1.5_
- [x] 1.2 Add `get_cursor_line_text(session_id)` in `terminal/mod.rs` + `TerminalNotebook`
  (line under the cursor via `get_cursor_position` + `text_range_format`), fallback to the last
  non-empty grid line.
  _Requirements: 1.2_
- [x] 1.3 Rewrite `detect_password_prompt` as a thin adapter (cursor line → core), update both
  auto-fill sites (initial-connect ~789-891, reconnect ~1164-1243).
  _Requirements: 1.1, 1.2_
- [x] 1.4 Add an idle re-check (`timeout_add_local_once(~120ms)`) in `check_and_inject` with a guard
  against re-scheduling; keep the one-shot `password_sent`.
  _Requirements: 1.3, 1.4_
- [x] 1.5 `rustconn-core` tests for `looks_like_password_prompt` (localizations, `pass:`, trailing
  spaces, no-trailing, passphrase negative).
  _Requirements: 1.1, 1.5_
- [x] 1.6 `getDiagnostics` on changed files; structured log without the secret.
  _Requirements: 1.6_

### #191 — Bastion password (Variable/Vault) + guard

- [x] 2.1 A shared connection-password resolver (Vault/Variable/cache) in the style of
  `resolve_credentials_blocking`; apply it to the bastion in `build_ssh_command_args` (replacing the
  narrow vault lookup, lines 249-301).
  _Requirements: 2.1, 2.6_
- [x] 2.2 Fix the string+ref combo: resolve the first reference hop's password independently of an
  added string `proxy_jump` (remove the dependency on `is_first_hop = jump_hosts.is_empty()`).
  _Requirements: 2.3_
- [x] 2.3 Add a guard to VTE auto-fill: inject the target password only if
  `!has_jump_host || bastion_handled_out_of_band`. Thread the flag into both auto-fill sites.
  _Requirements: 2.2, 2.5_
- [x] 2.4 Verify the key-auth bastion behavior is preserved (askpass not involved) and single-bastion
  Vault has no regressions.
  _Requirements: 2.4, 2.6_
- [x] 2.5 A resolution/ProxyCommand test for a Variable bastion; `getDiagnostics`.
  _Requirements: 2.1, 2.3_

### #197 — Focus-based auto-suspend of single-Ctrl accelerators

- [x] 3.1 Add a field `AppSettings.ui.terminal_passthrough_ctrl: bool` (default `true`) in
  `rustconn-core` (serde `#[serde(default)]`).
  _Requirements: 3.4_
- [x] 3.2 In `app.rs` add a constant list of "terminal" actions and the functions
  `suspend_terminal_accels` / `restore_terminal_accels` (modeled on `set_passthrough`).
  _Requirements: 3.1, 3.2_
- [x] 3.3 Add `EventControllerFocus` on the VTE (enter→suspend, leave→restore), honoring
  `terminal_passthrough_ctrl`; access to `app`+`state` via weak/clone.
  _Requirements: 3.1, 3.2, 3.4, 3.5_
- [x] 3.4 Extend the same focus controller to the embedded RDP/VNC/SPICE viewer containers.
  _Requirements: 3.1_
- [x] 3.5 The toggle "Send terminal control shortcuts to the session" in the settings tab, text in
  `i18n()`; reconcile with the Keybindings tab's `suspend_accels`.
  _Requirements: 3.4, 3.6_
- [x] 3.6 `bash po/update-pot.sh` + msgmerge for the 16 languages; `getDiagnostics`.
  _Requirements: 3.6_

### #4 — Native parallel snap arm64

- [x] 4.1 `snap/snapcraft.yaml`: add the `arm64` platform; `prime:` exclusions → wildcard
  `usr/lib/*/...`; sync `version`.
  _Requirements: 4.1, 4.2, 4.6_
- [x] 4.2 `release.yml` `build-snap` job: matrix `[ubuntu-24.04/amd64, ubuntu-24.04-arm/arm64]`,
  `fail-fast: false`, unique per-arch artifact names.
  _Requirements: 4.3, 4.4, 4.5_
- [x] 4.3 `snap.yml`: mirror the matrix for the `workflow_dispatch` dry-run.
  _Requirements: 4.3_
- [x] 4.4 `workflow_dispatch` dry-run of `snap.yml` to verify LXD on the arm runner; if needed —
  `--destructive-mode`.
  _Requirements: 4.3_
- [x] 4.5 Update `docs/CI_BUILD_FLOW.md` (dual-architecture snap flow).
  _Requirements: 4.7_

### Release wrap-up

- [x] 5.1 CHANGELOG: a `## [0.17.5]` entry with links to #194/#191/#197 and snap arm64.
- [x] 5.2 Bump the workspace version (`Cargo.toml` `0.17.4 → 0.17.5`) and derivatives (snapcraft, metainfo).
- [x] 5.3 Final `rust-quality-check` (fmt+clippy+tests) before tagging.

## Task Dependency Graph

```json
{
  "waves": [
    { "wave": 1, "tasks": ["1.1", "1.2"], "depends_on": [] },
    { "wave": 2, "tasks": ["1.3"], "depends_on": ["1.1", "1.2"] },
    { "wave": 3, "tasks": ["1.4", "1.5"], "depends_on": ["1.3"] },
    { "wave": 4, "tasks": ["1.6"], "depends_on": ["1.4", "1.5"] },
    { "wave": 5, "tasks": ["2.1"], "depends_on": ["1.6"] },
    { "wave": 6, "tasks": ["2.2", "2.3"], "depends_on": ["2.1"] },
    { "wave": 7, "tasks": ["2.4", "2.5"], "depends_on": ["2.2", "2.3"] },
    { "wave": 8, "tasks": ["3.1"], "depends_on": [] },
    { "wave": 9, "tasks": ["3.2"], "depends_on": ["3.1"] },
    { "wave": 10, "tasks": ["3.3"], "depends_on": ["3.2"] },
    { "wave": 11, "tasks": ["3.4", "3.5"], "depends_on": ["3.3"] },
    { "wave": 12, "tasks": ["3.6"], "depends_on": ["3.5"] },
    { "wave": 13, "tasks": ["4.1"], "depends_on": [] },
    { "wave": 14, "tasks": ["4.2", "4.5"], "depends_on": ["4.1"] },
    { "wave": 15, "tasks": ["4.3"], "depends_on": ["4.2"] },
    { "wave": 16, "tasks": ["4.4"], "depends_on": ["4.3"] },
    { "wave": 17, "tasks": ["5.1"], "depends_on": ["1.6", "2.5", "3.6", "4.4", "4.5"] },
    { "wave": 18, "tasks": ["5.2"], "depends_on": ["5.1"] },
    { "wave": 19, "tasks": ["5.3"], "depends_on": ["5.2"] }
  ]
}
```

## Notes

- Crate boundary: GUI stays in `rustconn/`; in `rustconn-core` — only the pure detector function and
  the settings field (no gtk/vte/adw).
- Security: passwords are `SecretString`, intermediates are `Zeroizing`, logs carry no secrets.
- Do not trim rustconn-core tests; argon2 tests are slow (~120s) — use `timeout 180s`.
- snap arm: verify LXD on `ubuntu-24.04-arm` via a dry-run before tagging; fallback `--destructive-mode`.
