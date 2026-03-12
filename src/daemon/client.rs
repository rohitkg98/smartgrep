use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::Duration;

use anyhow::{Context, Result};

use super::protocol::{Request, Response};

/// Derive the Unix socket path for a project root.
/// Uses a hash of the canonical project root path so multiple projects
/// get independent daemons.
pub fn socket_path(project_root: &Path) -> PathBuf {
    let canonical = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let hash = {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(canonical.to_string_lossy().as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)[..16].to_string()
    };
    PathBuf::from(format!("/tmp/smartgrep-{}.sock", hash))
}

/// Derive the PID file path for a project root.
pub fn pid_path(project_root: &Path) -> PathBuf {
    let sock = socket_path(project_root);
    sock.with_extension("pid")
}

/// Check if a daemon is running for the given project root.
pub fn is_running(project_root: &Path) -> bool {
    let sock = socket_path(project_root);
    if !sock.exists() {
        return false;
    }

    // Try to connect and send a ping
    match ping(project_root) {
        Ok(_) => true,
        Err(_) => {
            // Stale socket; clean up
            let _ = std::fs::remove_file(&sock);
            let _ = std::fs::remove_file(pid_path(project_root));
            false
        }
    }
}

/// Send a ping to the daemon and return the response.
pub fn ping(project_root: &Path) -> Result<Response> {
    send_request(
        project_root,
        &Request {
            command: "ping".to_string(),
            args: String::new(),
            format: "text".to_string(),
        },
    )
}

/// Send a request to the daemon and return the response.
pub fn send_request(project_root: &Path, request: &Request) -> Result<Response> {
    let sock = socket_path(project_root);

    let mut stream = UnixStream::connect(&sock)
        .with_context(|| format!("Cannot connect to daemon socket at {}", sock.display()))?;

    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    // Send request as a single JSON line
    let mut payload = serde_json::to_string(request)?;
    payload.push('\n');
    stream.write_all(payload.as_bytes())?;
    stream.flush()?;

    // Read response (single JSON line)
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let response: Response = serde_json::from_str(&line)
        .with_context(|| format!("Invalid response from daemon: {}", line.trim()))?;

    Ok(response)
}

/// Silently ensure a daemon is running for the given project root.
/// Spawns one in the background if needed. Returns Ok(()) on success,
/// Err if auto-start failed (caller should fall back to direct execution).
pub fn ensure_daemon(project_root: &Path) -> Result<()> {
    if is_running(project_root) {
        return Ok(());
    }

    // Get the path to our own executable
    let exe = std::env::current_exe()
        .context("Cannot determine executable path")?;

    // Spawn the daemon as a detached background process using the hidden
    // `run-server` subcommand.
    let _child = ProcessCommand::new(&exe)
        .arg("--project-root")
        .arg(project_root)
        .arg("run-server")
        .arg("--idle-timeout")
        .arg("1800")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn daemon process")?;

    // Wait briefly for the daemon to become ready
    let sock = socket_path(project_root);
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if sock.exists() {
            if ping(project_root).is_ok() {
                return Ok(());
            }
        }
    }

    anyhow::bail!("Daemon spawned but not responding after 5 seconds")
}

/// Try to execute a command via the daemon. If `no_daemon` is true, skips
/// entirely. Otherwise:
///   1. Try connecting to an existing daemon
///   2. If no daemon, auto-start one silently, then retry
///   3. If anything fails, return None (caller falls back to direct execution)
pub fn try_daemon(
    project_root: &Path,
    command: &str,
    args: &str,
    format: &str,
    no_daemon: bool,
) -> Option<String> {
    if no_daemon {
        return None;
    }

    let request = Request {
        command: command.to_string(),
        args: args.to_string(),
        format: format.to_string(),
    };

    // First try: connect to existing daemon
    if socket_path(project_root).exists() {
        if let Ok(resp) = send_request(project_root, &request) {
            if resp.status == "ok" {
                return resp.output;
            }
        }
    }

    // Second try: auto-start daemon, then connect
    if ensure_daemon(project_root).is_ok() {
        if let Ok(resp) = send_request(project_root, &request) {
            if resp.status == "ok" {
                return resp.output;
            }
        }
    }

    // Fall back to direct execution
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_path_deterministic() {
        let root = PathBuf::from("/tmp/test-project");
        let p1 = socket_path(&root);
        let p2 = socket_path(&root);
        assert_eq!(p1, p2);
        assert!(p1.to_string_lossy().starts_with("/tmp/smartgrep-"));
        assert!(p1.to_string_lossy().ends_with(".sock"));
    }

    #[test]
    fn test_pid_path() {
        let root = PathBuf::from("/tmp/test-project");
        let pid = pid_path(&root);
        assert!(pid.to_string_lossy().ends_with(".pid"));
    }

    #[test]
    fn test_socket_path_differs_for_different_projects() {
        let p1 = socket_path(&PathBuf::from("/tmp/project-a"));
        let p2 = socket_path(&PathBuf::from("/tmp/project-b"));
        assert_ne!(p1, p2);
    }

    #[test]
    fn test_try_daemon_no_daemon_flag() {
        let root = PathBuf::from("/tmp/nonexistent-project");
        // With no_daemon=true, should always return None immediately
        assert!(try_daemon(&root, "ls", "", "text", true).is_none());
    }
}
