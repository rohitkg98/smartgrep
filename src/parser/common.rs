use std::path::Path;

use tree_sitter::Node;

use crate::ir::types::SourceLoc;

/// Convert a tree-sitter Node to a SourceLoc (1-indexed line/col).
pub fn loc(node: &Node, path: &Path) -> SourceLoc {
    let start = node.start_position();
    SourceLoc {
        file: path.to_path_buf(),
        line: start.row + 1,
        col: start.column + 1,
    }
}

/// Get the UTF-8 text of a tree-sitter Node from the source string.
pub fn node_text<'a>(node: &Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

/// Find the first direct child of a node with the given kind.
pub fn find_child_by_kind<'a>(node: &Node<'a>, kind: &str) -> Option<Node<'a>> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == kind {
                return Some(child);
            }
        }
    }
    None
}

/// Callee text as recorded in a `Call` dep: generics / turbofish removed
/// (`collect::<Vec<_>>` → `collect`, `Vec::<u8>::new` → `Vec::new`) and
/// whitespace trimmed. Returns `None` if nothing usable remains.
pub fn call_target(text: &str) -> Option<String> {
    let cleaned = strip_type_args(text);
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// Strip generic/type-argument sections and whitespace from a callee or type
/// name: `Foo<Bar>` → `Foo`, `Map<K, List<V>>.of` → `Map.of`,
/// `Generic[T]` → `Generic`, `Vec::<u8>::new` → `Vec::new`.
pub fn strip_type_args(text: &str) -> String {
    crate::ir::names::strip_generics(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// True if `name` starts with an uppercase letter (a class/type-like name).
pub fn starts_uppercase(name: &str) -> bool {
    name.chars().next().map_or(false, |c| c.is_uppercase())
}

/// True if `name` looks like an UPPER_SNAKE constant (`LOG`, `MAX_SIZE`): at least
/// two alphabetic chars and none of them lowercase.
pub fn is_all_caps(name: &str) -> bool {
    let mut alpha = 0;
    for c in name.chars().filter(|c| c.is_alphabetic()) {
        if c.is_lowercase() {
            return false;
        }
        alpha += 1;
    }
    alpha > 1
}

/// Visit every descendant of `node` (excluding `node` itself) in pre-order,
/// iteratively, so deeply nested bodies can't overflow the stack.
pub fn walk_descendants<'a>(node: Node<'a>, mut visit: impl FnMut(Node<'a>)) {
    let mut cursor = node.walk();
    if !cursor.goto_first_child() {
        return;
    }
    loop {
        visit(cursor.node());
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() || cursor.node() == node {
                return;
            }
        }
    }
}

/// Emit `DepKind::Call` deps for one function body.
///
/// `calls` holds `(callee, loc)` pairs in any order. They are sorted by source
/// position, empty names are dropped, and each callee is kept once (first
/// occurrence), so every `(from_qualified, to_name)` appears once per function.
pub fn push_call_deps(
    deps: &mut Vec<crate::ir::types::Dependency>,
    from_qualified: &str,
    mut calls: Vec<(String, SourceLoc)>,
) {
    use crate::ir::types::{DepKind, Dependency};
    calls.sort_by_key(|(_, l)| (l.line, l.col));
    let mut seen = std::collections::HashSet::new();
    for (to_name, loc) in calls {
        let to_name = to_name.trim().to_string();
        if to_name.is_empty() || !seen.insert(to_name.clone()) {
            continue;
        }
        deps.push(Dependency {
            from_qualified: from_qualified.to_string(),
            to_name,
            kind: DepKind::Call,
            loc,
        });
    }
}
