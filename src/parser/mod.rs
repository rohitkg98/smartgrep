pub mod common;
pub mod go;
pub mod java;
pub mod rust;
pub mod typescript;

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::ir::types::Ir;

/// Parse a source file by dispatching to the appropriate language parser based on extension.
/// The set of supported languages lives in `crate::lang::LANGUAGES`.
pub fn parse_by_extension(path: &Path, source: &str) -> Result<Ir> {
    match crate::lang::language_for_path(path) {
        Some(lang) => (lang.parse)(path, source),
        None => {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            Err(anyhow!(
                "Unsupported file type '.{}'. smartgrep supports {} files.",
                ext,
                crate::lang::supported_extensions_display()
            ))
        }
    }
}
