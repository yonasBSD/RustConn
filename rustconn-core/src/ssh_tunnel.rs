//! SSH tunnel for forwarding connections through a jump host.
//!
//! Used by RDP, VNC, SPICE, and Telnet connections that have a
//! `jump_host_id` configured. Creates an `ssh -L` local port forward
//! in the background and returns the local port for the client to
//! connect to.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

/// Errors that can occur when creating an SSH tunnel.
#[derive(Debug, Error)]
pub enum SshTunnelError {
    /// No free local port could be found.
    #[error("Could not find a free local port")]
    NoFreePort,
    /// Failed to spawn the SSH process.
    #[error("Failed to spawn SSH tunnel: {0}")]
    SpawnFailed(#[from] std::io::Error),
}

/// Result type for SSH tunnel operations.
pub type SshTunnelResult<T> = Result<T, SshTunnelError>;

/// A running SSH tunnel (`ssh -N -L ...`).
///
/// The tunnel process is killed when this struct is dropped.
/// If a temporary askpass script was created, it is zeroized and deleted.
pub struct SshTunnel {
    /// The child SSH process.
    child: Child,
    /// The local port that forwards to the remote destination.
    local_port: u16,
    /// Captured stderr output from the SSH process (populated by background reader).
    stderr_output: Arc<Mutex<String>>,
    /// Path to the temporary askpass script (cleaned up on drop).
    askpass_script: Option<std::path::PathBuf>,
}

impl SshTunnel {
    /// Returns the local port to connect to.
    #[must_use]
    pub const fn local_port(&self) -> u16 {
        self.local_port
    }

    /// Checks whether the SSH tunnel process is still running.
    ///
    /// Returns `true` if the process is alive, `false` if it has exited.
    /// When the process has exited, any captured stderr is logged.
    pub fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            Ok(Some(status)) => {
                let stderr = self
                    .stderr_output
                    .lock()
                    .map(|s| s.clone())
                    .unwrap_or_default();
                if stderr.is_empty() {
                    tracing::error!(
                        local_port = self.local_port,
                        %status,
                        "SSH tunnel process exited"
                    );
                } else {
                    tracing::error!(
                        local_port = self.local_port,
                        %status,
                        stderr = %stderr.trim(),
                        "SSH tunnel process exited"
                    );
                }
                false
            }
            Err(e) => {
                tracing::error!(
                    local_port = self.local_port,
                    %e,
                    "Failed to check SSH tunnel process status"
                );
                false
            }
        }
    }

    /// Returns any captured stderr output from the SSH process.
    #[must_use]
    pub fn stderr(&self) -> String {
        self.stderr_output
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    /// Stops the tunnel by killing the SSH process.
    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        self.stop();
        if let Some(ref path) = self.askpass_script {
            cleanup_askpass_script(path);
        }
    }
}

/// Parameters for creating an SSH tunnel.
pub struct SshTunnelParams {
    /// Jump host address (e.g. `user@bastion.example.com`).
    pub jump_host: String,
    /// Jump host SSH port (default 22).
    pub jump_port: u16,
    /// Remote destination host (the actual RDP/VNC/SPICE server).
    pub remote_host: String,
    /// Remote destination port.
    pub remote_port: u16,
    /// Optional SSH identity file for the jump host.
    pub identity_file: Option<String>,
    /// Optional password for the jump host (used via `SSH_ASKPASS`).
    ///
    /// When set, a temporary askpass helper script is created and
    /// `SSH_ASKPASS_REQUIRE=force` is used so OpenSSH calls the script
    /// instead of prompting on a TTY. `BatchMode` is NOT set in this case.
    pub password: Option<SecretString>,
    /// Optional extra SSH args (e.g. `-o StrictHostKeyChecking=no`).
    pub extra_args: Vec<String>,
}

/// Environment variable name used to pass the password to the askpass script.
/// Intentionally obscure to reduce exposure in `/proc/PID/environ`.
const TUNNEL_ASKPASS_ENV_VAR: &str = "_RC_TUN_PW";

/// Creates a temporary `SSH_ASKPASS` helper script that echoes the password
/// from [`TUNNEL_ASKPASS_ENV_VAR`]. The script is created with mode 0700.
///
/// # Errors
///
/// Returns a human-readable error string on failure.
fn create_tunnel_askpass_script() -> Result<std::path::PathBuf, String> {
    use std::io::Write;

    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "rc-tun-askpass-{}",
        uuid::Uuid::new_v4().as_hyphenated()
    ));

    let script = format!("#!/bin/sh\necho \"${TUNNEL_ASKPASS_ENV_VAR}\"\n");

    let mut file = std::fs::File::create(&path)
        .map_err(|e| format!("Failed to create tunnel askpass script: {e}"))?;
    file.write_all(script.as_bytes())
        .map_err(|e| format!("Failed to write tunnel askpass script: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("Failed to set tunnel askpass script permissions: {e}"))?;
    }

    Ok(path)
}

/// Cleans up a temporary askpass script, zeroizing its content first.
fn cleanup_askpass_script(path: &std::path::Path) {
    // Overwrite with zeros before deletion to prevent recovery
    if let Ok(metadata) = std::fs::metadata(path) {
        let size = metadata.len() as usize;
        if size > 0 {
            let _ = std::fs::write(path, vec![0u8; size]);
        }
    }
    let _ = std::fs::remove_file(path);
}

/// Finds a free TCP port by binding to port 0 and reading the assigned port.
///
/// # Errors
///
/// Returns `SshTunnelError::NoFreePort` if binding fails.
pub fn find_free_port() -> SshTunnelResult<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| SshTunnelError::NoFreePort)?;
    let port = listener
        .local_addr()
        .map_err(|_| SshTunnelError::NoFreePort)?
        .port();
    // Drop the listener so the port is released before SSH binds to it.
    // There is a small TOCTOU window, but it is acceptable for this use case.
    drop(listener);
    Ok(port)
}

/// Creates an SSH tunnel by spawning `ssh -N -L local_port:remote:remote_port`.
///
/// The tunnel runs in the background. The caller must keep the returned
/// [`SshTunnel`] alive for the duration of the connection — dropping it
/// kills the SSH process.
///
/// # Errors
///
/// Returns an error if no free port is found or the SSH process fails to spawn.
pub fn create_tunnel(params: &SshTunnelParams) -> SshTunnelResult<SshTunnel> {
    let local_port = find_free_port()?;

    let forward_spec = format!(
        "{}:{}:{}",
        local_port, params.remote_host, params.remote_port
    );

    let mut cmd = Command::new("ssh");
    cmd.arg("-N") // No remote command — just forward
        .arg("-L")
        .arg(&forward_spec);

    // Jump host port
    if params.jump_port != 22 {
        cmd.arg("-p").arg(params.jump_port.to_string());
    }

    // Identity file
    if let Some(ref key) = params.identity_file {
        cmd.arg("-i").arg(key);
    }

    // Extra args
    for arg in &params.extra_args {
        cmd.arg(arg);
    }

    // Flatpak writable known_hosts
    if let Some(kh_path) = crate::get_flatpak_known_hosts_path() {
        cmd.arg("-o")
            .arg(format!("UserKnownHostsFile={}", kh_path.display()));
    }

    // SSH_ASKPASS for password-authenticated jump hosts, or BatchMode
    // when no password is available (prevents SSH from hanging on a
    // TTY prompt that nobody can answer).
    let askpass_script_path = if let Some(ref pw) = params.password {
        match create_tunnel_askpass_script() {
            Ok(script_path) => {
                cmd.env("SSH_ASKPASS", &script_path);
                cmd.env("SSH_ASKPASS_REQUIRE", "force");
                cmd.env(TUNNEL_ASKPASS_ENV_VAR, pw.expose_secret());
                // Ensure DISPLAY is set so SSH considers ASKPASS
                if std::env::var("DISPLAY").is_err() {
                    cmd.env("DISPLAY", "");
                }
                Some(script_path)
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "Failed to create SSH_ASKPASS script for tunnel; \
                     falling back to BatchMode (password auth will fail)"
                );
                cmd.arg("-o").arg("BatchMode=yes");
                None
            }
        }
    } else {
        // No password — prevent SSH from reading stdin
        cmd.arg("-o").arg("BatchMode=yes");
        None
    };

    // Exit if the forwarding fails (e.g. port already in use)
    cmd.arg("-o").arg("ExitOnForwardFailure=yes");

    // The jump host destination
    cmd.arg(&params.jump_host);

    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    tracing::info!(
        local_port,
        remote = %format!("{}:{}", params.remote_host, params.remote_port),
        jump_host = %params.jump_host,
        "Starting SSH tunnel"
    );

    let mut child = cmd.spawn()?;

    // Capture SSH stderr in a background thread so diagnostic messages
    // (auth failures, port unreachable, etc.) are available for logging.
    let stderr_output = Arc::new(Mutex::new(String::new()));
    if let Some(stderr_handle) = child.stderr.take() {
        let stderr_buf = stderr_output.clone();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(stderr_handle);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        tracing::warn!(target: "ssh_tunnel", "{}", line);
                        if let Ok(mut buf) = stderr_buf.lock() {
                            if !buf.is_empty() {
                                buf.push('\n');
                            }
                            buf.push_str(&line);
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    Ok(SshTunnel {
        child,
        local_port,
        stderr_output,
        askpass_script: askpass_script_path,
    })
}

/// Waits for the SSH tunnel to become ready by polling the local port.
///
/// Tries to connect to `127.0.0.1:local_port` up to `max_attempts` times
/// with `interval` between attempts. Also checks that the SSH process is
/// still alive between attempts. Returns `Ok(())` when the port
/// accepts connections, or `Err` if all attempts fail or the process exits.
///
/// # Errors
///
/// Returns `SshTunnelError::SpawnFailed` if the tunnel never becomes ready
/// or the SSH process exits prematurely.
pub fn wait_for_tunnel_ready(
    tunnel: &mut SshTunnel,
    max_attempts: u32,
    interval: std::time::Duration,
) -> SshTunnelResult<()> {
    use std::net::TcpStream;

    let local_port = tunnel.local_port;

    for attempt in 1..=max_attempts {
        // Check if SSH process is still alive before trying to connect
        if !tunnel.is_alive() {
            let stderr = tunnel.stderr();
            let detail = if stderr.is_empty() {
                "SSH process exited unexpectedly".to_string()
            } else {
                format!("SSH process exited: {}", stderr.trim())
            };
            return Err(SshTunnelError::SpawnFailed(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                detail,
            )));
        }

        match TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], local_port)),
            std::time::Duration::from_secs(1),
        ) {
            Ok(_) => {
                tracing::debug!(local_port, attempt, "SSH tunnel is ready");
                return Ok(());
            }
            Err(_) => {
                if attempt < max_attempts {
                    std::thread::sleep(interval);
                }
            }
        }
    }

    Err(SshTunnelError::SpawnFailed(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        format!("SSH tunnel on port {local_port} not ready after {max_attempts} attempts"),
    )))
}

/// Probes the remote endpoint through an established SSH tunnel.
///
/// Connects to the tunnel's local port and waits for the remote end to
/// respond within `timeout`. If the remote host/port is unreachable
/// (firewall, service down), the connection will either be refused or
/// time out.
///
/// Returns `Ok(())` if the remote end accepts the connection, or an
/// error describing why it failed.
///
/// # Errors
///
/// Returns `SshTunnelError::SpawnFailed` if the remote port is unreachable
/// or the SSH tunnel process has exited.
pub fn probe_tunnel_remote(
    tunnel: &mut SshTunnel,
    timeout: std::time::Duration,
) -> SshTunnelResult<()> {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    // First check the tunnel process is still alive
    if !tunnel.is_alive() {
        let stderr = tunnel.stderr();
        let detail = if stderr.is_empty() {
            "SSH tunnel process exited before probe".to_string()
        } else {
            format!("SSH tunnel exited: {}", stderr.trim())
        };
        return Err(SshTunnelError::SpawnFailed(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            detail,
        )));
    }

    let local_port = tunnel.local_port;
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], local_port));

    // Connect to the tunnel's local port
    let mut stream = TcpStream::connect_timeout(&addr, timeout).map_err(|e| {
        SshTunnelError::SpawnFailed(std::io::Error::new(
            e.kind(),
            format!("Cannot connect to tunnel port {local_port}: {e}"),
        ))
    })?;

    // Set read/write timeouts for the probe
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));

    // Send a minimal probe byte and wait for any response or error.
    // For RDP (port 3389), the server responds to any data with an
    // X.224 Connection Confirm or rejects the connection. If the
    // remote port is unreachable, SSH will close the forwarded
    // channel and we'll get a connection reset or EOF.
    //
    // We send a single zero byte — this is enough to trigger SSH
    // channel forwarding to the remote host. If the remote host is
    // unreachable, SSH will close the local socket.
    let _ = stream.write_all(&[0]);
    let _ = stream.flush();

    // Give SSH time to forward and detect unreachable remote
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Try to read — if the remote is unreachable, SSH will have
    // closed the connection and we'll get an error or EOF.
    let mut buf = [0u8; 1];
    match stream.read(&mut buf) {
        Ok(0) => {
            // EOF — SSH closed the forwarded channel (remote unreachable)
            Err(SshTunnelError::SpawnFailed(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                format!(
                    "Remote port unreachable through SSH tunnel (port {local_port}): \
                     connection closed by tunnel"
                ),
            )))
        }
        Ok(_) => {
            // Got data back — remote is alive and responding
            tracing::debug!(local_port, "Remote endpoint is reachable through tunnel");
            Ok(())
        }
        Err(ref e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
        {
            // Read timed out — the remote accepted the connection but
            // hasn't sent data yet. This is normal for many protocols
            // (they wait for a proper handshake). The important thing
            // is that SSH didn't close the channel, so the remote is
            // reachable.
            tracing::debug!(
                local_port,
                "Remote endpoint accepted connection through tunnel (read timed out, which is OK)"
            );
            Ok(())
        }
        Err(e) => {
            // Connection reset, broken pipe, etc. — remote unreachable
            Err(SshTunnelError::SpawnFailed(std::io::Error::new(
                e.kind(),
                format!("Remote port unreachable through SSH tunnel (port {local_port}): {e}"),
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_free_port() {
        let port = find_free_port().expect("should find a free port");
        assert!(port > 0);
        // Verify the port is actually free by binding to it
        let listener = TcpListener::bind(format!("127.0.0.1:{port}"));
        assert!(listener.is_ok(), "port {port} should be bindable");
    }

    #[test]
    fn test_find_free_port_unique() {
        let p1 = find_free_port().expect("port 1");
        let p2 = find_free_port().expect("port 2");
        // Ports should be different (extremely likely, not guaranteed)
        // This is a probabilistic test — skip assertion if they happen to match
        if p1 == p2 {
            eprintln!("Warning: two consecutive find_free_port() returned the same port {p1}");
        }
    }
}
