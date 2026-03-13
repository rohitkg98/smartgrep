use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

const SKILL_CONTENT: &str = include_str!("../../SKILL.md");

pub fn run(global: bool) -> Result<()> {
    let dest = if global {
        let home = std::env::var("HOME").context("HOME not set")?;
        PathBuf::from(home).join(".claude").join("skills").join("smartgrep")
    } else {
        PathBuf::from(".claude").join("skills").join("smartgrep")
    };

    fs::create_dir_all(&dest)?;
    let skill_path = dest.join("SKILL.md");
    fs::write(&skill_path, SKILL_CONTENT)?;

    let scope = if global { "global" } else { "repo" };
    println!("Claude Code skill installed ({scope}): {}", skill_path.display());
    Ok(())
}
