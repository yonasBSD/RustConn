# Tasks: Cloud Sync

> Derived from: `#[[file:.kiro/specs/cloud-sync/design.md]]` and `#[[file:.kiro/specs/cloud-sync/requirements.md]]`
> Primary reference: `#[[file:docs/CLOUD_SYNC_DESIGN.md]]`

## Phase 0: SSH Key Inheritance

- [x] 1. Phase 0: SSH Key Inheritance
  - [x] 1.1 Add `SshKeySource::Inherit` variant to `rustconn-core/src/models/protocol.rs`
  - [x] 1.2 Add SSH inheritance fields to `ConnectionGroup` in `rustconn-core/src/models/group.rs`: `ssh_auth_method: Option<SshAuthMethod>`, `ssh_key_path: Option<PathBuf>`, `ssh_proxy_jump: Option<String>`, `ssh_agent_socket: Option<String>`
  - [x] 1.3 Add i18n keys for new fields in `po/rustconn.pot`: "SSH Authentication Method", "SSH Key Path", "SSH Proxy Jump", "SSH Agent Socket", "Inherit from group", "Inherited from parent group"
  - [x] 1.4 Create `rustconn-core/src/connection/ssh_inheritance.rs` with functions: `resolve_ssh_key_path()`, `resolve_ssh_auth_method()`, `resolve_ssh_proxy_jump()`, `resolve_ssh_agent_socket()`
  - [x] 1.5 Integrate SSH inheritance into `rustconn-core/src/sftp.rs` (`build_sftp_command`, `get_ssh_key_path`)
  - [x] 1.6 Integrate SSH inheritance into `rustconn/src/window/protocols.rs` (SSH command building)
  - [x] 1.7 Integrate SSH inheritance into `rustconn/src/session/` (session start flow)
  - [x] 1.8 Write unit tests for inheritance chain resolution (3+ nesting levels, missing parent, no key in chain)
  - [x] 1.9 Write `proptest` property tests for inheritance chain resolution (arbitrary hierarchies, cycle detection, termination)
  - [x] 1.10 Extend group edit dialog: SSH Auth Method dropdown, SSH Key Path file chooser, Proxy Jump text field, Agent Socket text field
  - [x] 1.11 Add i18n keys for group SSH settings UI elements
  - [x] 1.12 Extend connection edit dialog: add "Inherit" option in SSH Key Source dropdown with tooltip showing resolved value
  - [x] 1.13 Add CLI support: `rustconn-cli group show <name>` displays SSH inheritance fields
  - [x] 1.14 Add CLI support: `rustconn-cli group edit <name> --ssh-key-path <path>`

## Phase 1: Sync Models and Settings

- [x] 2. Phase 1: Sync Models and Settings
  - [x] 2.1 Create `rustconn-core/src/sync/mod.rs` with pub exports
  - [x] 2.2 Create `rustconn-core/src/sync/settings.rs`: `SyncSettings` struct, `SyncMode` enum (None/Master/Import), Default implementations
  - [x] 2.3 Add `sync: SyncSettings` field to `AppSettings` in `rustconn-core/src/config/settings.rs`
  - [x] 2.4 Add `sync_mode`, `sync_file`, `last_synced_at` fields to `ConnectionGroup`
  - [x] 2.5 Create `rustconn-core/src/sync/variable_template.rs`: `VariableTemplate` struct
  - [x] 2.6 Add `sync` module to `rustconn-core/src/lib.rs`
  - [x] 2.7 Add i18n keys: "Cloud Sync", "Sync Directory", "Device Name", "Master", "Import", "Not synced", "Sync Mode", "Sync File", "Last Synced"
  - [x] 2.8 Create `rustconn-core/src/sync/group_export.rs`: `GroupSyncExport`, `SyncGroup`, `SyncConnection` structs with `to_file()`/`from_file()` (atomic writes) and `from_group_tree()`
  - [x] 2.9 Implement Connection → SyncConnection conversion (filter local-only fields)
  - [x] 2.10 Implement ConnectionGroup → SyncGroup conversion (path-based hierarchy)
  - [x] 2.11 Implement VariableTemplate collection from referenced variables
  - [x] 2.12 Write unit tests for serialization round-trip (GroupSyncExport)
  - [x] 2.13 Write unit tests for local-only field filtering
  - [x] 2.14 Add `slug` crate to `rustconn-core/Cargo.toml`
  - [x] 2.15 Implement `group_name_to_filename(name) -> String`
  - [x] 2.16 Write tests for slug generation: unicode, special chars, consecutive hyphens, determinism

## Phase 2: Group Sync Export (Master)

- [x] 3. Phase 2: Group Sync Export (Master)
  - [x] 3.1 Create `SyncManager` in `rustconn-core/src/sync/manager.rs`
  - [x] 3.2 Implement `export_group(group_id, conn_mgr)` — export single Master group
  - [x] 3.3 Implement `schedule_export(group_id)` — debounced (5s) via watch channel
  - [x] 3.4 Integrate export trigger into `ConnectionManager` persistence flow
  - [x] 3.5 Add `AdwComboRow` "Cloud Sync" (None/Master/Import) to group properties dialog
  - [x] 3.6 Add flat menu item "Enable Cloud Sync..." to group context menu (opens group properties)
  - [x] 3.7 Add flat menu item "Sync Now" to group context menu (visible only when sync enabled)
  - [x] 3.8 Implement `AdwAlertDialog` confirmation for Enable Master: heading "Enable Cloud Sync?", responses Cancel/Enable
  - [x] 3.9 Implement `AdwStatusPage` empty state when sync_dir not configured: icon `folder-remote-symbolic`, title "Set Up Cloud Sync", "Choose Directory" button
  - [x] 3.10 Implement sync_dir validation, sync_file generation, and first export flow
  - [x] 3.11 Add i18n keys: "Enable Cloud Sync...", "Sync Now", "Disable Sync", "Enable Cloud Sync?", "This group will be exported to %s.", "Cloud sync enabled for group '%s'", "Exported to cloud: %d connections", "Set Up Cloud Sync", "Choose a directory synced with your cloud service"

## Phase 3: Group Sync Import

- [x] 4. Phase 3: Group Sync Import
  - [x] 4.1 Create `GroupMergeEngine` in `rustconn-core/src/sync/group_merge.rs`
  - [x] 4.2 Implement merge connections by name (create/update/delete)
  - [x] 4.3 Implement merge groups by path (create/delete)
  - [x] 4.4 Implement variable templates: create missing locally
  - [x] 4.5 Implement preserve local-only fields on update
  - [x] 4.6 Write `proptest` property tests for GroupMergeEngine (completeness, determinism, local-only preservation)
  - [x] 4.7 Implement `import_all_on_start(conn_mgr)` for all Import groups
  - [x] 4.8 Implement `exported_at > last_synced_at` comparison for startup import
  - [x] 4.9 Integrate import into app startup flow
  - [x] 4.10 Implement `SyncReport` generation and logging
  - [x] 4.11 Implement `list_available_sync_files()` — list .rcn files in sync_dir
  - [x] 4.12 Add Settings → Cloud Sync → "Available files" section with "Import" button per file
  - [x] 4.13 Add toast: "Imported '%s': %d connections, %d groups" (i18n)
  - [x] 4.14 Implement Import group edit dialog: synced fields as read-only `AdwActionRow` (subtitle=value, description="Managed by cloud sync")
  - [x] 4.15 Implement Import group edit dialog: local fields as editable `AdwEntryRow`/`AdwComboRow`/`AdwSwitchRow`
  - [x] 4.16 Implement two `AdwPreferencesGroup` sections: "Synced Properties" (read-only) + "Local Settings" (editable)
  - [x] 4.17 Remove "New Connection", "New Subgroup", "Delete" from Import group context menu
  - [x] 4.18 Add flat "Sync Now" item to Import group context menu
  - [x] 4.19 Block drag-and-drop INTO Import groups
  - [x] 4.20 Add i18n keys: "Synced Properties", "Local Settings", "Managed by cloud sync", "Cannot add connections to synced group", "Cannot create subgroups in synced group"

## Phase 4: Credential Resolution UX

- [x] 5. Phase 4: Credential Resolution UX
  - [x] 5.1 Create `CredentialResolutionResult` enum in `rustconn-core/src/sync/credential_check.rs`
  - [x] 5.2 Modify `resolve_credentials_blocking` to return specific missing types instead of `None`
  - [x] 5.3 Create `rustconn/src/dialogs/variable_setup.rs`: `AdwAlertDialog` with heading, body, extra_child (`AdwPreferencesGroup` with `AdwPasswordEntryRow` + `AdwComboRow`), responses Cancel / Save & Connect
  - [x] 5.4 Integrate variable setup dialog into connection start flow: on `VariableMissing` → show dialog → save variable → retry connection
  - [x] 5.5 Add i18n keys: "Variable Not Configured", "Connection '%s' requires variable '%s'", "Value", "Store in", "Save & Connect"
  - [x] 5.6 Create `rustconn/src/dialogs/backend_missing.rs`: `AdwAlertDialog` with responses "Enter Password Manually" / "Open Settings"
  - [x] 5.7 Integrate backend missing dialog into credential resolution flow
  - [x] 5.8 Add i18n keys: "Secret Backend Not Configured", "This connection stores credentials in a secret vault, but no backend is set up yet.", "Enter Password Manually", "Open Settings"

## Phase 5: File Watcher + Settings UI

- [x] 6. Phase 5: File Watcher + Settings UI
  - [x] 6.1 Add `notify = "7"` to `rustconn-core/Cargo.toml`
  - [x] 6.2 Create `SyncFileWatcher` in `rustconn-core/src/sync/watcher.rs` with 3s debounce and Master group filtering
  - [x] 6.3 Integrate file watcher: auto-import on Import group file change
  - [x] 6.4 Create `AdwPreferencesPage` "Cloud Sync" in settings dialog with icon `emblem-synchronizing-symbolic`
  - [x] 6.5 Add `AdwPreferencesGroup` "Setup": `AdwEntryRow` "Sync Directory" (file chooser suffix), `AdwEntryRow` "Device Name"
  - [x] 6.6 Add `AdwPreferencesGroup` "Synced Groups": `AdwActionRow` per group with subtitle "Master · synced" / "Import · synced"
  - [x] 6.7 Add `AdwPreferencesGroup` "Available in Cloud": description + `AdwActionRow` per .rcn file with "Import" suffix button
  - [x] 6.8 Add `AdwPreferencesGroup` "Simple Sync": `AdwSwitchRow` "Sync everything between your devices"
  - [x] 6.9 Add sidebar sync indicators: `emblem-synchronizing-symbolic` (synced), `dialog-warning-symbolic` (error), tooltips with last sync time
  - [x] 6.10 Add i18n keys for all Settings UI elements: "Cloud Sync", "Setup", "Synced Groups", "Available in Cloud", "Simple Sync", "Sync Directory", "Device Name", "Master · synced", "Import · synced", "Sync error", "Files in sync directory not yet imported", "Sync everything between your devices", "No sync directory configured", "Master — last exported: %s", "Import — last synced: %s", "Sync error: %s"

## Phase 6: CLI Support

- [x] 7. Phase 6: CLI Support
  - [x] 7.1 Implement `rustconn-cli sync status`
  - [x] 7.2 Implement `rustconn-cli sync list`
  - [x] 7.3 Implement `rustconn-cli sync export <group>`
  - [x] 7.4 Implement `rustconn-cli sync import <file>`
  - [x] 7.5 Implement `rustconn-cli sync now`

## Phase 7: Simple Sync

- [x] 8. Phase 7: Simple Sync
  - [x] 8.1 Create `FullSyncExport` in `rustconn-core/src/sync/full_export.rs`
  - [x] 8.2 Create `Tombstone` model in `rustconn-core/src/sync/tombstone.rs`
  - [x] 8.3 Implement secret variable and local-only field filtering for FullSyncExport
  - [x] 8.4 Create `FullMergeEngine` in `rustconn-core/src/sync/full_merge.rs` with UUID-based merge and `updated_at` comparison
  - [x] 8.5 Implement tombstone processing and cleanup (retention_days)
  - [x] 8.6 Write `proptest` property tests for bidirectional merge (convergence, tombstone consistency)
  - [x] 8.7 Implement `enable_simple_sync()` — auto Master all root groups + extras file
  - [x] 8.8 Implement `import_simple_sync()` — UUID merge + tombstones
  - [x] 8.9 Implement auto-export/import triggers with `device_id` check
  - [x] 8.10 Add Simple Sync toggle in Settings with warning
  - [x] 8.11 Add Simple Sync status indicator
  - [x] 8.12 Add i18n keys: "Enable Simple Sync (sync everything between your devices)", "Simple Sync active — last sync: %s"

## Phase 8: Documentation + Polish + Backlog

- [x] 9. Phase 8: Documentation + Polish + Backlog
  - [x] 9.1 Update `docs/USER_GUIDE.md` with Cloud Sync section
  - [x] 9.2 Update `CHANGELOG.md`
  - [x] 9.3 Update `docs/CLI_REFERENCE.md` with sync commands
  - [x] 9.4 Run `po/update-pot.sh` to update translation template
  - [x] 9.5 Handle edge case: corrupt sync file → graceful error + toast
  - [x] 9.6 Handle edge case: sync_dir deleted during runtime → disable watcher + toast
  - [x] 9.7 Handle edge case: concurrent export from two Master instances → last-write-wins
  - [x] 9.8 Handle edge case: group with 0 connections → export empty valid file
  - [x] 9.9 Handle edge case: import group then disable sync → group becomes regular (editable)
  - [x] 9.10 Verify all i18n keys are present and correct
  - [x] 9.11 Backlog: add accessible labels to icon-only buttons (help_button, filter_toggle, quick_actions_button, password visibility toggles, password load buttons)
  - [x] 9.12 Backlog: audit all `Button::from_icon_name` in `rustconn/src/` for accessible labels
  - [x] 9.13 Backlog: add all new accessible label strings to `po/rustconn.pot`
  - [x] 9.14 Backlog: create `deny.toml` with advisories/licenses/bans/sources sections
  - [x] 9.15 Backlog: add `cargo-deny` job to `.github/workflows/ci.yml`
  - [x] 9.16 Backlog: add `cargo-audit` job to CI
  - [x] 9.17 Backlog: replace text `"• "` prefix with CSS dot indicator (`.document-dirty`) in sidebar
  - [x] 9.18 Backlog: add CSS for `.document-dirty` in `rustconn/assets/style.css`
  - [x] 9.19 Backlog: add accessible label and tooltip for document dirty badge

## Phase 9: Version Bump and Release

- [ ] 10. Phase 9: Version Bump to 0.12.0 and Release Metadata
  - [~] 10.1 Bump version to `0.12.0` in `Cargo.toml` workspace `[workspace.package]` section
  - [~] 10.2 Update `CHANGELOG.md` — add `## [0.12.0]` section with Cloud Sync feature summary (Group Sync, Simple Sync, SSH Key Inheritance, Credential Resolution UX, accessible labels, cargo-deny, document badge)
  - [~] 10.3 Update `rustconn/assets/io.github.totoshko88.RustConn.metainfo.xml` — add `<release version="0.12.0" date="YYYY-MM-DD">` entry with feature description
  - [~] 10.4 Update Flathub manifest `packaging/flathub/io.github.totoshko88.RustConn.yml` — update git tag from `v0.11.7` to `v0.12.0`
  - [~] 10.5 Regenerate Flathub cargo sources: `packaging/flathub/cargo-sources.json` (via `flatpak-cargo-generator.py`)
  - [~] 10.6 Update OBS `packaging/obs/_service` — change `<param name="revision">` from `v0.11.7` to `v0.12.0`
  - [~] 10.7 Update OBS `packaging/obs/rustconn.changes` — add new changelog entry for 0.12.0
  - [~] 10.8 Update OBS `packaging/obs/debian.dsc` — change `Version:` from `0.11.7-1` to `0.12.0-1` and `DEBTRANSFORM-TAR` filename
  - [~] 10.9 Update OBS `packaging/obs/rustconn.dsc` — change `Version:` from `0.11.7-1` to `0.12.0-1` and source tarball filename
  - [~] 10.10 Update OBS `packaging/obs/AppImageBuilder.yml` — change `version:` from `0.11.7` to `0.12.0`
  - [~] 10.11 Update local Flatpak manifest `packaging/flatpak/io.github.totoshko88.RustConn.yml` if it references a version tag
  - [~] 10.12 Verify all version references are consistent: `grep -r "0\.11\.7" .` should return 0 results (excluding git history and vendor)
  - [~] 10.13 Run `po/update-pot.sh` to ensure translation template includes all new i18n strings
  - [~] 10.14 Create git tag `v0.12.0` and push
