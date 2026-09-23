//! Path normalization: every path that enters the IR (and so the index and all
//! output) uses `/` as its separator, whatever the host OS. Windows hands us
//! `src\ir\types.rs`; we store and print `src/ir/types.rs`, so qualified names,
//! `loc.file`, and command output are byte-identical across platforms.
//!
//! The boundary is `parser::parse_by_extension`; user-supplied path filters
//! (`context <file>`, `--in`, `file contains`, `in '<path>'`) go through
//! [`to_slash`] so they match what's stored.

use std::path::{Path, PathBuf};

/// Replace `sep` with `/`. The OS-independent core of [`to_slash`], exposed so
/// tests can simulate Windows separators on any host.
pub fn to_slash_with(s: &str, sep: char) -> String {
    if sep == '/' {
        s.to_string()
    } else {
        s.replace(sep, "/")
    }
}

/// Convert a path string using the host's native separator to `/` form.
/// A no-op on Unix, where `\` is a legal file-name character.
pub fn to_slash(s: &str) -> String {
    to_slash_with(s, std::path::MAIN_SEPARATOR)
}

/// Render a path with `/` separators.
pub fn path_to_slash(p: &Path) -> String {
    to_slash(&p.to_string_lossy())
}

/// The `/`-separated form of `p`, as a `PathBuf` for storage in the IR.
pub fn normalize(p: &Path) -> PathBuf {
    PathBuf::from(path_to_slash(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_separators_become_slashes() {
        assert_eq!(to_slash_with(r"src\ir\types.rs", '\\'), "src/ir/types.rs");
        assert_eq!(to_slash_with(r"C:\proj\src\main.rs", '\\'), "C:/proj/src/main.rs");
        assert_eq!(to_slash_with(r"src\ir/", '\\'), "src/ir/");
    }

    #[test]
    fn slash_separator_is_identity() {
        assert_eq!(to_slash_with(r"src/a\b.rs", '/'), r"src/a\b.rs");
    }

    #[test]
    fn native_paths_normalize() {
        let p = Path::new("src").join("ir").join("types.rs");
        assert_eq!(path_to_slash(&p), "src/ir/types.rs");
        assert_eq!(normalize(&p), PathBuf::from("src/ir/types.rs"));
    }
}
