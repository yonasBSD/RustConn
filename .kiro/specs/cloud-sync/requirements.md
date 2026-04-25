# Requirements: Cloud Sync

> Derived from design: `#[[file:.kiro/specs/cloud-sync/design.md]]`
> Primary reference: `#[[file:docs/CLOUD_SYNC_DESIGN.md]]`

## Requirement 1: SSH Key Inheritance

### 1.1 Group-Level SSH Settings

GIVEN a `ConnectionGroup`
WHEN the group has `ssh_auth_method`, `ssh_key_path`, `ssh_proxy_jump`, or `ssh_agent_socket` fields set
THEN child connections with `SshKeySource::Inherit` inherit those values through the group hierarchy

### 1.2 Inheritance Chain Resolution

GIVEN a connection with `key_source = Inherit` in group "Web" (child of "Production")
WHEN "Web" has no `ssh_key_path` but "Production" has `ssh_key_path = Some("/home/dev/.ssh/prod_key")`
THEN `resolve_ssh_key_path()` returns `Some("/home/dev/.ssh/prod_key")`

### 1.3 Inheritance Termination on Cycles

GIVEN a group hierarchy with a cycle (group A → parent B → parent A)
WHEN `resolve_ssh_key_path()` is called for a connection in group A
THEN the function terminates without infinite loop (visited set prevents revisiting)

### 1.4 SshKeySource::Inherit Variant

GIVEN the `SshKeySource` enum
WHEN a connection's SSH key source is set to `Inherit`
THEN the connection delegates key resolution to its parent group chain

### 1.5 ssh_key_path is Local-Only

GIVEN a connection group with `ssh_key_path` set
WHEN the group is exported for sync
THEN `ssh_key_path` is NOT included in the sync file (each device sets its own path)

### 1.6 Integration with SSH Command Building

GIVEN a connection with inherited SSH settings
WHEN building the SSH command (in sftp.rs, protocols.rs, session start)
THEN the resolved inherited values are used for key_path, auth_method, proxy_jump, and agent_socket

## Requirement 2: Sync Data Models

### 2.1 SyncSettings Structure

GIVEN the application settings
WHEN Cloud Sync is configured
THEN `SyncSettings` contains: `sync_dir` (Optional path), `device_id` (UUID), `device_name` (String), `auto_import_on_start` (bool), `export_debounce_secs` (u32, default 5), `tombstone_retention_days` (u32, default 30)

### 2.2 SyncMode on ConnectionGroup

GIVEN a `ConnectionGroup`
WHEN sync is configured for the group
THEN the group has `sync_mode` (None/Master/Import), `sync_file` (Optional filename), and `last_synced_at` (Optional timestamp)

### 2.3 VariableTemplate Model

GIVEN a sync export
WHEN variable templates are included
THEN each `VariableTemplate` has: `name` (String), `description` (Optional), `is_secret` (bool), `default_value` (Optional, only for non-secret)

## Requirement 3: Group Sync Export (Master)

### 3.1 One Root Group = One .rcn File

GIVEN a root group (parent_id = None) with `sync_mode = Master`
WHEN the group is exported
THEN a single `.rcn` file is written containing the root group, all subgroups, all connections, and variable templates

### 3.2 Filename Fixed at First Export

GIVEN a root group named "Production Servers"
WHEN Cloud Sync is first enabled as Master
THEN the filename is generated as a slug (e.g., `production-servers.rcn`) and stored in `sync_file`, never changing even if the group is renamed

### 3.3 Local-Only Fields Excluded from Export

GIVEN a connection with fields like `last_connected`, `sort_order`, `is_pinned`, `window_geometry`, `ssh_key_path`
WHEN the connection is exported to a sync file
THEN those local-only fields are NOT present in the exported `SyncConnection`

### 3.4 No Secrets in Sync Files

GIVEN connections with `password_source = Variable("web_key")`
WHEN exported to a sync file
THEN only the variable name is included — no secret values, no plaintext passwords, no `SecretString` content

### 3.5 Debounced Export

GIVEN a Master group with active sync
WHEN multiple edits occur within 5 seconds
THEN only one export is triggered (debounce via watch channel)

### 3.6 Atomic File Writes

GIVEN a sync export operation
WHEN writing the `.rcn` file
THEN the write uses atomic write (temp file + rename) so readers never see partial JSON

### 3.7 GroupSyncExport Format

GIVEN a Group Sync export
WHEN serialized to JSON
THEN the file contains: `sync_version` (1), `sync_type` ("group"), `exported_at`, `app_version`, `master_device_id`, `master_device_name`, `root_group`, `groups` (path-based), `connections` (with `group_path`), `variable_templates`

### 3.8 Slug Generation

GIVEN a group name (possibly Unicode)
WHEN `group_name_to_filename()` is called
THEN the result contains only ASCII lowercase alphanumeric + hyphens, ends with `.rcn`, is deterministic, and handles edge cases (special chars, consecutive hyphens)

## Requirement 4: Group Sync Import

### 4.1 Name-Based Merge

GIVEN a local Import group and a remote `GroupSyncExport`
WHEN merge is performed
THEN connections are matched by name within group_path: new remote → create, missing remote → delete local, both exist + remote newer → update synced fields

### 4.2 Local-Only Fields Preserved on Update

GIVEN a local connection being updated from a remote sync
WHEN synced fields are updated
THEN local-only fields (`sort_order`, `is_pinned`, `pin_order`, `window_geometry`, `window_mode`, `last_connected`, `ssh_key_path`, `skip_port_check`) are preserved from the local version

### 4.3 Group Merge by Path

GIVEN local subgroups and remote groups
WHEN merge is performed
THEN groups are matched by hierarchical path: new remote path → create group, missing remote path → delete local group

### 4.4 Variable Templates Created for Missing Variables

GIVEN remote `variable_templates` in a sync file
WHEN a variable name does not exist locally
THEN an empty variable is created (value to be prompted on first connect)

### 4.5 Import on Startup

GIVEN Import groups with `auto_import_on_start = true`
WHEN the application starts
THEN all Import groups are checked: if `exported_at > last_synced_at`, import is triggered

### 4.6 Merge Determinism

GIVEN the same local state and remote export
WHEN `GroupMergeEngine::merge()` is called multiple times
THEN it always produces the same `GroupMergeResult`

### 4.7 Merge Completeness

GIVEN any Group Sync merge
WHEN processing remote connections
THEN every remote connection is either created, updates an existing connection, or matches an unchanged connection — none are silently dropped

## Requirement 5: Import Group UI Restrictions

### 5.1 Synced Fields Read-Only

GIVEN a connection in an Import group
WHEN the edit dialog is shown
THEN synced fields (name, host, port, protocol, username, tags, description, protocol_config) are displayed as read-only `AdwActionRow` with subtitle showing the value and description "Managed by cloud sync"

### 5.2 Local Fields Editable

GIVEN a connection in an Import group
WHEN the edit dialog is shown
THEN local-only fields (password_source, ssh_key_path, sort_order, is_pinned, window settings) are editable via `AdwEntryRow`/`AdwComboRow`/`AdwSwitchRow`

### 5.3 Context Menu Restrictions

GIVEN an Import group in the sidebar
WHEN the context menu is shown
THEN "New Connection", "New Subgroup", and "Delete" are NOT available; "Sync Now" IS available as a flat menu item

### 5.4 No Drag-and-Drop Into Import Group

GIVEN an Import group
WHEN a user attempts to drag a connection INTO the Import group
THEN the operation is rejected

### 5.5 Sidebar Sync Indicator

GIVEN a synced group (Master or Import)
WHEN displayed in the sidebar
THEN an `emblem-synchronizing-symbolic` icon is shown; on error, `dialog-warning-symbolic` is shown; tooltip shows last sync time

## Requirement 6: Credential Resolution UX

### 6.1 CredentialResolutionResult Enum

GIVEN the credential resolution system
WHEN resolving credentials for a connection
THEN the result is one of: `Resolved(Credentials)`, `NotNeeded`, `VariableMissing { variable_name, description, is_secret }`, `BackendNotConfigured { required_backend }`, `VaultEntryMissing { connection_name, lookup_key }`

### 6.2 Variable Missing Dialog

GIVEN a connection with `password_source = Variable("web_key")` where "web_key" has no value
WHEN the user attempts to connect
THEN an `AdwAlertDialog` is shown with heading "Variable Not Configured", extra child with `AdwPasswordEntryRow` (value) + `AdwComboRow` (backend), and responses Cancel / Save & Connect

### 6.3 Save and Retry Flow

GIVEN the Variable Missing dialog
WHEN the user enters a value and clicks "Save & Connect"
THEN the variable is saved to the selected backend AND the connection is retried automatically

### 6.4 Backend Not Configured Dialog

GIVEN a connection whose password_source references an unconfigured backend
WHEN the user attempts to connect
THEN an `AdwAlertDialog` is shown with responses "Enter Password Manually" and "Open Settings"

## Requirement 7: File Watcher

### 7.1 File Change Detection

GIVEN a configured sync directory
WHEN a `.rcn` file is modified by an external process (cloud client)
THEN the `SyncFileWatcher` detects the change using the `notify` crate (inotify/kqueue)

### 7.2 Debounce

GIVEN a file change event
WHEN the file is being written by a cloud client (potentially in chunks)
THEN the watcher debounces for 3 seconds before triggering import

### 7.3 Master Group Filtering

GIVEN a file change event for a file associated with a Master group
WHEN the watcher processes the event
THEN the event is ignored (prevents circular export → import)

### 7.4 JSON Validation Before Import

GIVEN a file change event that passes debounce
WHEN the file is read
THEN JSON is validated before triggering import; corrupt files produce a warning toast, not a crash

## Requirement 8: Settings UI

### 8.1 Cloud Sync Preferences Page

GIVEN the application settings dialog
WHEN the user navigates to Cloud Sync
THEN an `AdwPreferencesPage` is shown with icon `emblem-synchronizing-symbolic` containing groups: Setup, Synced Groups, Available in Cloud, Simple Sync

### 8.2 Setup Group

GIVEN the Cloud Sync settings page
WHEN displayed
THEN it shows `AdwEntryRow` "Sync Directory" (with file chooser suffix button) and `AdwEntryRow` "Device Name"

### 8.3 Available Files

GIVEN a configured sync directory with `.rcn` files not yet imported
WHEN the settings page is displayed
THEN each unimported file is shown as an `AdwActionRow` with an "Import" suffix button

### 8.4 Empty State

GIVEN no sync directory configured
WHEN the user opens Cloud Sync settings
THEN an `AdwStatusPage` is shown with icon `folder-remote-symbolic`, title "Set Up Cloud Sync", and a "Choose Directory" button

## Requirement 9: Simple Sync

### 9.1 Single File Bidirectional Sync

GIVEN Simple Sync enabled
WHEN syncing between devices
THEN a single `full-sync.rcn` file contains all connections, groups, templates, snippets, clusters, and non-secret variables

### 9.2 UUID-Based Merge

GIVEN two devices with Simple Sync
WHEN merging
THEN entities are matched by UUID; `updated_at` determines which version wins (newer wins)

### 9.3 Tombstone Support

GIVEN a deletion on device A
WHEN device B imports
THEN if `tombstone.deleted_at > entity.updated_at`, the entity is deleted on device B

### 9.4 Tombstone Cleanup

GIVEN tombstones in a Simple Sync file
WHEN tombstones are older than `tombstone_retention_days`
THEN they are removed during merge

### 9.5 Device ID Check

GIVEN a Simple Sync import
WHEN `remote.device_id == local.device_id`
THEN the import is skipped (prevents self-sync)

### 9.6 Convergence

GIVEN two devices A and B with Simple Sync
WHEN A exports → B imports → B exports → A imports (no concurrent edits)
THEN both devices have identical non-local-only data

## Requirement 10: CLI Support

### 10.1 Sync Status Command

GIVEN the CLI
WHEN `rustconn-cli sync status` is executed
THEN it displays sync directory, device name, and per-group sync status

### 10.2 Sync List Command

GIVEN the CLI
WHEN `rustconn-cli sync list` is executed
THEN it lists all synced groups with their mode (Master/Import) and last sync time

### 10.3 Sync Export Command

GIVEN the CLI
WHEN `rustconn-cli sync export <group>` is executed
THEN the specified Master group is exported to its sync file

### 10.4 Sync Import Command

GIVEN the CLI
WHEN `rustconn-cli sync import <file>` is executed
THEN the specified `.rcn` file is imported

### 10.5 Sync Now Command

GIVEN the CLI
WHEN `rustconn-cli sync now` is executed
THEN all Master groups are exported and all Import groups are imported

## Requirement 11: Context Menu and Actions

### 11.1 Flat Menu Items

GIVEN a group context menu
WHEN sync is relevant
THEN "Sync Now" and "Enable Cloud Sync..." appear as flat menu items (not in a submenu)

### 11.2 Enable Master Confirmation

GIVEN a user enabling Master sync on a group
WHEN the action is triggered
THEN an `AdwAlertDialog` is shown with heading "Enable Cloud Sync?", body explaining the export path, and responses Cancel / Enable

## Requirement 12: i18n

### 12.1 All User-Facing Strings Internationalized

GIVEN any user-facing string in the Cloud Sync feature
WHEN displayed in the UI
THEN it is wrapped in `i18n()` or `i18n_f()` and has a corresponding entry in `po/rustconn.pot`

## Requirement 13: Edge Cases and Error Handling

### 13.1 Corrupt Sync File

GIVEN a corrupt or partially written `.rcn` file
WHEN import is attempted
THEN a graceful error is returned, an `AdwToast` is shown, and the file watcher continues monitoring

### 13.2 Sync Directory Unavailable

GIVEN a sync directory that becomes unavailable during runtime
WHEN an export or import is attempted
THEN a graceful error is returned, the file watcher is disabled, and sync resumes when the directory is available again

### 13.3 Empty Group Export

GIVEN a Master group with 0 connections
WHEN exported
THEN a valid `.rcn` file is written with empty connections array

### 13.4 Disable Sync Restores Editability

GIVEN an Import group
WHEN sync is disabled
THEN the group becomes a regular editable group (all fields editable, full context menu)

## Requirement 14: Backlog Items

### 14.1 Accessible Labels Cleanup

GIVEN icon-only buttons in the application
WHEN rendered
THEN each has an `update_property` accessible label via `i18n()`

### 14.2 cargo-deny in CI

GIVEN the CI pipeline
WHEN a PR is submitted
THEN `cargo-deny` checks advisories, licenses, bans, and sources

### 14.3 Document Dirty Badge

GIVEN a document with unsaved changes in the sidebar
WHEN displayed
THEN a CSS dot indicator (`.document-dirty`) replaces the text `"• "` prefix
