/// Path display optimization for text output.
///
/// Two modes:
/// - **SingleFile**: All symbols come from the same file. Emit the path once as a header,
///   drop it from every row (saves N-1 repetitions).
/// - **Alias**: Multiple files share a long common directory prefix. Emit a `[P] = prefix`
///   header and replace the prefix with `[P]` in every row.

const ALIAS_MARKER: &str = "[P]";
const MIN_PREFIX_LEN: usize = 20;

/// How to display paths in text output.
#[derive(Debug, Clone)]
pub enum PathDisplay {
    /// All symbols are from one file — show path once as header, omit from rows.
    SingleFile { path: String },
    /// Multiple files share a common prefix — use alias substitution.
    Alias(PathAlias),
}

/// The result of path alias computation (multi-file case).
#[derive(Debug, Clone)]
pub struct PathAlias {
    /// The common prefix that was factored out (always ends with `/`).
    pub prefix: String,
    /// The alias marker, e.g. `[P]`.
    pub marker: String,
}

impl PathDisplay {
    /// Format the header line(s) for text output.
    pub fn header(&self) -> String {
        match self {
            PathDisplay::SingleFile { path } => format!("# {}", path),
            PathDisplay::Alias(alias) => alias.header(),
        }
    }

    /// Shorten a file path for display in a row.
    /// For SingleFile, returns empty string (path is in the header).
    /// For Alias, replaces the prefix with the marker.
    pub fn shorten_file(&self, path: &str) -> String {
        match self {
            PathDisplay::SingleFile { .. } => String::new(),
            PathDisplay::Alias(alias) => alias.shorten(path),
        }
    }

    /// Format a location (file + line) for display in a row.
    /// For SingleFile, returns just `:line`.
    /// For Alias, returns `[P]rest/file.rs:line`.
    /// For no optimization, returns `file:line` as-is.
    pub fn format_loc(&self, path: &str, line: usize) -> String {
        match self {
            PathDisplay::SingleFile { .. } => format!(":{}", line),
            PathDisplay::Alias(alias) => format!("{}:{}", alias.shorten(path), line),
        }
    }
}

impl PathAlias {
    /// Format the alias header line for text output.
    pub fn header(&self) -> String {
        format!("{} = {}", self.marker, self.prefix)
    }

    /// Shorten a path by replacing the prefix with the alias marker.
    pub fn shorten(&self, path: &str) -> String {
        if let Some(rest) = path.strip_prefix(&self.prefix) {
            format!("{}{}", self.marker, rest)
        } else {
            path.to_string()
        }
    }
}

/// Compute the best path display strategy for a set of file paths.
///
/// Returns:
/// - `SingleFile` if all paths point to the same file
/// - `Alias` if multiple files share a long common directory prefix
/// - `None` if no optimization helps
pub fn compute_path_display(paths: &[&str]) -> Option<PathDisplay> {
    if paths.is_empty() {
        return None;
    }

    // Deduplicate
    let mut unique: Vec<&str> = paths.to_vec();
    unique.sort();
    unique.dedup();

    // Single file — all symbols come from the same path
    if unique.len() == 1 {
        return Some(PathDisplay::SingleFile {
            path: unique[0].to_string(),
        });
    }

    // Multiple files — try to find a common prefix alias
    let mut prefix = longest_common_prefix(&unique);

    // Truncate to last '/' boundary
    if let Some(pos) = prefix.rfind('/') {
        prefix = &prefix[..=pos]; // include the trailing '/'
    } else {
        return None; // no directory separator found
    }

    if prefix.len() > MIN_PREFIX_LEN {
        Some(PathDisplay::Alias(PathAlias {
            prefix: prefix.to_string(),
            marker: ALIAS_MARKER.to_string(),
        }))
    } else {
        None
    }
}

/// Backwards-compatible wrapper — returns just the alias if applicable.
pub fn compute_path_alias(paths: &[&str]) -> Option<PathAlias> {
    match compute_path_display(paths) {
        Some(PathDisplay::Alias(alias)) => Some(alias),
        _ => None,
    }
}

/// Find the longest common prefix of a slice of strings.
fn longest_common_prefix<'a>(strings: &[&'a str]) -> &'a str {
    if strings.is_empty() {
        return "";
    }
    let first = strings[0];
    let mut len = first.len();

    for s in &strings[1..] {
        len = len.min(s.len());
        for (i, (a, b)) in first.bytes().zip(s.bytes()).enumerate() {
            if a != b {
                len = len.min(i);
                break;
            }
        }
    }

    &first[..len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_file_display() {
        let paths = vec![
            "src/com/example/User.java",
            "src/com/example/User.java",
            "src/com/example/User.java",
        ];
        let display = compute_path_display(&paths).unwrap();
        match &display {
            PathDisplay::SingleFile { path } => {
                assert_eq!(path, "src/com/example/User.java");
            }
            _ => panic!("expected SingleFile"),
        }
        assert_eq!(display.header(), "# src/com/example/User.java");
        assert_eq!(display.format_loc("src/com/example/User.java", 10), ":10");
    }

    #[test]
    fn test_single_unique_file() {
        let paths = vec!["src/com/example/User.java"];
        let display = compute_path_display(&paths).unwrap();
        assert!(matches!(display, PathDisplay::SingleFile { .. }));
    }

    #[test]
    fn test_common_prefix_java_paths() {
        let paths = vec![
            "src/main/java/com/example/ecommerce/catalog/controller/ProductController.java",
            "src/main/java/com/example/ecommerce/catalog/service/InventoryService.java",
            "src/main/java/com/example/ecommerce/catalog/model/Order.java",
        ];
        let display = compute_path_display(&paths).unwrap();
        match &display {
            PathDisplay::Alias(alias) => {
                assert_eq!(alias.prefix, "src/main/java/com/example/ecommerce/catalog/");
                assert_eq!(alias.marker, "[P]");
            }
            _ => panic!("expected Alias"),
        }
        assert_eq!(
            display.header(),
            "[P] = src/main/java/com/example/ecommerce/catalog/"
        );
    }

    #[test]
    fn test_alias_format_loc() {
        let paths = vec![
            "src/main/java/com/example/ecommerce/catalog/controller/ProductController.java",
            "src/main/java/com/example/ecommerce/catalog/service/InventoryService.java",
        ];
        let display = compute_path_display(&paths).unwrap();
        assert_eq!(
            display.format_loc(
                "src/main/java/com/example/ecommerce/catalog/controller/ProductController.java",
                42
            ),
            "[P]controller/ProductController.java:42"
        );
    }

    #[test]
    fn test_no_alias_short_prefix() {
        let paths = vec!["src/foo/a.rs", "src/foo/b.rs"];
        // "src/foo/" is 8 chars, below MIN_PREFIX_LEN — no alias, not single file
        assert!(compute_path_display(&paths).is_none());
    }

    #[test]
    fn test_shorten_replaces_prefix() {
        let alias = PathAlias {
            prefix: "src/main/java/com/example/ecommerce/catalog/".to_string(),
            marker: "[P]".to_string(),
        };
        assert_eq!(
            alias.shorten("src/main/java/com/example/ecommerce/catalog/controller/Foo.java"),
            "[P]controller/Foo.java"
        );
    }

    #[test]
    fn test_shorten_no_match() {
        let alias = PathAlias {
            prefix: "src/main/java/com/example/".to_string(),
            marker: "[P]".to_string(),
        };
        assert_eq!(alias.shorten("other/path/Foo.java"), "other/path/Foo.java");
    }

    #[test]
    fn test_header_format() {
        let alias = PathAlias {
            prefix: "src/main/java/com/example/ecommerce/catalog/".to_string(),
            marker: "[P]".to_string(),
        };
        assert_eq!(
            alias.header(),
            "[P] = src/main/java/com/example/ecommerce/catalog/"
        );
    }

    #[test]
    fn test_prefix_at_directory_boundary() {
        let paths = vec![
            "src/main/java/com/example/alpha/Foo.java",
            "src/main/java/com/example/beta/Bar.java",
        ];
        let alias = compute_path_alias(&paths).unwrap();
        assert_eq!(alias.prefix, "src/main/java/com/example/");
        assert!(alias.prefix.ends_with('/'));
    }

    #[test]
    fn test_empty_paths() {
        let paths: Vec<&str> = vec![];
        assert!(compute_path_display(&paths).is_none());
    }

    #[test]
    fn test_compute_path_alias_backwards_compat() {
        // Single file — alias returns None
        let paths = vec!["src/foo.rs", "src/foo.rs"];
        assert!(compute_path_alias(&paths).is_none());

        // Multi-file with long prefix — alias returns Some
        let paths = vec![
            "src/main/java/com/example/alpha/Foo.java",
            "src/main/java/com/example/beta/Bar.java",
        ];
        assert!(compute_path_alias(&paths).is_some());
    }
}
