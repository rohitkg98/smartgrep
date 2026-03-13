pub mod context;
pub mod deps;
pub mod index_cmd;
pub mod log_cmd;
pub mod ls;
pub mod query;
pub mod refs;
pub mod show;

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::index::auto;

/// Resolve the project root from an explicit option or by walking up from cwd.
pub fn resolve_root(project_root: &Option<PathBuf>) -> Result<PathBuf> {
    if let Some(root) = project_root {
        return Ok(root.clone());
    }
    let cwd = std::env::current_dir()?;
    auto::detect_project_root(&cwd)
        .ok_or_else(|| anyhow::anyhow!("Could not find Cargo.toml in any parent directory"))
}

/// Log a command execution in direct (non-daemon) mode.
pub fn log_direct(project_root: &Path, command: &str, args: &str, output: &str, duration_ms: u64) {
    let result_count = crate::daemon::logger::count_results(output);
    let entry = crate::daemon::logger::make_entry(command, args, result_count, duration_ms);
    crate::daemon::logger::append(project_root, &entry);
}
