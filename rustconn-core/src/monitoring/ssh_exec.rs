//! SSH command execution for monitoring
//!
//! Runs monitoring commands on remote hosts via `ssh` with `SSH_ASKPASS`
//! for password-authenticated connections. This uses a separate SSH process
//! (not the VTE terminal session) to avoid interfering with the user's
//! interactive shell.
//!
//! Password authentication uses the `SSH_ASKPASS` mechanism instead of
//! `sshpass`: a temporary script echoes the password from an environment
//! variable, and `SSH_ASKPASS_REQUIRE=force` tells OpenSSH to use it
//! even without a TTY. This eliminates the `sshpass` external dependency.

use std::sync::Arc;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use tokio::process::Command;

/// Default timeout for SSH monitoring commands (seconds)
const SSH_EXEC_TIMEOUT_SECS: u64 = 10;

/// Environment variable name used to pass the password to the askpass script.
/// Intentionally obscure to reduce exposure in `/proc/PID/environ`.
const ASKPASS_ENV_VAR: &str = "_RC_MON_PW";

/// RAII wrapper for the temporary `SSH_ASKPASS` script.
///
/// Deletes the script file when the last `Arc<AskpassScript>` reference is dropped
/// (i.e. when the monitoring session ends and the factory closure is freed).
struct AskpassScript(std::path::PathBuf);

impl Drop for AskpassScript {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.0) {
            tracing::debug!(
                path = %self.0.display(),
                error = %e,
                "Failed to clean up askpass script"
            );
        }
    }
}

/// Creates a temporary `SSH_ASKPASS` helper script that echoes the password
/// from `ASKPASS_ENV_VAR`. The script is created with mode 0700 and lives
/// in the system temp directory.
///
/// Returns the path to the script on success.
fn create_askpass_script() -> Result<std::path::PathBuf, String> {
    use std::io::Write;

    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "rc-askpass-{}",
        uuid::Uuid::new_v4().as_hyphenated()
    ));

    let script = format!("#!/bin/sh\necho \"${ASKPASS_ENV_VAR}\"\n");

    let mut file = std::fs::File::create(&path)
        .map_err(|e| format!("Failed to create askpass script: {e}"))?;
    file.write_all(script.as_bytes())
        .map_err(|e| format!("Failed to write askpass script: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("Failed to set askpass script permissions: {e}"))?;
    }

    Ok(path)
}

/// Builds jump host arguments for the SSH monitoring command.
///
/// In Flatpak, `-J` (ProxyJump) spawns a nested SSH process that does NOT
/// inherit `-o` flags from the outer command. The jump host SSH tries to
/// write to `~/.ssh/known_hosts` (read-only in Flatpak) and prompts for
/// host key verification. This function replaces `-J` with a `ProxyCommand`
/// that passes `StrictHostKeyChecking` and `UserKnownHostsFile` to the
/// jump host SSH process.
///
/// Outside Flatpak, standard `-J` is used.
fn build_jump_host_args(cmd: &mut Command, jump_host: &str, identity_file: Option<&str>) {
    let flatpak_kh = crate::flatpak::get_flatpak_known_hosts_path();
    if flatpak_kh.is_some() {
        let mut proxy_parts = vec![
            "ssh".to_string(),
            "-W".to_string(),
            "%h:%p".to_string(),
            "-o".to_string(),
            "StrictHostKeyChecking=accept-new".to_string(),
        ];
        if let Some(ref kh_path) = flatpak_kh {
            proxy_parts.push("-o".to_string());
            proxy_parts.push(format!("UserKnownHostsFile={}", kh_path.display()));
        }
        if let Some(key) = identity_file {
            proxy_parts.push("-i".to_string());
            proxy_parts.push(key.to_string());
            proxy_parts.push("-o".to_string());
            proxy_parts.push("IdentitiesOnly=yes".to_string());
        }
        proxy_parts.push(jump_host.to_string());
        let proxy_cmd = proxy_parts.join(" ");
        tracing::debug!(
            protocol = "ssh",
            proxy_command = %proxy_cmd,
            "Monitoring: using ProxyCommand instead of -J for Flatpak compatibility"
        );
        cmd.arg("-o").arg(format!("ProxyCommand={proxy_cmd}"));
    } else {
        cmd.arg("-J").arg(jump_host);
    }
}

/// Builds an SSH exec closure for use with [`super::start_collector`].
///
/// The returned closure spawns `ssh` with the given host/port/user and
/// executes the provided shell command, returning stdout as a `String`.
///
/// When a password is provided, the `SSH_ASKPASS` mechanism is used:
/// a temporary script echoes the password from an environment variable,
/// and `SSH_ASKPASS_REQUIRE=force` tells OpenSSH to invoke it. This
/// replaces the previous `sshpass` dependency.
///
/// # Arguments
/// * `host` - Remote hostname or IP
/// * `port` - SSH port
/// * `username` - Optional SSH username
/// * `identity_file` - Optional path to SSH private key
/// * `password` - Optional password (as `SecretString`) for SSH_ASKPASS auth
/// * `jump_host` - Optional jump host chain for `-J` flag (e.g. `"user@bastion:22"`)
pub fn ssh_exec_factory(
    host: String,
    port: u16,
    username: Option<String>,
    identity_file: Option<String>,
    password: Option<SecretString>,
    jump_host: Option<String>,
) -> impl Fn(
    String,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>
+ Send
+ 'static {
    // Create the askpass script once at factory creation time.
    // It is reused for every monitoring command invocation.
    // Wrapped in Arc<AskpassScript> so the file is deleted when the factory is dropped.
    let askpass_script = if password.is_some() {
        match create_askpass_script() {
            Ok(p) => Some(Arc::new(AskpassScript(p))),
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "Failed to create SSH_ASKPASS script; \
                     password auth will not work for monitoring"
                );
                None
            }
        }
    } else {
        None
    };

    move |command: String| {
        let host = host.clone();
        let username = username.clone();
        let identity_file = identity_file.clone();
        let password = password.clone();
        let jump_host = jump_host.clone();
        let askpass_script = askpass_script.clone();

        Box::pin(async move {
            let mut cmd = Command::new("ssh");

            if let (Some(pw), Some(script)) = (&password, &askpass_script) {
                // SSH_ASKPASS mechanism: OpenSSH calls the script to get
                // the password. DISPLAY must be set (even empty) and
                // SSH_ASKPASS_REQUIRE=force skips the TTY check.
                cmd.env("SSH_ASKPASS", &script.0);
                cmd.env("SSH_ASKPASS_REQUIRE", "force");
                cmd.env(ASKPASS_ENV_VAR, pw.expose_secret());
                // Ensure DISPLAY is set so SSH considers ASKPASS
                if std::env::var("DISPLAY").is_err() {
                    cmd.env("DISPLAY", "");
                }
            } else if password.is_none() {
                // Batch mode only when NOT using password auth
                cmd.arg("-o").arg("BatchMode=yes");
            }

            // Accept new host keys but reject changed ones (OpenSSH 7.6+).
            // Using `accept-new` instead of `no` prevents MITM attacks on
            // hosts whose key has changed while still allowing first-time
            // connections without manual intervention.
            cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");

            // In Flatpak, ~/.ssh is read-only — use writable known_hosts path
            if let Some(kh_path) = crate::flatpak::get_flatpak_known_hosts_path() {
                let kh_opt = format!("UserKnownHostsFile={}", kh_path.display());
                cmd.arg("-o").arg(kh_opt);
            }

            // Short connection timeout
            cmd.arg("-o").arg("ConnectTimeout=5");

            // Jump host chain for tunneled connections
            if let Some(ref jh) = jump_host {
                build_jump_host_args(&mut cmd, jh, identity_file.as_deref());
            }

            if port != 22 {
                cmd.arg("-p").arg(port.to_string());
            }

            if let Some(ref key) = identity_file {
                cmd.arg("-i").arg(key);
            }

            let destination = if let Some(ref user) = username {
                format!("{user}@{host}")
            } else {
                host.clone()
            };
            cmd.arg(&destination);
            cmd.arg(&command);

            // Suppress stderr to avoid noise
            cmd.stderr(std::process::Stdio::piped());
            cmd.stdout(std::process::Stdio::piped());

            let timeout = Duration::from_secs(SSH_EXEC_TIMEOUT_SECS);

            match tokio::time::timeout(timeout, cmd.output()).await {
                Ok(Ok(output)) => {
                    if output.status.success() {
                        String::from_utf8(output.stdout)
                            .map_err(|e| format!("Invalid UTF-8 in SSH output: {e}"))
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        Err(format!(
                            "SSH command failed (exit {}): {}",
                            output.status,
                            stderr.trim()
                        ))
                    }
                }
                Ok(Err(e)) => Err(format!("Failed to spawn SSH process: {e}")),
                Err(_) => Err(format!(
                    "SSH monitoring command timed out after {SSH_EXEC_TIMEOUT_SECS}s"
                )),
            }
        })
    }
}
