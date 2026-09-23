//! Language registry: the single place that knows which languages smartgrep
//! supports, which file extensions map to which parser, which files mark a
//! project root, and which directories to skip while walking sources.
//!
//! To add a language: write `src/parser/<lang>.rs` exposing
//! `parse_file(&Path, &str) -> Result<Ir>`, then add an entry to `LANGUAGES`.

use std::path::Path;

use anyhow::Result;

use crate::ir::types::Ir;
use crate::parser;

/// A supported source language.
pub struct Language {
    /// Lowercase language name (e.g. "rust", "typescript").
    pub name: &'static str,
    /// File extensions without the leading dot.
    pub extensions: &'static [&'static str],
    /// Parser entry point: path (relative to project root) + source text → IR.
    pub parse: fn(&Path, &str) -> Result<Ir>,
    /// Files whose presence marks a project root.
    pub project_markers: &'static [&'static str],
    /// Directory names to skip while collecting sources (build output, caches, deps).
    pub skip_dirs: &'static [&'static str],
}

/// Directories skipped regardless of language.
/// `target` is shared by Cargo and Maven, so it lives here.
pub const GLOBAL_SKIP_DIRS: &[&str] = &[".smartgrep", "target"];

pub static LANGUAGES: &[Language] = &[
    Language {
        name: "rust",
        extensions: &["rs"],
        parse: parser::rust::parse_file,
        project_markers: &["Cargo.toml"],
        skip_dirs: &[],
    },
    Language {
        name: "java",
        extensions: &["java"],
        parse: parser::java::parse_file,
        project_markers: &["pom.xml", "build.gradle", "build.gradle.kts"],
        skip_dirs: &[],
    },
    Language {
        name: "go",
        extensions: &["go"],
        parse: parser::go::parse_file,
        project_markers: &["go.mod"],
        skip_dirs: &[],
    },
    Language {
        name: "typescript",
        extensions: &["ts", "tsx"],
        parse: parser::typescript::parse_file,
        project_markers: &["package.json", "tsconfig.json"],
        skip_dirs: &["node_modules"],
    },
];

/// Find the language that handles a file, by extension.
pub fn language_for_path(path: &Path) -> Option<&'static Language> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    LANGUAGES.iter().find(|l| l.extensions.contains(&ext))
}

/// True if the path has an extension handled by some registered language.
pub fn is_source_file(path: &Path) -> bool {
    language_for_path(path).is_some()
}

/// True if a directory with this name should be skipped while walking sources.
pub fn is_skip_dir(name: &str) -> bool {
    GLOBAL_SKIP_DIRS.contains(&name) || LANGUAGES.iter().any(|l| l.skip_dirs.contains(&name))
}

/// True if any directory component of `path` is a skipped directory.
pub fn is_in_skipped_dir(path: &Path) -> bool {
    path.parent()
        .map(|p| {
            p.components()
                .any(|c| c.as_os_str().to_str().map_or(false, is_skip_dir))
        })
        .unwrap_or(false)
}

/// Iterator over every project marker file name across all languages.
pub fn project_markers() -> impl Iterator<Item = &'static str> {
    LANGUAGES.iter().flat_map(|l| l.project_markers.iter().copied())
}

/// Human-readable list of supported extensions, e.g. ".rs, .java, .go, .ts, .tsx".
pub fn supported_extensions_display() -> String {
    LANGUAGES
        .iter()
        .flat_map(|l| l.extensions.iter())
        .map(|e| format!(".{}", e))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_by_extension() {
        assert_eq!(language_for_path(Path::new("a/b.rs")).unwrap().name, "rust");
        assert_eq!(language_for_path(Path::new("x.tsx")).unwrap().name, "typescript");
        assert!(language_for_path(Path::new("README.md")).is_none());
        assert!(language_for_path(Path::new("Makefile")).is_none());
    }

    #[test]
    fn extensions_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for l in LANGUAGES {
            for e in l.extensions {
                assert!(seen.insert(*e), "extension {} registered twice", e);
            }
        }
    }

    #[test]
    fn skip_dirs() {
        assert!(is_skip_dir("target"));
        assert!(is_skip_dir(".smartgrep"));
        assert!(is_skip_dir("node_modules"));
        assert!(!is_skip_dir("src"));
        assert!(is_in_skipped_dir(Path::new("/p/node_modules/x/a.ts")));
        assert!(!is_in_skipped_dir(Path::new("/p/src/target.rs")));
    }
}
