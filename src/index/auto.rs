use std::path::{Path, PathBuf};

use anyhow::Result;
use ignore::WalkBuilder;

use crate::ir::types::Ir;

use super::builder;
use super::store;
use super::types::Index;

const INDEX_DIR: &str = ".smartgrep";
const INDEX_FILE: &str = "index.json";

/// Walk up from `start` looking for a directory containing any registered
/// language's project marker (Cargo.toml, go.mod, package.json, ...).
pub fn detect_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        if crate::lang::project_markers().any(|m| current.join(m).exists()) {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Collect all supported source files under `root`, respecting .gitignore and
/// skipping hidden, build, cache and dependency directories.
pub fn collect_sources(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let walker = WalkBuilder::new(root)
        .hidden(true) // skip hidden files/dirs
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|entry| {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if crate::lang::is_skip_dir(name) {
                    return false;
                }
            }
            true
        })
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file() && crate::lang::is_source_file(path) {
            files.push(path.to_path_buf());
        }
    }
    // Directory read order is filesystem-dependent (ext4 hash order, APFS/NTFS
    // sorted); sort so symbol order, and so all output, is the same on every OS.
    files.sort();
    files
}

/// Return the path to the index file for a given project root.
pub fn index_path(root: &Path) -> PathBuf {
    root.join(INDEX_DIR).join(INDEX_FILE)
}

/// Check whether the index is stale (any source file newer than the index).
pub fn is_stale(root: &Path) -> bool {
    let idx_path = index_path(root);
    if !idx_path.exists() {
        return true;
    }

    let idx_mtime = match std::fs::metadata(&idx_path).and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return true,
    };

    let sources = collect_sources(root);
    for src in &sources {
        if let Ok(meta) = std::fs::metadata(src) {
            if let Ok(mtime) = meta.modified() {
                if mtime > idx_mtime {
                    return true;
                }
            }
        }
    }
    false
}

/// Extract the package name from Cargo.toml via simple text matching.
pub fn package_name(root: &Path) -> Option<String> {
    let cargo_path = root.join("Cargo.toml");
    let content = std::fs::read_to_string(cargo_path).ok()?;
    // Simple approach: look for name = "..." after [package]
    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if in_package {
            if let Some(rest) = trimmed.strip_prefix("name") {
                let rest = rest.trim();
                if let Some(rest) = rest.strip_prefix('=') {
                    let rest = rest.trim().trim_matches('"').trim_matches('\'');
                    return Some(rest.to_string());
                }
            }
        }
    }
    None
}

/// Parse all source files under root into a merged Ir.
pub fn parse_all_sources(root: &Path) -> Result<Ir> {
    let sources = collect_sources(root);
    let mut merged = Ir::default();

    for src_path in &sources {
        // Make paths relative to root for cleaner qualified names
        let rel_path = src_path
            .strip_prefix(root)
            .unwrap_or(src_path);

        let source = std::fs::read_to_string(src_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {}: {}", src_path.display(), e))?;

        let result = crate::parser::parse_by_extension(rel_path, &source);

        match result {
            Ok(ir) => {
                merged.symbols.extend(ir.symbols);
                merged.dependencies.extend(ir.dependencies);
            }
            Err(e) => {
                eprintln!("Warning: failed to parse {}: {}", src_path.display(), e);
            }
        }
    }

    Ok(merged)
}

/// Ensure an up-to-date index exists. Load from cache if fresh, otherwise rebuild.
pub fn ensure_index(root: &Path) -> Result<Index> {
    let idx_path = index_path(root);

    if idx_path.exists() && !is_stale(root) {
        // A load error (old format / version mismatch) means rebuild, not fail.
        if let Ok(index) = store::load(&idx_path) {
            return Ok(index);
        }
    }

    rebuild_index(root)
}

/// Force rebuild the index: parse all sources, build, save, return.
pub fn rebuild_index(root: &Path) -> Result<Index> {
    let ir = parse_all_sources(root)?;
    let index = builder::build(&ir);
    let idx_path = index_path(root);
    store::save(&index, &idx_path)?;
    Ok(index)
}
