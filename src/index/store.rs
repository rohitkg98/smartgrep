use std::path::Path;

use anyhow::Result;

use super::types::Index;

/// Save the index to disk as JSON.
///
/// JSON is intentionally chosen over a binary format for now:
/// - Human-readable — inspect with `cat .smartgrep/index.json | jq .`
/// - No hidden schema mismatch surprises; a corrupt or stale index fails
///   loudly with a clear parse error rather than silent garbage
/// - Easy to diff across versions during development
///
/// See docs/FUTURE_INDEX_FORMAT.md for the plan to move to a versioned
/// binary format once the schema stabilises and load time becomes measurable.
pub fn save(index: &Index, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(index)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load the index from a JSON file on disk.
///
/// Returns an error if the file cannot be read or parsed. Callers treat any
/// load error as a stale/missing index and trigger a rebuild.
pub fn load(path: &Path) -> Result<Index> {
    let data = std::fs::read_to_string(path)?;
    let index: Index = serde_json::from_str(&data)
        .map_err(|_| anyhow::anyhow!("index format changed — re-indexing"))?;
    if index.version != super::types::INDEX_VERSION {
        return Err(anyhow::anyhow!("index version mismatch — re-indexing"));
    }
    Ok(index)
}
