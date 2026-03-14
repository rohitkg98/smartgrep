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
