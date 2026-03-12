use std::path::Path;

use anyhow::Result;

use super::types::Index;

/// Save an index to disk using bincode serialization.
pub fn save(index: &Index, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let encoded = bincode::serialize(index)?;
    std::fs::write(path, encoded)?;
    Ok(())
}

/// Load an index from disk using bincode deserialization.
pub fn load(path: &Path) -> Result<Index> {
    let data = std::fs::read(path)?;
    let index: Index = bincode::deserialize(&data)?;
    Ok(index)
}
