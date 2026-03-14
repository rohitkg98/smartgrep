use anyhow::Result;

use crate::daemon::client;
use crate::index::auto;
use crate::query::{engine, parser};

/// Run the `query` command: parse and execute a composable query.
pub fn run(
    query_str: &str,
    format_str: &str,
    project_root: &Option<std::path::PathBuf>,
    use_daemon: bool,
) -> Result<()> {
    let root = super::resolve_root(project_root)?;

    // Try daemon first (auto-starts if needed, skipped if --no-daemon)
    if let Some(output) = client::try_daemon(&root, "query", query_str, format_str, use_daemon) {
        println!("{}", output);
        return Ok(());
    }

    let start = std::time::Instant::now();
    let index = auto::ensure_index(&root)?;

    let batch = parser::parse(query_str)?;
    let output = engine::execute_batch(&batch, &index, format_str)?;

    super::log_direct(&root, "query", query_str, &output, start.elapsed().as_millis() as u64);
    println!("{}", output);
    Ok(())
}
