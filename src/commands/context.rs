use std::path::Path;

use anyhow::Result;

use crate::daemon::client;
use crate::format::OutputFormat;
use crate::index::auto;

/// Run the `context` command: parse a single file and print its symbols.
pub fn run(file: &Path, format_str: &str, use_daemon: bool) -> Result<()> {
    // Try daemon first — we need to resolve the project root for the socket path
    if use_daemon {
        let cwd = std::env::current_dir()?;
        if let Some(root) = auto::detect_project_root(&cwd) {
            let args = file.to_string_lossy();
            if let Some(output) = client::try_daemon(&root, "context", &args, format_str, use_daemon) {
                println!("{}", output);
                return Ok(());
            }
        }
    }

    let start = std::time::Instant::now();

    let source = std::fs::read_to_string(file)
        .map_err(|e| anyhow::anyhow!("Cannot read {}: {}", file.display(), e))?;

    let ir = crate::parser::parse_by_extension(file, &source)?;

    let output = match format_str.parse::<OutputFormat>().unwrap() {
        OutputFormat::Json => crate::format::json::format_symbols(&ir),
        OutputFormat::Text => crate::format::text::format_symbols(&ir),
    };

    // Log in direct mode
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(root) = auto::detect_project_root(&cwd) {
            let args = file.to_string_lossy();
            super::log_direct(&root, "context", &args, &output, start.elapsed().as_millis() as u64);
        }
    }

    println!("{}", output);
    Ok(())
}
