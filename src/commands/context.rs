use std::path::Path;

use anyhow::Result;

use crate::daemon::client;
use crate::format::OutputFormat;
use crate::index::auto;
use crate::parser::java as java_parser;
use crate::parser::rust as rust_parser;

/// Run the `context` command: parse a single file and print its symbols.
pub fn run(file: &Path, format_str: &str, no_daemon: bool) -> Result<()> {
    // Try daemon first — we need to resolve the project root for the socket path
    if !no_daemon {
        let cwd = std::env::current_dir()?;
        if let Some(root) = auto::detect_project_root(&cwd) {
            let args = file.to_string_lossy();
            if let Some(output) = client::try_daemon(&root, "context", &args, format_str, no_daemon) {
                println!("{}", output);
                return Ok(());
            }
        }
    }

    let source = std::fs::read_to_string(file)
        .map_err(|e| anyhow::anyhow!("Cannot read {}: {}", file.display(), e))?;

    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
    let ir = match ext {
        "java" => java_parser::parse_file(file, &source)?,
        _ => rust_parser::parse_file(file, &source)?,
    };

    let output = match OutputFormat::from_str(format_str) {
        OutputFormat::Json => crate::format::json::format_symbols(&ir),
        OutputFormat::Text => crate::format::text::format_symbols(&ir),
    };

    println!("{}", output);
    Ok(())
}
