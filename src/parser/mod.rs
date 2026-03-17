pub mod common;
pub mod go;
pub mod java;
pub mod rust;
pub mod typescript;

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::ir::types::Ir;

/// Parse a source file by dispatching to the appropriate language parser based on extension.
pub fn parse_by_extension(path: &Path, source: &str) -> Result<Ir> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "rs" => rust::parse_file(path, source),
        "java" => java::parse_file(path, source),
        "go" => go::parse_file(path, source),
        "ts" | "tsx" => typescript::parse_file(path, source),
        other => Err(anyhow!(
            "Unsupported file type '.{}'. smartgrep supports .rs, .java, .go, .ts, and .tsx files.",
            other
        )),
    }
}
