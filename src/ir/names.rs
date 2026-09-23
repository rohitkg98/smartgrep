//! Name normalization for `Dependency.to_name`.
//!
//! Parsers record targets as written in source (`crate::ir::types::Symbol`,
//! `fmt.Println`, `Processor<String>`). The index keys reverse lookups on
//! [`dep_target_key`] and `refs` / `implementing` match with [`dep_matches`],
//! so every language shares one set of rules.

/// Remove generic / subscript argument lists: `Processor<String>` → `Processor`,
/// `Vec::<i32>::new` → `Vec::new`, `Generic[T]` → `Generic`.
pub fn strip_generics(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '<' | '[' => depth += 1,
            '>' | ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    // `Vec::<i32>::new` leaves `Vec::::new`; `collect::<T>` leaves `collect::`.
    while out.contains("::::") {
        out = out.replace("::::", "::");
    }
    out.trim_end_matches(':').trim().to_string()
}

/// Split a path on any of the separators `::`, `.`, `/`, dropping empty
/// segments (leading `./`, `..x`, trailing `::`) and glob segments (`*`).
pub fn path_segments(s: &str) -> Vec<&str> {
    s.split(|c| c == ':' || c == '.' || c == '/')
        .map(str::trim)
        .filter(|seg| !seg.is_empty() && *seg != "*")
        .collect()
}

/// Reduce a dependency target (or a query) to the path it names, without
/// generics, aliases, grouped lists or reference/pointer sigils:
/// `use a::b as c` → `a::b`, `crate::x::{A, B}` → `crate::x`, `&dyn Foo<T>` → `Foo`.
fn normalize_target(to_name: &str) -> String {
    let mut s = to_name.trim();
    if let Some(pos) = s.find(" as ") {
        s = &s[..pos];
    }
    // Grouped Rust import that wasn't split by the parser: keep the prefix.
    if let Some(pos) = s.find('{') {
        s = &s[..pos];
    }
    let s = s
        .trim_start_matches(|c| c == '&' || c == '*')
        .trim_start_matches("mut ")
        .trim_start_matches("dyn ")
        .trim_start_matches("impl ");
    strip_generics(s)
}

/// Lookup key for a dependency target: the last path segment of the
/// normalized target. `crate::index::types::Index` → `Index`,
/// `fmt.Println` → `Println`, `Processor<String>` → `Processor`,
/// `github.com/x/y/models` → `models`, `crate::ir::types::*` → `types`.
pub fn dep_target_key(to_name: &str) -> String {
    let norm = normalize_target(to_name);
    path_segments(&norm).last().map(|s| s.to_string()).unwrap_or_default()
}

/// Does a dependency target match a (bare or qualified) query name?
/// Bare query: keys are equal. Qualified query: the dep's path segments end
/// with the query's segments (`::`, `.`, `/` equivalent), so `Symbol::new`
/// matches `Symbol::new` and `crate::ir::types::Symbol::new` but not `new`.
pub fn dep_matches(to_name: &str, query: &str) -> bool {
    let dep_norm = normalize_target(to_name);
    let q_norm = normalize_target(query);
    let dep_segs = path_segments(&dep_norm);
    let q_segs = path_segments(&q_norm);
    if q_segs.is_empty() || dep_segs.len() < q_segs.len() {
        return false;
    }
    dep_segs[dep_segs.len() - q_segs.len()..] == q_segs[..]
}

/// Secondary lookup key for a qualified call target: the segment naming the
/// callee's owner (`User::new` → `User`, `fmt.Println` → `fmt`,
/// `crate::index::store::load` → `store`). Indexing calls under it makes
/// `refs User` include `User::new(...)` call sites. `None` for bare names.
pub fn dep_qualifier_key(to_name: &str) -> Option<String> {
    let norm = normalize_target(to_name);
    let segs = path_segments(&norm);
    if segs.len() < 2 {
        return None;
    }
    let q = segs[segs.len() - 2];
    if matches!(q, "self" | "Self" | "super" | "crate") {
        return None;
    }
    Some(q.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_last_segment_without_generics() {
        assert_eq!(dep_target_key("crate::index::types::Index"), "Index");
        assert_eq!(dep_target_key("Index"), "Index");
        assert_eq!(dep_target_key("fmt.Println"), "Println");
        assert_eq!(dep_target_key("Processor<String>"), "Processor");
        assert_eq!(dep_target_key("Repository<Map<K, V>>"), "Repository");
        assert_eq!(dep_target_key("Generic[T]"), "Generic");
        assert_eq!(dep_target_key("collect::<Vec<_>>"), "collect");
        assert_eq!(dep_target_key("Vec::<u8>::new"), "new");
        assert_eq!(dep_target_key("github.com/acme/app/models"), "models");
        assert_eq!(dep_target_key("./models"), "models");
        assert_eq!(dep_target_key("..x.y"), "y");
        assert_eq!(dep_target_key("crate::ir::types::*"), "types");
        assert_eq!(dep_target_key("crate::ir::{Symbol, Dep}"), "ir");
        assert_eq!(dep_target_key("x::y as z"), "y");
        assert_eq!(dep_target_key("&dyn Foo<T>"), "Foo");
        assert_eq!(dep_target_key(""), "");
    }

    #[test]
    fn qualified_matching_is_segment_suffix() {
        assert!(dep_matches("crate::ir::types::Symbol::new", "Symbol::new"));
        assert!(dep_matches("Symbol::new", "Symbol::new"));
        assert!(!dep_matches("new", "Symbol::new"));
        assert!(!dep_matches("MySymbol::new", "Symbol::new"));
        assert!(dep_matches("std::fmt::Display", "fmt.Display"));
        assert!(dep_matches("Processor<String>", "Processor"));
    }

    #[test]
    fn qualifier_key() {
        assert_eq!(dep_qualifier_key("User::new").as_deref(), Some("User"));
        assert_eq!(dep_qualifier_key("fmt.Println").as_deref(), Some("fmt"));
        assert_eq!(dep_qualifier_key("Vec::<u8>::new").as_deref(), Some("Vec"));
        assert_eq!(dep_qualifier_key("Self::new"), None);
        assert_eq!(dep_qualifier_key("new"), None);
    }
}
