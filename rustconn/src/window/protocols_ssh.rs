//! SSH connection launch and reconnect logic.
//!
//! Extracted from `window/protocols.rs` to reduce module complexity.

use super::MainWindow;
use super::protocols::{
    SharedNotebook, SharedSidebar, append_proxy_command_destination, contains_ssh_failure,
    resolve_automation_for_connection, substitute_variables,
};
use crate::state::SharedAppState;
use crate::utils::spawn_blocking_with_callback;
use gtk4::prelude::*;
use rustconn_core::connection::check_port;
use rustconn_core::connection::ssh_inheritance;
use secrecy::SecretString;
use std::rc::Rc;
use uuid::Uuid;

/// Returns `true` if `text` ends with an SSH password prompt, in any of the
/// supported UI languages.
///
/// Returns `false` for key-passphrase prompts: those require `key_passphrase`,
/// not the account password. Shared by initial connect and in-place reconnect so
/// multilingual auto-login behaves identically on both paths.
fn detect_password_prompt(text: &str) -> bool {
    let lower = text.to_lowercase();

    // Reject passphrase prompts — these need key_passphrase, not the password.
    let last_line = lower.lines().last().unwrap_or("").trim();
    if last_line.contains("passphrase for key") || last_line.contains("passphrase for") {
        return false;
    }

    lower.ends_with("password: ")
        || lower.ends_with("password:")
        || lower.contains("password: \n")
        || lower.lines().last().is_some_and(|line| {
            let l = line.trim().to_lowercase();
            l.ends_with("password:")
                || l.ends_with("password: ")
                || l.contains("'s password:")
                // German
                || l.ends_with("passwort:")
                || l.ends_with("passwort: ")
                || l.ends_with("kennwort:")
                || l.ends_with("kennwort: ")
                // French
                || l.ends_with("mot de passe:")
                || l.ends_with("mot de passe :")
                || l.ends_with("mot de passe : ")
                // Spanish
                || l.ends_with("contraseña:")
                || l.ends_with("contraseña: ")
                // Portuguese
                || l.ends_with("senha:")
                || l.ends_with("senha: ")
                // Ukrainian / Belarusian
                || l.ends_with("пароль:")
                || l.ends_with("пароль: ")
                // Polish
                || l.ends_with("hasło:")
                || l.ends_with("hasło: ")
                // Czech/Slovak
                || l.ends_with("heslo:")
                || l.ends_with("heslo: ")
                // Dutch
                || l.ends_with("wachtwoord:")
                || l.ends_with("wachtwoord: ")
                // Swedish/Danish/Norwegian
                || l.ends_with("lösenord:")
                || l.ends_with("lösenord: ")
                || l.ends_with("adgangskode:")
                || l.ends_with("adgangskode: ")
                // Chinese
                || l.ends_with("密码:")
                || l.ends_with("密码：")
                || l.ends_with("密碼:")
                || l.ends_with("密碼：")
                // Japanese
                || l.ends_with("パスワード:")
                || l.ends_with("パスワード：")
                // Korean
                || l.ends_with("비밀번호:")
                || l.ends_with("비밀번호：")
                // Generic colon-terminated prompt (catch-all for PAM)
                || l.ends_with("pass:")
                || l.ends_with("pass: ")
        })
}

/// Environment variable carrying the jump host (bastion) password to the
/// `SSH_ASKPASS` helper. Intentionally obscure to reduce exposure in
/// `/proc/<pid>/environ`, matching the SSH tunnel askpass convention.
const JUMP_HOST_PW_ENV: &str = "_RC_JH_PW";

/// Returns the path to a reusable `SSH_ASKPASS` helper that echoes the jump
/// host password from [`JUMP_HOST_PW_ENV`].
///
/// The script holds NO secret — only the env var name — so it is safe to keep
/// for the process lifetime and share across sessions. The password itself
/// lives solely in the spawned ssh process's environment. The script is placed
/// in `$XDG_RUNTIME_DIR` (tmpfs, mode 0700, user-private) to avoid `/tmp`
/// symlink races on a fixed filename, falling back to a randomized temp path.
/// Created once (mode 0700) and cached; returns `None` if creation fails.
fn jump_host_askpass_script() -> Option<std::path::PathBuf> {
    use std::sync::OnceLock;
    static SCRIPT: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    SCRIPT
        .get_or_init(|| {
            let path = match std::env::var_os("XDG_RUNTIME_DIR") {
                Some(dir) if !dir.is_empty() => {
                    std::path::PathBuf::from(dir).join("rustconn-jh-askpass.sh")
                }
                // No user-private runtime dir — randomize the name so a hostile
                // local user cannot pre-create/symlink a predictable /tmp path.
                _ => std::env::temp_dir().join(format!("rc-jh-askpass-{}.sh", Uuid::new_v4())),
            };

            let script = format!("#!/bin/sh\nprintf '%s\\n' \"${{{JUMP_HOST_PW_ENV}}}\"\n");
            if let Err(e) = std::fs::write(&path, script.as_bytes()) {
                tracing::error!(error = %e, "Failed to create jump host askpass script");
                return None;
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Err(e) =
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
                {
                    tracing::error!(error = %e, "Failed to chmod jump host askpass script");
                    return None;
                }
            }

            Some(path)
        })
        .clone()
}

/// Builds the SSH command pieces shared by initial connect and in-place
/// reconnect: the resolved identity file, the extra CLI args (including the
/// jump-host `ProxyCommand`/`-J` wiring and Flatpak known_hosts), whether
/// waypipe is used, and the resolved jump-host chain string (for monitoring).
///
/// Returns `(identity_file, extra_args, use_waypipe, jump_host_chain,
/// jump_host_password)`. The last element is the immediate jump hop's own
/// password (issue #191), set only when an `SSH_ASKPASS` helper was wired into
/// the `ProxyCommand`; the caller must expose it via [`JUMP_HOST_PW_ENV`] in
/// the spawned ssh environment. For non-SSH protocols it returns empty defaults.
///
/// Extracted from `start_ssh_connection` and `reconnect_ssh_in_place`, which
/// previously carried ~150 near-identical lines each — a fix to one path could
/// silently miss the other.
fn build_ssh_command_args(
    conn: &rustconn_core::Connection,
    connection_id: Uuid,
    state: &SharedAppState,
    groups: &[rustconn_core::ConnectionGroup],
) -> (
    Option<String>,
    Vec<String>,
    bool,
    Option<String>,
    Option<SecretString>,
) {
    let rustconn_core::ProtocolConfig::Ssh(ssh_config) = &conn.protocol_config else {
        return (None, Vec::new(), false, None, None);
    };

    // Resolve key path via inheritance (connection → group → parent group → root)
    let key = ssh_inheritance::resolve_ssh_key_path(conn, groups)
        .and_then(|p| {
            // Resolve stale portal paths: if the stored path doesn't exist,
            // check the Flatpak SSH dir for a file with the same name.
            rustconn_core::resolve_key_path(&p)
        })
        .map(|p| p.to_string_lossy().to_string());

    // Use build_command_args() for all SSH-specific flags:
    // identity, IdentitiesOnly, proxy_jump, ControlMaster/Persist,
    // agent forwarding, X11, compression, custom options, port forwards
    let mut args = ssh_config.build_command_args();

    // Remove -i <path> from args because the identity file is already
    // resolved separately via resolve_ssh_key_path() and passed as
    // `identity_file` to spawn_ssh(). Keeping both causes the key to
    // appear twice in the final command line.
    if key.is_some()
        && let Some(pos) = args.iter().position(|a| a == "-i")
    {
        args.remove(pos); // remove "-i"
        if pos < args.len() {
            args.remove(pos); // remove the path value
        }
    }

    // Resolve jump host chain from connection references (needs state access)
    let mut jump_hosts = Vec::new();
    // PKCS#11 provider of the immediate (first) jump hop, if it opts in.
    // `-o PKCS11Provider` is NOT inherited by ProxyJump child connections,
    // so it must be injected into the first hop's ProxyCommand explicitly.
    let mut first_hop_pkcs11: Option<String> = None;
    // Password of the immediate jump hop, resolved from its OWN cached
    // credentials. Without this the target connection's password is fed to the
    // bastion prompt (issue #191). Delivered to the bastion via SSH_ASKPASS on
    // the nested ProxyCommand ssh, NOT via the VTE prompt.
    let mut first_hop_password: Option<SecretString> = None;

    // Handle string-based proxy jump (legacy/manual or inherited from group)
    if let Some(proxy) = ssh_inheritance::resolve_ssh_proxy_jump(conn, groups) {
        jump_hosts.push(proxy);
    }

    // Handle reference-based jump host (recursive resolution)
    if let Some(jump_id) = ssh_config.jump_host_id
        && let Ok(state_ref) = state.try_borrow()
    {
        let mut current_id = Some(jump_id);
        let mut visited = std::collections::HashSet::new();
        visited.insert(connection_id); // Avoid self-reference loop

        // Limit recursion depth to avoid infinite loops
        for _ in 0..10 {
            if let Some(jid) = current_id {
                if visited.contains(&jid) {
                    break;
                }
                visited.insert(jid);

                if let Some(jump_conn) = state_ref.get_connection(jid) {
                    // The immediate hop is the one we ProxyCommand into.
                    let is_first_hop = jump_hosts.is_empty();
                    // Format: [user@]host[:port]
                    let mut host_str = jump_conn.host.clone();
                    if let Some(user) = &jump_conn.username {
                        host_str = format!("{user}@{host_str}");
                    }
                    if jump_conn.port != 22 {
                        host_str = format!("{}:{}", host_str, jump_conn.port);
                    }
                    jump_hosts.push(host_str);

                    // Check if this jump host has its own jumper
                    if let rustconn_core::ProtocolConfig::Ssh(jump_config) =
                        &jump_conn.protocol_config
                    {
                        // Opt-in PKCS#11 for the first hop (token to reach the bastion)
                        if is_first_hop {
                            first_hop_pkcs11 = jump_config
                                .pkcs11_provider
                                .clone()
                                .filter(|p| !p.trim().is_empty());
                            // Resolve the bastion's OWN password (issue #191).
                            // First try the in-memory cache (fast path).
                            first_hop_password = state_ref
                                .get_cached_credentials(jid)
                                .filter(|c| {
                                    use secrecy::ExposeSecret;
                                    !c.password.expose_secret().is_empty()
                                })
                                .map(|c| c.password.clone());
                            // Fallback: resolve from vault if not cached (issue #191).
                            // By this point the vault is already unlocked (target
                            // credentials were resolved first), so this is fast (~100ms).
                            if first_hop_password.is_none() {
                                let secret_settings = state_ref.settings().secrets.clone();
                                let jump_name = jump_conn.name.clone();
                                let jump_host = jump_conn.host.clone();
                                let next_jump_id = jump_config.jump_host_id;
                                let manual_proxy = jump_config.proxy_jump.clone();
                                let backend_type =
                                    crate::vault_ops::select_backend_for_load(&secret_settings);
                                let lookup_key = crate::vault_ops::generate_store_key(
                                    &jump_name,
                                    &jump_host,
                                    "ssh",
                                    backend_type,
                                );
                                // Must drop state borrow before blocking vault call
                                drop(state_ref);
                                if let Ok(Some(creds)) = crate::vault_ops::dispatch_vault_op(
                                    &secret_settings,
                                    &lookup_key,
                                    crate::vault_ops::VaultOp::Retrieve,
                                ) && let Some(pw) = creds.expose_password()
                                    && !pw.is_empty()
                                {
                                    let pw_secret = SecretString::from(pw.to_string());
                                    // Cache for future use
                                    if let Ok(mut state_mut) = state.try_borrow_mut() {
                                        state_mut.cache_credentials(
                                            jid,
                                            creds.username.as_deref().unwrap_or(""),
                                            pw,
                                            "",
                                        );
                                    }
                                    first_hop_password = Some(pw_secret);
                                }
                                // Prepend manual proxy from first hop (saved before drop)
                                if let Some(p) = manual_proxy {
                                    jump_hosts.insert(jump_hosts.len() - 1, p);
                                }
                                // Continue collecting the rest of the chain if multi-hop.
                                // Re-borrow and resume from next_jump_id.
                                if let Some(nid) = next_jump_id
                                    && let Ok(state_ref2) = state.try_borrow()
                                {
                                    let mut cid = Some(nid);
                                    for _ in 0..9 {
                                        if let Some(id) = cid {
                                            if visited.contains(&id) {
                                                break;
                                            }
                                            visited.insert(id);
                                            if let Some(jc) = state_ref2.get_connection(id) {
                                                let mut hs = jc.host.clone();
                                                if let Some(u) = &jc.username {
                                                    hs = format!("{u}@{hs}");
                                                }
                                                if jc.port != 22 {
                                                    hs = format!("{}:{}", hs, jc.port);
                                                }
                                                jump_hosts.push(hs);
                                                cid = match &jc.protocol_config {
                                                    rustconn_core::ProtocolConfig::Ssh(c) => {
                                                        c.jump_host_id
                                                    }
                                                    _ => None,
                                                };
                                            } else {
                                                break;
                                            }
                                        } else {
                                            break;
                                        }
                                    }
                                }
                                break;
                            }
                        }
                        // Prepend manual proxy if exists on jump host (unlikely but possible)
                        if let Some(p) = &jump_config.proxy_jump {
                            jump_hosts.insert(jump_hosts.len() - 1, p.clone());
                        }
                        current_id = jump_config.jump_host_id;
                    } else {
                        current_id = None;
                    }
                } else {
                    current_id = None;
                }
            } else {
                break;
            }
        }
    }

    // In Flatpak, ~/.ssh is read-only — point known_hosts to a writable path.
    // Must be set BEFORE jump host resolution because ProxyCommand needs it too.
    let flatpak_known_hosts = {
        let user_set = ssh_config
            .custom_options
            .keys()
            .any(|k| k.eq_ignore_ascii_case("UserKnownHostsFile"));
        if user_set {
            None
        } else {
            rustconn_core::get_flatpak_known_hosts_path()
        }
    };
    if let Some(ref kh_path) = flatpak_known_hosts {
        tracing::debug!(
            protocol = "ssh",
            path = %kh_path.display(),
            "Using Flatpak-writable known_hosts"
        );
        args.push("-o".to_string());
        args.push(format!("UserKnownHostsFile={}", kh_path.display()));
    }

    // Override proxy_jump with resolved jump host chain if we have
    // reference-based jump hosts (build_command_args already added -J
    // for the string-based proxy_jump, so only add if we have more)
    //
    // In Flatpak, -J (ProxyJump) spawns a nested SSH process that does NOT
    // inherit -o or -i flags from the outer command. This means the jump host
    // SSH tries to write to ~/.ssh/known_hosts (read-only) and cannot find
    // identity files. Fix: replace -J with -o ProxyCommand that passes
    // UserKnownHostsFile and identity to the jump host SSH process.
    // Password to deliver to the first jump hop via SSH_ASKPASS (issue #191).
    // Set only when an askpass helper was successfully wired into ProxyCommand.
    let mut jump_host_password: Option<SecretString> = None;

    let jump_host_str = if jump_hosts.is_empty() {
        None
    } else {
        // Remove the -J added by build_command_args (if proxy_jump was set)
        if ssh_config.proxy_jump.is_some()
            && let Some(pos) = args.iter().position(|a| a == "-J")
        {
            args.remove(pos); // remove "-J"
            if pos < args.len() {
                args.remove(pos); // remove the value
            }
        }
        let chain = jump_hosts.join(",");

        // `-J` spawns a nested SSH process that does NOT inherit -o/-i
        // from the outer command. When the jump host needs Flatpak
        // known_hosts/identity OR a PKCS#11 token, switch to an explicit
        // ProxyCommand that passes those to the first hop.
        //
        // The first hop's own password (issue #191) is also delivered here:
        // the nested ProxyCommand ssh has no controlling TTY, so SSH_ASKPASS
        // with SSH_ASKPASS_REQUIRE=force — scoped to it via the shell
        // env-assignment prefix — authenticates the bastion with ITS password.
        // The OUTER ssh keeps its VTE TTY and prompts for the TARGET password,
        // which the VTE auto-fill handles. The password rides in the obscure
        // JUMP_HOST_PW_ENV env var of the outer ssh; only the var NAME appears
        // in the command line.
        let askpass_script = if first_hop_password.is_some() {
            jump_host_askpass_script()
        } else {
            None
        };

        if flatpak_known_hosts.is_some() || first_hop_pkcs11.is_some() || askpass_script.is_some() {
            // Build a ProxyCommand for the first hop;
            // if there are multiple hops, nest them via -J within ProxyCommand.
            let mut proxy_parts: Vec<String> = Vec::new();

            // Env assignments scoped to the nested ssh only (issue #191).
            // Use `env` command because OpenSSH ≥10 prepends `exec` to ProxyCommand,
            // and `exec VAR=val cmd` is not valid POSIX sh (the shell treats it
            // as a command path). `env VAR=val cmd` works in all shells.
            if let Some(ref script) = askpass_script {
                proxy_parts.push("env".to_string());
                proxy_parts.push(format!("SSH_ASKPASS={}", script.display()));
                proxy_parts.push("SSH_ASKPASS_REQUIRE=force".to_string());
                jump_host_password = first_hop_password.clone();
            }

            proxy_parts.push("ssh".to_string());
            proxy_parts.push("-W".to_string());
            proxy_parts.push("%h:%p".to_string());

            // Pass identity file to jump host if we have one
            if let Some(pos) = args.iter().position(|a| a == "-i")
                && let Some(key_path) = args.get(pos + 1)
            {
                proxy_parts.push("-i".to_string());
                proxy_parts.push(key_path.clone());
                proxy_parts.push("-o".to_string());
                proxy_parts.push("IdentitiesOnly=yes".to_string());
            }

            // Pass PKCS#11 provider to the first hop (token also auths the bastion)
            if let Some(ref provider) = first_hop_pkcs11 {
                proxy_parts.push("-o".to_string());
                proxy_parts.push(format!("PKCS11Provider={}", provider.trim()));
            }

            // Pass UserKnownHostsFile to jump host (Flatpak only)
            if let Some(ref kh_path) = flatpak_known_hosts {
                proxy_parts.push("-o".to_string());
                proxy_parts.push(format!("UserKnownHostsFile={}", kh_path.display()));
            }

            // ponytail: PKCS#11/identity reach only the first hop; deeper
            // hops still don't get the bastion's own PKCS#11 token. Fine for
            // the common single-bastion case.
            //
            // Multi-hop: nest a ProxyCommand per remaining hop so EACH inherits
            // the identity file and Flatpak known_hosts. Plain `-J b,c` here
            // would drop them and the deeper hops fail key auth / host-key
            // verification in Flatpak (issue #191 follow-up — double jump).
            if jump_hosts.len() > 1 {
                let identity_key = args
                    .iter()
                    .position(|a| a == "-i")
                    .and_then(|pos| args.get(pos + 1))
                    .map(String::as_str);
                let inner_hops: Vec<&str> = jump_hosts[1..].iter().map(String::as_str).collect();
                // accept_new = false: keep the existing host-key posture (the
                // bastions are expected to already be in known_hosts).
                let inner = rustconn_core::ssh_tunnel::build_nested_proxy_command(
                    &inner_hops,
                    identity_key,
                    flatpak_known_hosts.as_deref(),
                    false,
                );
                proxy_parts.push("-o".to_string());
                proxy_parts.push(format!(
                    "ProxyCommand={}",
                    rustconn_core::ssh_tunnel::shell_single_quote(&inner)
                ));
            }

            // Add the first hop destination (parse user@host:port into -p port user@host)
            append_proxy_command_destination(&mut proxy_parts, &jump_hosts[0]);

            let proxy_cmd = proxy_parts.join(" ");
            tracing::debug!(
                protocol = "ssh",
                proxy_command = %proxy_cmd,
                "Using ProxyCommand instead of -J (Flatpak known_hosts or PKCS#11 jump host)"
            );
            args.push("-o".to_string());
            args.push(format!("ProxyCommand={proxy_cmd}"));
        } else {
            // Non-Flatpak: use standard -J. `chain` is target-first (RustConn's
            // internal order); OpenSSH `-J` visits hops client-first, so reverse.
            args.push("-J".to_string());
            args.push(rustconn_core::ssh_tunnel::proxy_jump_arg(&chain));
        }

        Some(chain)
    };

    // Check waypipe: enabled in config + binary available on PATH
    let waypipe = ssh_config.waypipe && rustconn_core::protocol::detect_waypipe().installed;
    if ssh_config.waypipe && !waypipe {
        tracing::warn!(
            protocol = "ssh",
            host = %conn.host,
            "Waypipe enabled but not found on PATH, falling back to direct SSH"
        );
    }
    if waypipe {
        tracing::info!(
            protocol = "ssh",
            host = %conn.host,
            "Using waypipe for Wayland application forwarding"
        );
    }

    (key, args, waypipe, jump_host_str, jump_host_password)
}

/// Creates a terminal tab and spawns the SSH process with the given configuration.
pub fn start_ssh_connection(
    state: &SharedAppState,
    notebook: &SharedNotebook,
    sidebar: &SharedSidebar,
    monitoring: &super::types::SharedMonitoring,
    connection_id: Uuid,
    conn: &rustconn_core::Connection,
    logging_enabled: bool,
) -> Option<Uuid> {
    // Check if port check is needed
    let settings = state.borrow().settings().clone();
    // Collect groups for SSH inheritance resolution (proxy_jump can be inherited from group)
    let groups: Vec<rustconn_core::ConnectionGroup> = state
        .try_borrow()
        .ok()
        .map(|s| s.list_groups().into_iter().cloned().collect())
        .unwrap_or_default();
    let has_inherited_proxy = ssh_inheritance::resolve_ssh_proxy_jump(conn, &groups).is_some();
    // Use centralized probe-bypass logic + inherited proxy jump from groups
    let should_check = conn.should_pre_connect_check(&settings.connection) && !has_inherited_proxy;

    if conn.bypasses_direct_probe() || has_inherited_proxy {
        tracing::debug!(
            protocol = "ssh",
            host = %conn.host,
            port = conn.port,
            "Skipping port check — connection bypasses direct probe"
        );
    }

    if should_check {
        let host = conn.host.clone();
        let port = conn.port;
        let timeout = settings.connection.port_check_timeout_secs;
        let state_clone = state.clone();
        let notebook_clone = notebook.clone();
        let sidebar_clone = sidebar.clone();
        let monitoring_clone = Rc::clone(monitoring);
        let conn_clone = conn.clone();

        // Run port check in background thread
        spawn_blocking_with_callback(
            move || check_port(&host, port, timeout),
            move |result| {
                match result {
                    Ok(_) => {
                        // Port is open, proceed with connection
                        start_ssh_connection_internal(
                            &state_clone,
                            &notebook_clone,
                            &sidebar_clone,
                            &monitoring_clone,
                            connection_id,
                            &conn_clone,
                            logging_enabled,
                        );
                    }
                    Err(e) => {
                        // Port check failed, show error with retry
                        tracing::warn!(
                            protocol = "ssh",
                            host = %conn_clone.host,
                            port = conn_clone.port,
                            error = %e,
                            "Port check failed for SSH connection"
                        );
                        sidebar_clone
                            .update_connection_status(&connection_id.to_string(), "failed");
                        // Record the failed attempt in history (the session is
                        // never created on a port-check failure, so do it here).
                        if let Ok(mut state_mut) = state_clone.try_borrow_mut() {
                            state_mut.record_connection_attempt_failed(
                                &conn_clone,
                                conn_clone.username.as_deref(),
                                &e.to_string(),
                            );
                        }
                        if let Some(root) = notebook_clone.widget().root()
                            && let Some(window) = root.downcast_ref::<gtk4::Window>()
                        {
                            crate::toast::show_retry_toast_on_window(
                                window,
                                &e.to_string(),
                                &connection_id.to_string(),
                            );
                        }
                    }
                }
            },
        );
        // Return None since the actual session will be created asynchronously
        None
    } else {
        // Port check disabled, proceed directly
        start_ssh_connection_internal(
            state,
            notebook,
            sidebar,
            monitoring,
            connection_id,
            conn,
            logging_enabled,
        )
    }
}

/// Internal function to start SSH connection (after port check).
///
/// Creates a terminal tab and spawns the SSH process with the given configuration.
fn start_ssh_connection_internal(
    state: &SharedAppState,
    notebook: &SharedNotebook,
    sidebar: &SharedSidebar,
    monitoring: &super::types::SharedMonitoring,
    connection_id: Uuid,
    conn: &rustconn_core::Connection,
    logging_enabled: bool,
) -> Option<Uuid> {
    use rustconn_core::protocol::{format_command_message, format_connection_message};

    let conn_name = conn.name.clone();

    // Get terminal settings from state
    let terminal_settings = state
        .try_borrow()
        .ok()
        .map(|s| s.settings().terminal.clone())
        .unwrap_or_default();

    // Get global variables for substitution (secret values resolved from vault)
    let global_variables = state
        .try_borrow()
        .ok()
        .map(|s| crate::state::resolve_global_variables(s.settings()))
        .unwrap_or_default();

    // Resolve automation config with group inheritance
    let resolved_automation = resolve_automation_for_connection(state, conn);

    // Create terminal tab for SSH with user settings
    let session_id = notebook.create_terminal_tab_with_settings(
        connection_id,
        &conn.name,
        "ssh",
        Some(&resolved_automation),
        &terminal_settings,
        conn.theme_override.as_ref(),
        &global_variables,
    );

    // Apply highlight rules (built-in defaults + global + per-connection)
    {
        let global_rules = state
            .try_borrow()
            .ok()
            .map(|s| s.settings().highlight_rules.clone())
            .unwrap_or_default();
        notebook.set_highlight_rules(session_id, &global_rules, &conn.highlight_rules);
    }

    // Record connection start in history
    let history_entry_id = if let Ok(mut state_mut) = state.try_borrow_mut() {
        Some(state_mut.record_connection_start(conn, conn.username.as_deref()))
    } else {
        None
    };

    // Store history entry ID in session for later use
    if let Some(entry_id) = history_entry_id {
        notebook.set_history_entry_id(session_id, entry_id);
    }

    // Build and spawn SSH command
    let port = conn.port;

    // Collect groups for SSH inheritance resolution
    let groups: Vec<rustconn_core::ConnectionGroup> = state
        .try_borrow()
        .ok()
        .map(|s| s.list_groups().into_iter().cloned().collect())
        .unwrap_or_default();

    // Detect jump host / proxy for status detection and monitoring
    let has_jump_host = matches!(
        &conn.protocol_config,
        rustconn_core::ProtocolConfig::Ssh(ssh)
            if ssh.jump_host_id.is_some() || ssh.proxy_command.is_some()
    ) || ssh_inheritance::resolve_ssh_proxy_jump(conn, &groups).is_some();

    // Apply variable substitution to host and username (e.g., ${VAR_NAME} -> actual value)
    let host = substitute_variables(&conn.host, &global_variables);
    let username = conn
        .username
        .as_ref()
        .map(|u| substitute_variables(u, &global_variables));

    // Get SSH-specific options
    let (identity_file, extra_args, use_waypipe, jump_host_chain, jump_host_password) =
        build_ssh_command_args(conn, connection_id, state, &groups);

    // Update last_connected timestamp
    if let Ok(mut state_mut) = state.try_borrow_mut()
        && let Err(e) = state_mut.update_last_connected(connection_id)
    {
        tracing::warn!(?e, "Failed to update last_connected");
    }

    // Set up session logging if enabled
    if logging_enabled {
        MainWindow::setup_session_logging(state, notebook, session_id, connection_id, &conn_name);
    }

    // Wire up child exited callback for session cleanup
    MainWindow::setup_child_exited_handler(state, notebook, sidebar, session_id, connection_id);

    // Build SSH command string for display
    let mut ssh_cmd_parts = if use_waypipe {
        vec!["waypipe".to_string(), "ssh".to_string()]
    } else {
        vec!["ssh".to_string()]
    };
    if port != 22 {
        ssh_cmd_parts.push("-p".to_string());
        ssh_cmd_parts.push(port.to_string());
    }
    if let Some(ref key) = identity_file {
        ssh_cmd_parts.push("-i".to_string());
        ssh_cmd_parts.push(key.clone());
    }
    ssh_cmd_parts.extend(extra_args.clone());
    let destination = if let Some(ref user) = username {
        format!("{user}@{host}")
    } else {
        host.clone()
    };
    ssh_cmd_parts.push(destination);
    let ssh_command = ssh_cmd_parts.join(" ");

    // Display CLI output feedback before executing command
    let conn_msg = format_connection_message("SSH", &host);
    let cmd_msg = format_command_message(&ssh_command);
    let feedback = format!("{conn_msg}\r\n{cmd_msg}\r\n\r\n");
    notebook.display_output(session_id, &feedback);

    // Retrieve cached credentials (resolved from vault earlier)
    let cached_password: Option<SecretString> = state
        .try_borrow()
        .ok()
        .and_then(|s| s.get_cached_credentials(connection_id).cloned())
        .and_then(|c| {
            use secrecy::ExposeSecret;
            let pw = c.password.expose_secret();
            if pw.is_empty() {
                None
            } else {
                Some(c.password.clone())
            }
        });

    // Spawn SSH normally — password injection happens via VTE feed_child
    // when the terminal detects a password prompt (see below).
    {
        let extra_refs: Vec<&str> = extra_args.iter().map(std::string::String::as_str).collect();
        let agent_socket = ssh_inheritance::resolve_ssh_agent_socket(conn, &groups);
        let startup_cmd = match &conn.protocol_config {
            rustconn_core::ProtocolConfig::Ssh(cfg) => cfg.startup_command.as_deref(),
            _ => None,
        };
        // Jump host password (issue #191) travels in an obscure env var read by
        // the SSH_ASKPASS helper wired into ProxyCommand. Zeroized once the VTE
        // spawn has consumed the environment.
        let jump_host_env = jump_host_password.as_ref().map(|pw| {
            use secrecy::ExposeSecret;
            zeroize::Zeroizing::new(format!("{JUMP_HOST_PW_ENV}={}", pw.expose_secret()))
        });
        let extra_env = jump_host_env.as_ref().map(|e| [e.as_str()]);
        notebook.spawn_ssh(
            session_id,
            &host,
            port,
            username.as_deref(),
            identity_file.as_deref(),
            &extra_refs,
            use_waypipe,
            agent_socket.as_deref(),
            startup_cmd,
            extra_env.as_ref().map(<[&str; 1]>::as_slice),
        );
    }

    // --- VTE password injection: detect "password:" prompt and feed cached password ---
    // This replaces the previous sshpass dependency. The terminal output is
    // monitored for SSH password prompts; when detected, the vault password
    // is sent via feed_child() exactly once.
    // NOTE: Passphrase prompts ("Enter passphrase for key") are explicitly
    // excluded to avoid sending the wrong secret when SSH auth is PublicKey.
    //
    // We subscribe to BOTH `contents-changed` AND `cursor-moved` because
    // `contents-changed` alone does not fire reliably for SSH password prompts
    // output in no-echo mode with cursor positioning escapes (issue #194).
    if let Some(vault_password) = cached_password.clone() {
        let password_sent = std::rc::Rc::new(std::cell::Cell::new(false));

        tracing::info!(
            protocol = "ssh",
            host = %host,
            "Vault password available; will auto-fill on prompt"
        );

        // Shared closure logic extracted into an Rc to avoid duplicating
        // the detection + injection code across two signal handlers.
        let check_and_inject = {
            let notebook_clone = notebook.clone();
            let password_sent = password_sent.clone();
            let vault_password = vault_password.clone();
            std::rc::Rc::new(move || {
                if password_sent.get() {
                    return;
                }
                let Some(text) = notebook_clone.get_terminal_text(session_id) else {
                    return;
                };

                if detect_password_prompt(&text) {
                    use secrecy::ExposeSecret;
                    // Wrap in Zeroizing so the plaintext password is wiped from memory
                    // immediately after it is handed to VTE, instead of lingering until GC.
                    let input =
                        zeroize::Zeroizing::new(format!("{}\n", vault_password.expose_secret()));
                    notebook_clone.send_text_to_session(session_id, &input);
                    password_sent.set(true);
                    tracing::info!(
                        protocol = "ssh",
                        "Password prompt detected; credentials sent via VTE"
                    );
                }
            })
        };

        // contents-changed: fires for most terminal output
        let on_contents_changed = check_and_inject.clone();
        notebook.connect_contents_changed(session_id, move || on_contents_changed());

        // cursor-moved: fires reliably for prompts using cursor positioning
        // escapes without a trailing newline (SSH password prompt, issue #194)
        let on_cursor_moved = check_and_inject;
        notebook.connect_cursor_moved(session_id, move || on_cursor_moved());
    }

    // --- SSH status detection: mark sidebar "connected" once terminal output appears ---
    // For jump host connections, also check terminal text for SSH failure patterns
    // to avoid false positives (jump host connects but destination times out).
    {
        let sidebar_clone = sidebar.clone();
        let notebook_clone = notebook.clone();
        let connection_id_str = connection_id.to_string();
        let session_connected = std::rc::Rc::new(std::cell::Cell::new(false));
        let session_connected_clone = session_connected.clone();
        let protocol_str = String::from("ssh");
        let uses_jump_host = has_jump_host;

        notebook.connect_contents_changed(session_id, move || {
            if session_connected_clone.get() {
                return;
            }
            if let Some(row) = notebook_clone.get_terminal_cursor_row(session_id) {
                tracing::debug!(
                    protocol = "ssh",
                    cursor_row = row,
                    threshold = 2,
                    "SSH status detection: checking cursor row"
                );
                if row > 2 {
                    // When using a jump host, the cursor may advance past row 2
                    // due to jump host banners or SSH error output even if the
                    // final destination is unreachable. Check terminal text for
                    // known SSH failure patterns before marking as connected.
                    if uses_jump_host
                        && let Some(text) = notebook_clone.get_terminal_text(session_id)
                        && contains_ssh_failure(&text)
                    {
                        tracing::debug!(
                            protocol = "ssh",
                            cursor_row = row,
                            "Jump host connection: SSH failure detected in terminal"
                        );
                        return;
                    }
                    sidebar_clone.increment_session_count(&connection_id_str);
                    session_connected_clone.set(true);
                    tracing::info!(
                        protocol = %protocol_str,
                        cursor_row = row,
                        "Terminal connection detected as established"
                    );
                }
            }
        });
    }

    // --- Auto-recording: start recording once SSH connection is established ---
    if conn.session_recording_enabled {
        let notebook_clone = notebook.clone();
        let recording_conn_name = conn_name.clone();
        let recording_started = std::rc::Rc::new(std::cell::Cell::new(false));
        let recording_started_clone = recording_started.clone();
        let recording_ssh_params = Some(crate::terminal::SshRecordingParams {
            host: host.clone(),
            port,
            username: username.clone(),
            identity_file: identity_file.clone(),
        });

        notebook.connect_contents_changed(session_id, move || {
            if recording_started_clone.get() {
                return;
            }
            // Wait for connection to be established (cursor row > 2)
            if let Some(row) = notebook_clone.get_terminal_cursor_row(session_id)
                && row > 2
            {
                recording_started_clone.set(true);
                notebook_clone.start_recording(
                    session_id,
                    &recording_conn_name,
                    rustconn_core::session::SanitizeConfig::default(),
                    recording_ssh_params.clone(),
                );
                tracing::info!(
                    %session_id,
                    "Auto-recording started after SSH connection established"
                );
            }
        });
    }

    // --- Deferred monitoring start: wait for SSH to connect before opening monitor ---
    if let Ok(state_ref) = state.try_borrow() {
        let settings = state_ref.settings().monitoring.clone();
        let mon_enabled = conn
            .monitoring_config
            .as_ref()
            .map_or(settings.enabled, |mc| mc.is_enabled(&settings));
        if mon_enabled {
            let effective = rustconn_core::MonitoringSettings {
                enabled: true,
                interval_secs: conn.monitoring_config.as_ref().map_or_else(
                    || settings.effective_interval_secs(),
                    |mc| mc.effective_interval(&settings),
                ),
                ..settings
            };
            let identity_file_mon = ssh_inheritance::resolve_ssh_key_path(conn, &groups)
                .and_then(|p| rustconn_core::resolve_key_path(&p))
                .map(|p| p.to_string_lossy().to_string());
            let cached_pw = state_ref
                .get_cached_credentials(connection_id)
                .and_then(|c| {
                    use secrecy::ExposeSecret;
                    let pw = c.password.expose_secret();
                    if pw.is_empty() {
                        None
                    } else {
                        Some(c.password.clone())
                    }
                });

            let monitoring_clone = Rc::clone(monitoring);
            let notebook_clone = notebook.clone();
            let mon_host = conn.host.clone();
            let mon_port = conn.port;
            let mon_username = conn.username.clone();
            let mon_jump_host = jump_host_chain.clone();
            let monitoring_started = std::rc::Rc::new(std::cell::Cell::new(false));
            let monitoring_started_clone = monitoring_started.clone();

            notebook.connect_contents_changed(session_id, move || {
                if monitoring_started_clone.get() {
                    return;
                }
                let Some(row) = notebook_clone.get_terminal_cursor_row(session_id) else {
                    return;
                };
                if row <= 2 {
                    return;
                }
                monitoring_started_clone.set(true);
                if let Some(container) = notebook_clone.get_session_container(session_id) {
                    monitoring_clone.start_monitoring(
                        session_id,
                        &container,
                        &effective,
                        &mon_host,
                        mon_port,
                        mon_username.as_deref(),
                        identity_file_mon.as_deref(),
                        cached_pw.clone(),
                        mon_jump_host.as_deref(),
                    );
                }
            });
        }
    }

    Some(session_id)
}

/// Returns `true` if reconnect was initiated, `false` if the tab no longer exists.
pub fn reconnect_ssh_in_place(
    state: &SharedAppState,
    notebook: &SharedNotebook,
    sidebar: &SharedSidebar,
    monitoring: &super::types::SharedMonitoring,
    session_id: Uuid,
    connection_id: Uuid,
) -> bool {
    use rustconn_core::protocol::{format_command_message, format_connection_message};

    // Prepare the existing tab for reconnect
    if !notebook.prepare_for_reconnect(session_id) {
        tracing::warn!(%session_id, "Tab no longer exists, cannot reconnect in-place");
        return false;
    }

    // Show "connecting" status in sidebar immediately
    sidebar.update_connection_status(&connection_id.to_string(), "connecting");

    // Get connection data
    let conn = {
        let Ok(state_ref) = state.try_borrow() else {
            return false;
        };
        match state_ref.get_connection(connection_id) {
            Some(c) => c.clone(),
            None => return false,
        }
    };

    // Re-apply highlight rules
    {
        let global_rules = state
            .try_borrow()
            .ok()
            .map(|s| s.settings().highlight_rules.clone())
            .unwrap_or_default();
        notebook.set_highlight_rules(session_id, &global_rules, &conn.highlight_rules);
    }

    // Record connection start in history
    let history_entry_id = if let Ok(mut state_mut) = state.try_borrow_mut() {
        Some(state_mut.record_connection_start(&conn, conn.username.as_deref()))
    } else {
        None
    };
    if let Some(entry_id) = history_entry_id {
        notebook.set_history_entry_id(session_id, entry_id);
    }

    // Get global variables for substitution
    let global_variables = state
        .try_borrow()
        .ok()
        .map(|s| crate::state::resolve_global_variables(s.settings()))
        .unwrap_or_default();

    let host = substitute_variables(&conn.host, &global_variables);
    let username = conn
        .username
        .as_ref()
        .map(|u| substitute_variables(u, &global_variables));

    // Collect groups for SSH inheritance resolution
    let groups: Vec<rustconn_core::ConnectionGroup> = state
        .try_borrow()
        .ok()
        .map(|s| s.list_groups().into_iter().cloned().collect())
        .unwrap_or_default();

    let has_jump_host = matches!(
        &conn.protocol_config,
        rustconn_core::ProtocolConfig::Ssh(ssh)
            if ssh.jump_host_id.is_some() || ssh.proxy_command.is_some()
    ) || ssh_inheritance::resolve_ssh_proxy_jump(&conn, &groups).is_some();

    // Build SSH args (shared with start_ssh_connection).
    let (identity_file, extra_args, use_waypipe, jump_host_chain, jump_host_password) =
        build_ssh_command_args(&conn, connection_id, state, &groups);

    // Re-wire child-exited handler for the new process
    MainWindow::setup_child_exited_handler(state, notebook, sidebar, session_id, connection_id);

    // Build SSH command string for display
    let port = conn.port;
    let mut ssh_cmd_parts = if use_waypipe {
        vec!["waypipe".to_string(), "ssh".to_string()]
    } else {
        vec!["ssh".to_string()]
    };
    if port != 22 {
        ssh_cmd_parts.push("-p".to_string());
        ssh_cmd_parts.push(port.to_string());
    }
    if let Some(ref key) = identity_file {
        ssh_cmd_parts.push("-i".to_string());
        ssh_cmd_parts.push(key.clone());
    }
    ssh_cmd_parts.extend(extra_args.clone());
    let destination = if let Some(ref user) = username {
        format!("{user}@{host}")
    } else {
        host.clone()
    };
    ssh_cmd_parts.push(destination);
    let ssh_command = ssh_cmd_parts.join(" ");

    // Display CLI output feedback
    let conn_msg = format_connection_message("SSH", &host);
    let cmd_msg = format_command_message(&ssh_command);
    let feedback = format!("{conn_msg}\r\n{cmd_msg}\r\n\r\n");
    notebook.display_output(session_id, &feedback);

    // Retrieve cached credentials
    let cached_password: Option<SecretString> = state
        .try_borrow()
        .ok()
        .and_then(|s| s.get_cached_credentials(connection_id).cloned())
        .and_then(|c| {
            use secrecy::ExposeSecret;
            let pw = c.password.expose_secret();
            if pw.is_empty() {
                None
            } else {
                Some(c.password.clone())
            }
        });

    // Spawn SSH in the existing terminal
    {
        let extra_refs: Vec<&str> = extra_args.iter().map(std::string::String::as_str).collect();
        let agent_socket = ssh_inheritance::resolve_ssh_agent_socket(&conn, &groups);
        let startup_cmd = match &conn.protocol_config {
            rustconn_core::ProtocolConfig::Ssh(cfg) => cfg.startup_command.as_deref(),
            _ => None,
        };
        // Jump host password (issue #191) — see start_ssh_connection_internal.
        let jump_host_env = jump_host_password.as_ref().map(|pw| {
            use secrecy::ExposeSecret;
            zeroize::Zeroizing::new(format!("{JUMP_HOST_PW_ENV}={}", pw.expose_secret()))
        });
        let extra_env = jump_host_env.as_ref().map(|e| [e.as_str()]);
        notebook.spawn_ssh(
            session_id,
            &host,
            port,
            username.as_deref(),
            identity_file.as_deref(),
            &extra_refs,
            use_waypipe,
            agent_socket.as_deref(),
            startup_cmd,
            extra_env.as_ref().map(<[&str; 1]>::as_slice),
        );
    }

    // VTE password injection (issue #194: also subscribe to cursor-moved)
    // NOTE: Passphrase prompts ("Enter passphrase for key") are explicitly
    // excluded to avoid sending the wrong secret when SSH auth is PublicKey.
    if let Some(vault_password) = cached_password {
        let password_sent = std::rc::Rc::new(std::cell::Cell::new(false));

        let check_and_inject = {
            let notebook_clone = notebook.clone();
            let password_sent = password_sent.clone();
            let vault_password = vault_password.clone();
            std::rc::Rc::new(move || {
                if password_sent.get() {
                    return;
                }
                let Some(text) = notebook_clone.get_terminal_text(session_id) else {
                    return;
                };

                if detect_password_prompt(&text) {
                    use secrecy::ExposeSecret;
                    let input =
                        zeroize::Zeroizing::new(format!("{}\n", vault_password.expose_secret()));
                    notebook_clone.send_text_to_session(session_id, &input);
                    password_sent.set(true);
                }
            })
        };

        let on_contents_changed = check_and_inject.clone();
        notebook.connect_contents_changed(session_id, move || on_contents_changed());

        let on_cursor_moved = check_and_inject;
        notebook.connect_cursor_moved(session_id, move || on_cursor_moved());
    }

    // SSH status detection
    {
        let sidebar_clone = sidebar.clone();
        let notebook_clone = notebook.clone();
        let connection_id_str = connection_id.to_string();
        let session_connected = std::rc::Rc::new(std::cell::Cell::new(false));
        let session_connected_clone = session_connected.clone();
        let uses_jump_host = has_jump_host;

        notebook.connect_contents_changed(session_id, move || {
            if session_connected_clone.get() {
                return;
            }
            if let Some(row) = notebook_clone.get_terminal_cursor_row(session_id)
                && row > 2
            {
                if uses_jump_host
                    && let Some(text) = notebook_clone.get_terminal_text(session_id)
                    && contains_ssh_failure(&text)
                {
                    return;
                }
                sidebar_clone.increment_session_count(&connection_id_str);
                session_connected_clone.set(true);
            }
        });
    }

    // Deferred monitoring start
    if let Ok(state_ref) = state.try_borrow() {
        let settings = state_ref.settings().monitoring.clone();
        let mon_enabled = conn
            .monitoring_config
            .as_ref()
            .map_or(settings.enabled, |mc| mc.is_enabled(&settings));
        if mon_enabled {
            let effective = rustconn_core::MonitoringSettings {
                enabled: true,
                interval_secs: conn.monitoring_config.as_ref().map_or_else(
                    || settings.effective_interval_secs(),
                    |mc| mc.effective_interval(&settings),
                ),
                ..settings
            };
            let identity_file_mon = ssh_inheritance::resolve_ssh_key_path(&conn, &groups)
                .and_then(|p| rustconn_core::resolve_key_path(&p))
                .map(|p| p.to_string_lossy().to_string());
            let cached_pw = state_ref
                .get_cached_credentials(connection_id)
                .and_then(|c| {
                    use secrecy::ExposeSecret;
                    let pw = c.password.expose_secret();
                    if pw.is_empty() {
                        None
                    } else {
                        Some(c.password.clone())
                    }
                });

            let monitoring_clone = Rc::clone(monitoring);
            let notebook_clone = notebook.clone();
            let mon_host = conn.host.clone();
            let mon_port = conn.port;
            let mon_username = conn.username.clone();
            let mon_jump_host = jump_host_chain;
            let monitoring_started = std::rc::Rc::new(std::cell::Cell::new(false));
            let monitoring_started_clone = monitoring_started.clone();

            notebook.connect_contents_changed(session_id, move || {
                if monitoring_started_clone.get() {
                    return;
                }
                let Some(row) = notebook_clone.get_terminal_cursor_row(session_id) else {
                    return;
                };
                if row <= 2 {
                    return;
                }
                monitoring_started_clone.set(true);
                if let Some(container) = notebook_clone.get_session_container(session_id) {
                    monitoring_clone.start_monitoring(
                        session_id,
                        &container,
                        &effective,
                        &mon_host,
                        mon_port,
                        mon_username.as_deref(),
                        identity_file_mon.as_deref(),
                        cached_pw.clone(),
                        mon_jump_host.as_deref(),
                    );
                }
            });
        }
    }

    // Update last_connected timestamp
    if let Ok(mut state_mut) = state.try_borrow_mut()
        && let Err(e) = state_mut.update_last_connected(connection_id)
    {
        tracing::warn!(?e, "Failed to update last_connected");
    }

    true
}
