//! Isolated FFI helper for RustConn's macOS native PTY.
//!
//! The GUI process is typically launched without a controlling terminal
//! (e.g. from the macOS Finder / LaunchServices). A child spawned into a
//! pseudo-terminal therefore has no controlling terminal either, so
//! interactive programs such as `ssh` cannot open `/dev/tty` to read a
//! password prompt and fail immediately with "Permission denied"
//! ([#175](https://github.com/totoshko88/RustConn/issues/175)).
//!
//! This crate exposes a single safe function that registers a `pre_exec`
//! hook so the child claims its PTY slave as a controlling terminal. It is
//! the workspace's only `unsafe` code, kept tiny and documented per the
//! project's `M-UNSAFE` guideline (isolate FFI in a `-sys` crate) instead of
//! relaxing `unsafe_code = "forbid"` in the main crates.

#[cfg(unix)]
mod imp {
    use std::io;
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    /// Arranges for `cmd`'s child to acquire its standard input terminal as a
    /// controlling terminal.
    ///
    /// The child is placed in a new session via `setsid(2)` and then claims
    /// the terminal on file descriptor 0 with the `TIOCSCTTY` ioctl. This lets
    /// interactive programs (notably `ssh`) open `/dev/tty` to prompt for a
    /// password. Without it, a child of a GUI process that has no controlling
    /// terminal cannot read the password and authentication fails instantly.
    ///
    /// # Preconditions
    ///
    /// * The caller MUST connect a PTY slave to the child's stdin (fd 0)
    ///   before spawning (e.g. via [`Command::stdin`]).
    /// * The caller MUST NOT also set [`CommandExt::process_group`]: `setsid(2)`
    ///   fails with `EPERM` when the calling process is already a process-group
    ///   leader. `setsid(2)` already makes the child a session and
    ///   process-group leader, which is sufficient for job control
    ///   (`Ctrl-C` → `SIGINT` to the foreground group).
    pub fn set_controlling_terminal(cmd: &mut Command) {
        // SAFETY: the registered hook runs in the forked child, after `std`
        // has wired up the stdio descriptors and before `execvp`. It calls
        // only async-signal-safe libc functions (`setsid`, `ioctl`) and does
        // not allocate, lock, or touch shared state, satisfying the contract
        // of `CommandExt::pre_exec`.
        unsafe {
            cmd.pre_exec(|| {
                // New session: detach from any inherited controlling terminal
                // and become a session + process-group leader.
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                // Claim fd 0 (the PTY slave) as the controlling terminal.
                if libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
}

#[cfg(unix)]
pub use imp::set_controlling_terminal;

#[cfg(all(unix, test))]
mod tests {
    use std::process::{Command, Stdio};

    /// Contract test (M-UNSAFE): proves the `pre_exec` hook is actually
    /// registered and runs in the forked child.
    ///
    /// Miri cannot execute `setsid`/`ioctl`, so we verify the contract
    /// observably instead: with stdin redirected to `/dev/null` (never a
    /// terminal), the hook's `setsid()` succeeds but `ioctl(0, TIOCSCTTY)`
    /// fails with `ENOTTY`, returning `Err` from the closure — which makes
    /// `spawn()` fail. If the hook were not wired, `true` would spawn fine.
    #[test]
    fn pre_exec_hook_runs_and_fails_without_a_tty() {
        let mut cmd = Command::new("true");
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        super::set_controlling_terminal(&mut cmd);

        let result = cmd.spawn();
        assert!(
            result.is_err(),
            "spawn should fail: TIOCSCTTY on a /dev/null stdin returns an error, \
             proving the pre_exec hook executed in the child",
        );
    }
}
