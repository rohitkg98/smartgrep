use anyhow::{Context, Result};

#[cfg(not(windows))]
const INSTALL_SCRIPT_URL: &str =
    "https://raw.githubusercontent.com/rohitkg98/smartgrep/main/install.sh";

#[cfg(windows)]
const INSTALL_PS1_URL: &str =
    "https://raw.githubusercontent.com/rohitkg98/smartgrep/main/install.ps1";

/// Windows: hand off to the PowerShell installer. A running .exe can't be
/// overwritten in place; install.ps1 moves the old binary aside, so all we do
/// is run it and exit without touching our own file afterwards.
#[cfg(windows)]
pub fn run() -> Result<()> {
    println!("Updating smartgrep...");

    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"])
        .arg(format!("irm {} | iex", INSTALL_PS1_URL))
        .status()
        .context("Failed to run PowerShell installer (is powershell on PATH?)")?;

    if !status.success() {
        anyhow::bail!("Update script exited with status {}", status);
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn run() -> Result<()> {
    // Check that curl/wget are available before doing anything
    let downloader = pick_downloader().context(
        "Neither curl nor wget found. Install one and retry.",
    )?;

    println!("Updating smartgrep...");

    let script = fetch_script(&downloader)?;

    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(&script)
        .status()
        .context("Failed to run install script")?;

    if !status.success() {
        anyhow::bail!("Update script exited with status {}", status);
    }

    Ok(())
}

#[cfg(not(windows))]
enum Downloader {
    Curl,
    Wget,
}

#[cfg(not(windows))]
fn pick_downloader() -> Option<Downloader> {
    if runs("curl", "--version") {
        Some(Downloader::Curl)
    } else if runs("wget", "--version") {
        Some(Downloader::Wget)
    } else {
        None
    }
}

/// Whether `cmd` exists: `cmd arg` can be spawned (its exit code is ignored;
/// BusyBox wget rejects `--version`). Portable replacement for `which`, which
/// isn't installed everywhere.
#[cfg(not(windows))]
fn runs(cmd: &str, arg: &str) -> bool {
    std::process::Command::new(cmd)
        .arg(arg)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

#[cfg(not(windows))]
fn fetch_script(downloader: &Downloader) -> Result<String> {
    let output = match downloader {
        Downloader::Curl => std::process::Command::new("curl")
            .args(["-fsSL", INSTALL_SCRIPT_URL])
            .output()
            .context("curl failed to fetch install script")?,
        Downloader::Wget => std::process::Command::new("wget")
            .args(["-qO-", INSTALL_SCRIPT_URL])
            .output()
            .context("wget failed to fetch install script")?,
    };

    if !output.status.success() {
        anyhow::bail!(
            "Failed to download install script from {}: {}",
            INSTALL_SCRIPT_URL,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8(output.stdout).context("Install script contained non-UTF8 bytes")
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn runs_detects_missing_command() {
        assert!(!runs("smartgrep-definitely-not-a-command", "--version"));
    }

    #[test]
    fn runs_detects_present_command() {
        assert!(runs("sh", "-c"));
    }
}
