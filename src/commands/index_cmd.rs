use anyhow::Result;
use std::path::PathBuf;

use crate::index::auto;

/// Run the `index` command: force rebuild the index and print a summary.
pub fn run(project_root: &Option<PathBuf>) -> Result<()> {
    let root = super::resolve_root(project_root)?;

    // Delete existing index first
    let idx_path = auto::index_path(&root);
    if idx_path.exists() {
        std::fs::remove_file(&idx_path)?;
    }

    let sources = auto::collect_sources(&root);
    let file_count = sources.len();

    let index = auto::rebuild_index(&root)?;

    println!(
        "Indexed {} symbols, {} dependencies from {} files",
        index.symbols.len(),
        index.deps.len(),
        file_count,
    );

    Ok(())
}
