# Requirements Document

Release 0.17.5 — Requirements.

## Introduction

Release 0.17.5 finishes three SSH/UX issues and extends snap packaging to arm64.
Important context: #194 and #191 already had fixes (in 0.17.4 and 0.17.2 respectively), but the
issues remain OPEN — the previous fixes were incomplete. So this is root-cause follow-up work,
not new features.

Implementation order: **#194 → #191 → #197 → snap arm64** (the first two are related — SSH auto-login).

Out of scope for this release:
- Auto-filling the username on devices that prompt `Username:`/`login:` (#194).
- Full interactive support for prompt-only bastions (#191).
- Reviving arm64 deb/rpm/appimage builds (only snap arm in this release).
- Multi-hop bastions with a different password per hop (remains future work).

## Glossary

- **VTE** — the embedded terminal widget (`rustconn/src/terminal/`).
- **Auto-fill** — injecting the password into the terminal when a `password:` prompt is detected.
- **Bastion / jump host** — an intermediate SSH host used to reach the target.
- **Single-Ctrl chord** — a `<Control>` + one key combination without `<Shift>`.
- **out-of-band** — delivering the bastion password via `SSH_ASKPASS`, not through the VTE prompt.

## Requirements

### Requirement 1: Variable password performs auto-login on network equipment (#194)

**User Story:** As a user whose password comes from a Variable source, I want RustConn to
automatically fill the password into the SSH prompt of a network device (OLT/router), so that
I do not have to type it manually.

#### Acceptance Criteria

1. WHEN an SSH session receives a password prompt that ends on the cursor line without a trailing
   `\n` (no-echo, cursor-positioning escape sequences), THE SYSTEM SHALL detect the prompt and
   inject the cached password exactly once.
2. WHEN the terminal grid contains trailing blank rows below the prompt, THE SYSTEM SHALL determine
   the prompt by cursor position (the line under the cursor / the text up to the cursor) rather than
   solely by `.lines().last()` of the full grid.
3. IF the `cursor-moved`/`contents-changed` signal fired before the prompt glyphs were committed to
   the grid, THEN THE SYSTEM SHALL perform a deferred re-check (idle re-check) so the prompt is not
   missed due to the race.
4. THE SYSTEM SHALL inject the password exactly once per session (the one-shot guard is kept to avoid
   locking the account with repeated attempts).
5. THE SYSTEM SHALL NOT inject the password into a key-passphrase prompt (`passphrase for key`) —
   the current behavior is preserved.
6. THE SYSTEM SHALL emit structured logs for the detection and injection events (without the secret value).
7. Auto-filling the **username** (`Username:`/`login:`) is out of scope for this release.

### Requirement 2: Jump host authenticates with its own password, Variable/Vault (#191)

**User Story:** As a user connecting through a bastion whose login/password differs from the target,
I want the bastion to authenticate with ITS OWN password, and the target's password to never reach
the bastion prompt.

#### Acceptance Criteria

1. WHEN the bastion (first hop) has a stored password from the Vault OR Variable source, THE SYSTEM
   SHALL resolve it via the same path as the target's password (honoring `PasswordSource`) and deliver
   it to the bastion out-of-band via `SSH_ASKPASS` on the ProxyCommand.
2. THE SYSTEM SHALL inject the target's password into the VTE prompt only when the bastion is handled
   out-of-band OR when there is no jump host (guard against leaking the target password to the bastion).
3. WHEN a connection has both a string `proxy_jump` and a reference `jump_host_id`, THE SYSTEM SHALL
   correctly resolve the first hop's password (not skip the block due to `is_first_hop == false`).
4. IF the bastion uses key authentication (no password needed), THEN THE SYSTEM SHALL work as before
   (askpass is not involved).
5. IF the bastion is interactive-prompt-only with no stored password, THEN THE SYSTEM SHALL NOT leak
   the target's password to the bastion (full interactive prompt-only bastion support is out of scope
   for this release).
6. THE SYSTEM SHALL preserve the existing single-bastion-with-Vault-password behavior (no regressions).

### Requirement 3: Terminal Ctrl chords reach the session (#197)

**User Story:** As a heavy terminal user, I want readline chords (Ctrl+F/P/N and relatives) to reach
the shell instead of being intercepted by the application.

#### Acceptance Criteria

1. WHEN the VTE terminal OR an embedded viewer (RDP/VNC/SPICE) has focus, THE SYSTEM SHALL temporarily
   suspend the single-Ctrl accelerators that collide with the terminal: `win.search` (Ctrl+F),
   `win.command-palette` (Ctrl+P), `win.new-connection` (Ctrl+N), `win.close-tab` (Ctrl+W),
   `win.show-history` (Ctrl+H), `win.move-to-group` (Ctrl+M), `win.import` (Ctrl+I).
2. WHEN focus leaves the terminal/viewer (to the sidebar, a dialog, an input field), THE SYSTEM SHALL
   restore the accelerators from settings.
3. THE SYSTEM SHALL keep `<Control><Shift>` chords and function keys active while the terminal is
   focused (copy=Ctrl+Shift+C, paste=Ctrl+Shift+V, terminal-search=Ctrl+Shift+F, etc.).
4. THE SYSTEM SHALL provide a setting "Send terminal control shortcuts to the session", enabled
   **by default**; when disabled the behavior is the old one (accelerators always active).
5. THE SYSTEM SHALL stay compatible with the global passthrough (`win.toggle-passthrough`) and the
   layout-independent controller (both read accelerators live).
6. THE SYSTEM SHALL wrap the new setting text in `i18n()` and update the POT/translations.

### Requirement 4: Native parallel snap build for arm64 (snap arm64)

**User Story:** As an arm64 (aarch64) user, I want to install RustConn from the Snap Store so I can
use the application on ARM hardware.

#### Acceptance Criteria

1. THE SYSTEM SHALL declare the `arm64` platform alongside `amd64` in `snap/snapcraft.yaml`.
2. THE SYSTEM SHALL make the `prime:` exclusion paths architecture-independent (wildcard
   `usr/lib/*/...` instead of the hardcoded `usr/lib/x86_64-linux-gnu/...`) so the GTK/GLib stack
   deduplication works on arm64 too (otherwise a transparent window / broken icons).
3. THE SYSTEM SHALL build the snap for amd64 and arm64 **in parallel** via a matrix in the
   `release.yml` `build-snap` job: amd64 on `ubuntu-24.04`, arm64 on `ubuntu-24.04-arm` (native, no QEMU).
4. THE SYSTEM SHALL upload both snaps to the `candidate` channel and attach them to the GitHub release,
   with unique per-arch artifact names.
5. THE SYSTEM SHALL keep the snap build best-effort (`fail-fast: false`, the existing `continue-on-error`
   on publish) — a failure of any architecture SHALL NOT gate the release.
6. THE SYSTEM SHALL keep `version` in `snapcraft.yaml` in sync with the release version.
7. Documentation (`docs/CI_BUILD_FLOW.md`) SHALL reflect the dual-architecture snap flow.
