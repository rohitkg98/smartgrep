use anyhow::Result;

use crate::index::auto;
use crate::query::{engine, parser};

/// Run the `query` command: parse and execute a composable query.
pub fn run(
    query_str: &str,
    format_str: &str,
    project_root: &Option<std::path::PathBuf>,
) -> Result<()> {
    let root = resolve_root(project_root)?;
    let index = auto::ensure_index(&root)?;

    let batch = parser::parse(query_str)?;
    let output = engine::execute_batch(&batch, &index, format_str)?;

    println!("{}", output);
    Ok(())
}

fn resolve_root(project_root: &Option<std::path::PathBuf>) -> Result<std::path::PathBuf> {
    if let Some(root) = project_root {
        return Ok(root.clone());
    }
    let cwd = std::env::current_dir()?;
    auto::detect_project_root(&cwd)
        .ok_or_else(|| anyhow::anyhow!("Could not find Cargo.toml in any parent directory"))
}
