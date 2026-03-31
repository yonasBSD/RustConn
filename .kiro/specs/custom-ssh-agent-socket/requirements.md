# Requirements Document

## Introduction

RustConn users running inside Flatpak cannot override `SSH_AUTH_SOCK` because the Flatpak runtime hard-overwrites the variable after applying environment overrides (via `--socket=ssh-auth`). Users who rely on alternative SSH agents (KeePassXC, Bitwarden SSH agent, or a manually started `ssh-agent`) have no way to point RustConn at a non-default agent socket.

This feature adds two layers of override:

1. A global setting (Settings → SSH Agent tab) that overrides the auto-detected socket for all connections.
2. A per-connection setting (Connection Dialog → SSH tab) that overrides both the global setting and auto-detected socket for a specific connection.

Priority chain: per-connection → global setting → OnceLock agent info → inherited environment.

## Glossary

- **Settings_Dialog**: The application-wide preferences dialog (`rustconn/src/dialogs/settings/`), containing tabs for terminal, UI, SSH agent, etc.
- **SSH_Agent_Tab**: The SSH Agent tab inside the Settings_Dialog, defined in `ssh_agent_tab.rs`.
- **Connection_Dialog**: The dialog for creating or editing a connection (`rustconn/src/dialogs/connection/`).
- **SSH_Tab**: The SSH-specific options panel inside the Connection_Dialog, defined in `connection/ssh.rs`.
- **SshConfig**: The per-connection SSH configuration struct in `rustconn-core/src/models/protocol.rs`.
- **ConfigManager**: The application settings persistence layer that reads/writes TOML configuration.
- **Terminal_Spawner**: The VTE terminal environment builder in `rustconn/src/terminal/mod.rs` that constructs the child process environment including `SSH_AUTH_SOCK`.
- **Agent_Env_Applicator**: The `apply_agent_env()` function in `rustconn-core/src/sftp.rs` that injects `SSH_AUTH_SOCK` into `std::process::Command` objects.
- **CLI**: The `rustconn-cli` crate providing command-line management of connections.
- **Flatpak_Manifest**: The Flatpak build manifest files (`packaging/flatpak/*.yml`, `packaging/flathub/*.yml`) that declare sandbox permissions.
- **Socket_Path**: An absolute filesystem path to a Unix domain socket file used by an SSH agent.
- **Priority_Chain**: The resolution order for determining which SSH agent socket to use: per-connection → global setting → OnceLock agent info → inherited environment variable.

## Requirements

### Requirement 1: Global Custom SSH Agent Socket Setting

**User Story:** As a user with a non-default SSH agent (e.g. KeePassXC or Bitwarden), I want to configure a global custom SSH agent socket path in Settings, so that all my connections use my preferred agent without per-connection configuration.

#### Acceptance Criteria

1. THE SSH_Agent_Tab SHALL display a text entry field labeled "Custom SSH Agent Socket Path" within the Agent Status group
2. WHEN the user enters a Socket_Path into the "Custom SSH Agent Socket Path" field and saves settings, THE ConfigManager SHALL persist the value to the TOML configuration file
3. WHEN the "Custom SSH Agent Socket Path" field is empty, THE ConfigManager SHALL omit the field from the TOML configuration file
4. WHEN the user opens the Settings_Dialog, THE SSH_Agent_Tab SHALL populate the "Custom SSH Agent Socket Path" field with the previously saved value
5. THE SSH_Agent_Tab SHALL display a subtitle or description below the field explaining its purpose (e.g. "Overrides auto-detected SSH_AUTH_SOCK for all connections")

### Requirement 2: Per-Connection SSH Agent Socket Override

**User Story:** As a user who needs different SSH agents for different servers, I want to specify a custom SSH agent socket per connection, so that each connection can use a different agent independently.

#### Acceptance Criteria

1. THE SshConfig SHALL include an optional `ssh_agent_socket` field of type `Option<String>`
2. WHEN the `ssh_agent_socket` field is `None` or empty, THE SshConfig serializer SHALL omit the field from the TOML output
3. THE SSH_Tab SHALL display a text entry field labeled "SSH Agent Socket" within the session or connection options group
4. WHEN the user enters a Socket_Path into the per-connection "SSH Agent Socket" field and saves the connection, THE Connection_Dialog SHALL store the value in the SshConfig `ssh_agent_socket` field
5. WHEN the user opens the Connection_Dialog for an existing connection, THE SSH_Tab SHALL populate the "SSH Agent Socket" field with the previously saved value
6. THE SSH_Tab SHALL display a subtitle or description explaining that this field overrides both the global setting and auto-detected socket

### Requirement 3: Socket Resolution Priority Chain

**User Story:** As a user, I want a clear and predictable priority chain for SSH agent socket resolution, so that per-connection settings take precedence over global settings, which take precedence over auto-detected values.

#### Acceptance Criteria

1. WHEN a connection has a non-empty `ssh_agent_socket` value in SshConfig, THE Terminal_Spawner SHALL use that value as `SSH_AUTH_SOCK` in the child process environment
2. WHEN a connection has no per-connection `ssh_agent_socket` AND a non-empty global custom socket path is configured, THE Terminal_Spawner SHALL use the global setting as `SSH_AUTH_SOCK`
3. WHEN neither per-connection nor global custom socket paths are configured AND OnceLock agent info is available, THE Terminal_Spawner SHALL use the OnceLock agent info socket path as `SSH_AUTH_SOCK`
4. WHEN neither per-connection, global, nor OnceLock agent info is available, THE Terminal_Spawner SHALL inherit `SSH_AUTH_SOCK` from the parent process environment
5. WHEN a connection has a non-empty `ssh_agent_socket` value in SshConfig, THE Agent_Env_Applicator SHALL use that value as `SSH_AUTH_SOCK` when building `std::process::Command` objects for that connection
6. WHEN a non-empty global custom socket path is configured AND no per-connection override exists, THE Agent_Env_Applicator SHALL use the global setting as `SSH_AUTH_SOCK` when building `std::process::Command` objects

### Requirement 4: Socket Path Validation

**User Story:** As a user, I want feedback when I enter an invalid socket path, so that I can correct mistakes before attempting a connection.

#### Acceptance Criteria

1. WHEN the user enters a Socket_Path that is not an absolute path (does not start with `/`), THE SSH_Agent_Tab SHALL display a warning indicating the path must be absolute
2. WHEN the user enters a Socket_Path that is not an absolute path (does not start with `/`), THE SSH_Tab SHALL display a warning indicating the path must be absolute
3. WHEN the user enters a valid absolute Socket_Path that does not exist on disk, THE SSH_Agent_Tab SHALL display an informational message indicating the socket file was not found (non-blocking, as the socket may be created later)
4. WHEN the user enters a valid absolute Socket_Path that does not exist on disk, THE SSH_Tab SHALL display an informational message indicating the socket file was not found (non-blocking, as the socket may be created later)
5. THE Settings_Dialog SHALL allow saving a Socket_Path even when the socket file does not currently exist on disk
6. THE Connection_Dialog SHALL allow saving a Socket_Path even when the socket file does not currently exist on disk

### Requirement 5: CLI Parity

**User Story:** As a CLI user, I want to specify a custom SSH agent socket when adding or updating connections via `rustconn-cli`, so that I have feature parity with the GUI.

#### Acceptance Criteria

1. THE CLI `add` command SHALL accept an optional `--ssh-agent-socket <PATH>` argument
2. THE CLI `update` command SHALL accept an optional `--ssh-agent-socket <PATH>` argument
3. WHEN `--ssh-agent-socket` is provided to the `add` command, THE CLI SHALL store the value in the SshConfig `ssh_agent_socket` field of the new connection
4. WHEN `--ssh-agent-socket` is provided to the `update` command, THE CLI SHALL update the SshConfig `ssh_agent_socket` field of the existing connection
5. THE CLI `show` command SHALL display the `ssh_agent_socket` value when it is set on a connection

### Requirement 6: Flatpak Filesystem Access

**User Story:** As a Flatpak user, I want RustConn to have filesystem access to common alternative SSH agent socket locations, so that custom socket paths are reachable from inside the sandbox.

#### Acceptance Criteria

1. THE Flatpak_Manifest SHALL include `--filesystem=xdg-run/gnupg:ro` to allow access to GPG agent sockets in `XDG_RUNTIME_DIR`
2. THE Flatpak_Manifest SHALL include filesystem access to `~/.var/app/com.bitwarden.desktop/data/:ro` for the Bitwarden SSH agent socket
3. THE Flatpak_Manifest SHALL include `--filesystem=xdg-run/ssh-agent:ro` to allow access to custom SSH agent sockets placed in `XDG_RUNTIME_DIR/ssh-agent/`
4. THE Flatpak_Manifest changes SHALL be applied to both `packaging/flatpak/` and `packaging/flathub/` manifest files

### Requirement 7: Internationalization

**User Story:** As a non-English-speaking user, I want all new UI strings to be translatable, so that the feature is accessible in my language.

#### Acceptance Criteria

1. THE SSH_Agent_Tab SHALL wrap all user-visible strings for the custom socket path field in the `i18n()` macro
2. THE SSH_Tab SHALL wrap all user-visible strings for the per-connection socket field in the `i18n()` macro
3. THE SSH_Agent_Tab SHALL wrap all validation and informational messages for socket path feedback in the `i18n()` macro
4. THE SSH_Tab SHALL wrap all validation and informational messages for socket path feedback in the `i18n()` macro
