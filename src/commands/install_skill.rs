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

/// The user's home directory: `$HOME`, else `%USERPROFILE%` (Windows, where
/// `HOME` is usually unset). Empty values count as unset.
pub fn home_dir(env: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .iter()
        .filter_map(|k| env(k))
        .find(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub fn run(global: bool) -> Result<()> {
    // Repo-local installs stay relative to the current directory (historical behavior).
    let base = if global {
        home_dir(|k| std::env::var_os(k)).context("home directory not found (set HOME or USERPROFILE)")?
    } else {
        PathBuf::new()
    };
    let skill_path = install_to(&base)?;

    let scope = if global { "global" } else { "repo" };
    println!("Claude Code skill installed ({scope}): {}", skill_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |k| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| OsString::from(v))
    }

    #[test]
    fn home_prefers_home() {
        let e = env(&[("HOME", "/home/u"), ("USERPROFILE", r"C:\Users\u")]);
        assert_eq!(home_dir(e), Some(PathBuf::from("/home/u")));
    }

    #[test]
    fn home_falls_back_to_userprofile() {
        assert_eq!(home_dir(env(&[("USERPROFILE", r"C:\Users\u")])), Some(PathBuf::from(r"C:\Users\u")));
        assert_eq!(home_dir(env(&[("HOME", ""), ("USERPROFILE", r"C:\Users\u")])), Some(PathBuf::from(r"C:\Users\u")));
    }

    #[test]
    fn home_missing() {
        assert_eq!(home_dir(env(&[])), None);
    }
}
