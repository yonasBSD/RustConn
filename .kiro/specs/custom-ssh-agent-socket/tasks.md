# Tasks: Custom SSH Agent Socket

## Task 1: Data Model Changes (rustconn-core)

- [x] 1.1 Add `ssh_agent_socket: Option<String>` field to `SshConfig` in `rustconn-core/src/models/protocol.rs` with `#[serde(default, skip_serializing_if = "Option::is_none")]`
- [x] 1.2 Add `ssh_agent_socket: Option<String>` field to `AppSettings` in `rustconn-core/src/config/settings.rs` with `#[serde(default, skip_serializing_if = "Option::is_none")]`

## Task 2: Socket Resolution and Validation (rustconn-core)

- [x] 2.1 Add `SocketPathValidation` enum and `validate_socket_path()` function to `rustconn-core/src/sftp.rs`
- [x] 2.2 Add `resolve_ssh_agent_socket(per_connection: Option<&str>, global_setting: Option<&str>) -> Option<String>` function to `rustconn-core/src/sftp.rs`
- [x] 2.3 Add `apply_agent_env_with_overrides()` function to `rustconn-core/src/sftp.rs` that uses `resolve_ssh_agent_socket`
- [x] 2.4 Add unit tests for `validate_socket_path` (Empty, NotAbsolute, NotFound, Valid cases)
- [x] 2.5 Add unit tests for `resolve_ssh_agent_socket` (all priority chain levels)

## Task 3: Terminal Spawner Integration (rustconn)

- [x] 3.1 Add `ssh_agent_socket: Option<&str>` parameter to `spawn_ssh` in `rustconn/src/terminal/mod.rs`
- [x] 3.2 Update `spawn_command` env_vec building to use resolved socket path when provided (override OnceLock injection)
- [x] 3.3 Update all `spawn_ssh` call sites to pass the resolved socket path

## Task 4: GUI — Settings SSH Agent Tab (rustconn)

- [x] 4.1 Add `adw::EntryRow` for "Custom SSH Agent Socket Path" to `create_ssh_agent_page()` in `ssh_agent_tab.rs`
- [x] 4.2 Add real-time validation feedback (warning/info CSS classes) on the entry row using `validate_socket_path`
- [x] 4.3 Wire load/save of the global `ssh_agent_socket` field from/to `AppSettings` via `ConfigManager`
- [x] 4.4 Wrap all new user-visible strings in `i18n()` macro

## Task 5: GUI — Connection Dialog SSH Tab (rustconn)

- [x] 5.1 Add `adw::EntryRow` for "SSH Agent Socket" to the session group in `create_ssh_options()` in `connection/ssh.rs`
- [x] 5.2 Extend `SshOptionsWidgets` tuple type with the new `Entry` widget
- [x] 5.3 Add real-time validation feedback on the entry row using `validate_socket_path`
- [x] 5.4 Wire load/save of the per-connection `ssh_agent_socket` field from/to `SshConfig`
- [x] 5.5 Update all call sites that destructure `SshOptionsWidgets` to handle the new tuple element
- [x] 5.6 Wrap all new user-visible strings in `i18n()` macro

## Task 6: CLI Parity (rustconn-cli)

- [x] 6.1 Add `--ssh-agent-socket <PATH>` argument to the `Add` command in `rustconn-cli/src/cli.rs`
- [x] 6.2 Add `--ssh-agent-socket <PATH>` argument to the `Update` command in `rustconn-cli/src/cli.rs`
- [x] 6.3 Update the `Add` command handler to store the value in `SshConfig.ssh_agent_socket`
- [x] 6.4 Update the `Update` command handler to update `SshConfig.ssh_agent_socket`
- [x] 6.5 Update the `Show` command handler to display `ssh_agent_socket` when set

## Task 7: Flatpak Manifest Changes

- [x] 7.1 Add `--filesystem=xdg-run/gnupg:ro` to `packaging/flatpak/io.github.totoshko88.RustConn.yml` and `packaging/flatpak/io.github.totoshko88.RustConn.local.yml`
- [x] 7.2 Add `--filesystem=home/.var/app/com.bitwarden.desktop/data/:ro` to the same flatpak manifests
- [x] 7.3 Add `--filesystem=xdg-run/ssh-agent:ro` to the same flatpak manifests
- [x] 7.4 Apply the same three filesystem additions to `packaging/flathub/io.github.totoshko88.RustConn.yml`

## Task 8: Property-Based Tests

- [x] 8.1 Add `proptest` dev-dependency to `rustconn-core/Cargo.toml`
- [x] 8.2 Write property test: Priority chain ordering (Property 1)
- [x] 8.3 Write property test: Empty strings treated as absent (Property 2)
- [x] 8.4 Write property test: SshConfig serialization round-trip (Property 3)
- [x] 8.5 Write property test: AppSettings serialization round-trip (Property 4)
- [x] 8.6 Write property test: Socket path validation correctness (Property 5)
- [x] 8.7 Write property test: Per-connection override isolation (Property 6)
