use std::path::Path;

use anyhow::Result;

use crate::format::OutputFormat;
use crate::parser::rust as rust_parser;

/// Run the `context` command: parse a single file and print its symbols.
pub fn run(file: &Path, format_str: &str) -> Result<()> {
    let source = std::fs::read_to_string(file)
        .map_err(|e| anyhow::anyhow!("Cannot read {}: {}", file.display(), e))?;

    let ir = rust_parser::parse_file(file, &source)?;

    let output = match OutputFormat::from_str(format_str) {
        OutputFormat::Json => crate::format::json::format_symbols(&ir),
        OutputFormat::Text => crate::format::text::format_symbols(&ir),
    };

    println!("{}", output);
    Ok(())
}
