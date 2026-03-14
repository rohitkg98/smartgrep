use anyhow::{Context, Result};

const INSTALL_SCRIPT_URL: &str =
    "https://raw.githubusercontent.com/rohitkg98/smartgrep/main/install.sh";

pub fn run() -> Result<()> {
    // Check that sh and curl/wget are available before doing anything
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

enum Downloader {
    Curl,
    Wget,
}

fn pick_downloader() -> Option<Downloader> {
    if which("curl") {
        Some(Downloader::Curl)
    } else if which("wget") {
        Some(Downloader::Wget)
    } else {
        None
    }
}

fn which(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

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
