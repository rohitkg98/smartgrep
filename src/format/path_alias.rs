/// Path alias detection for text output.
///
/// Finds the longest common directory prefix among a set of file paths,
/// and if it saves significant tokens (prefix > 20 chars, 2+ unique paths),
/// returns an alias mapping so output can use `[P]` instead of the full prefix.

const ALIAS_MARKER: &str = "[P]";
const MIN_PREFIX_LEN: usize = 20;
const MIN_UNIQUE_PATHS: usize = 2;

/// The result of path alias computation.
#[derive(Debug, Clone)]
pub struct PathAlias {
    /// The common prefix that was factored out (always ends with `/`).
    pub prefix: String,
    /// The alias marker, e.g. `[P]`.
    pub marker: String,
}

impl PathAlias {
    /// Format the alias header line for text output.
    pub fn header(&self) -> String {
        format!("[paths] {} = {}", self.prefix, self.marker)
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

/// Compute a path alias from a list of file paths.
///
/// Returns `Some(PathAlias)` if:
/// - There are at least `MIN_UNIQUE_PATHS` unique paths
/// - The longest common directory prefix is longer than `MIN_PREFIX_LEN` chars
///
/// Returns `None` otherwise (no savings from aliasing).
pub fn compute_path_alias(paths: &[&str]) -> Option<PathAlias> {
    if paths.is_empty() {
        return None;
    }

    // Deduplicate
    let mut unique: Vec<&str> = paths.to_vec();
    unique.sort();
    unique.dedup();

    if unique.len() < MIN_UNIQUE_PATHS {
        return None;
    }

    // Find longest common prefix
    let mut prefix = longest_common_prefix(&unique);

    // Truncate to last '/' boundary
    if let Some(pos) = prefix.rfind('/') {
        prefix = &prefix[..=pos]; // include the trailing '/'
    } else {
        return None; // no directory separator found
    }

    if prefix.len() > MIN_PREFIX_LEN {
        Some(PathAlias {
            prefix: prefix.to_string(),
            marker: ALIAS_MARKER.to_string(),
        })
    } else {
        None
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
    fn test_common_prefix_java_paths() {
        let paths = vec![
            "src/main/java/com/example/ecommerce/catalog/controller/ProductController.java",
            "src/main/java/com/example/ecommerce/catalog/service/InventoryService.java",
            "src/main/java/com/example/ecommerce/catalog/model/Order.java",
        ];
        let alias = compute_path_alias(&paths).unwrap();
        assert_eq!(alias.prefix, "src/main/java/com/example/ecommerce/catalog/");
        assert_eq!(alias.marker, "[P]");
    }

    #[test]
    fn test_no_alias_short_prefix() {
        let paths = vec![
            "src/foo/a.rs",
            "src/foo/b.rs",
        ];
        // "src/foo/" is 8 chars, below MIN_PREFIX_LEN
        assert!(compute_path_alias(&paths).is_none());
    }

    #[test]
    fn test_no_alias_single_file() {
        let paths = vec![
            "src/main/java/com/example/ecommerce/catalog/Foo.java",
        ];
        assert!(compute_path_alias(&paths).is_none());
    }

    #[test]
    fn test_no_alias_single_unique_file() {
        // Same path twice — only 1 unique
        let paths = vec![
            "src/main/java/com/example/ecommerce/catalog/Foo.java",
            "src/main/java/com/example/ecommerce/catalog/Foo.java",
        ];
        assert!(compute_path_alias(&paths).is_none());
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
            "[paths] src/main/java/com/example/ecommerce/catalog/ = [P]"
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
        assert!(compute_path_alias(&paths).is_none());
    }
}
