use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const SKILL_CONTENT: &str = include_str!("../../SKILL.md");

/// Where the skill lives under `base` (a repo root, or `$HOME` for a global install).
pub fn skill_path(base: &Path) -> PathBuf {
    base.join(".claude").join("skills").join("smartgrep").join("SKILL.md")
}

/// Write the skill under `base`, returning the path written.
pub fn install_to(base: &Path) -> Result<PathBuf> {
    let skill_path = skill_path(base);
    if let Some(dir) = skill_path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&skill_path, SKILL_CONTENT)?;
    Ok(skill_path)
}

pub fn run(global: bool) -> Result<()> {
    // Repo-local installs stay relative to the current directory (historical behavior).
    let base = if global {
        PathBuf::from(std::env::var("HOME").context("HOME not set")?)
    } else {
        PathBuf::new()
    };
    let skill_path = install_to(&base)?;

    let scope = if global { "global" } else { "repo" };
    println!("Claude Code skill installed ({scope}): {}", skill_path.display());
    Ok(())
}
