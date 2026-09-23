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
    let cleaned: String = crate::ir::names::strip_generics(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// Emit `DepKind::Call` deps for one function body.
///
/// `calls` holds `(callee, loc)` pairs in any order. They are sorted by source
/// position and deduplicated per callee, keeping the first occurrence, so each
/// `(from_qualified, to_name)` appears once per function.
pub fn push_call_deps(
    ir: &mut crate::ir::types::Ir,
    from_qualified: &str,
    mut calls: Vec<(String, SourceLoc)>,
) {
    use crate::ir::types::{DepKind, Dependency};
    calls.sort_by_key(|(_, l)| (l.line, l.col));
    let mut seen = std::collections::HashSet::new();
    for (to_name, loc) in calls {
        if seen.insert(to_name.clone()) {
            ir.dependencies.push(Dependency {
                from_qualified: from_qualified.to_string(),
                to_name,
                kind: DepKind::Call,
                loc,
            });
        }
    }
}
