use std::path::Path;

use anyhow::Result;
use tree_sitter::{Node, Parser};

use crate::ir::types::*;

/// Derive a qualified module prefix from a file path.
/// `src/index/builder.rs` -> `crate::index::builder`
/// `src/main.rs` -> `crate`
/// `src/lib.rs` -> `crate`
fn module_prefix_from_path(path: &Path) -> String {
    let path_str = path.to_string_lossy();

    // Strip leading src/ if present
    let stripped = if let Some(rest) = path_str.strip_prefix("src/") {
        rest
    } else {
        &path_str
    };

    // Remove .rs extension
    let without_ext = stripped.strip_suffix(".rs").unwrap_or(stripped);

    // main.rs and lib.rs map to crate root
    if without_ext == "main" || without_ext == "lib" {
        return "crate".to_string();
    }

    // Strip trailing /mod for mod.rs files
    let module_path = without_ext.strip_suffix("/mod").unwrap_or(without_ext);

    format!("crate::{}", module_path.replace('/', "::"))
}

/// Parse a Rust source file and return the IR.
pub fn parse_file(path: &Path, source: &str) -> Result<Ir> {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser.set_language(&language.into())?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter failed to parse {}", path.display()))?;

    let prefix = module_prefix_from_path(path);
    let mut ir = Ir::default();

    extract_items(tree.root_node(), source, path, &prefix, None, &mut ir);

    Ok(ir)
}

fn extract_items(
    node: Node,
    source: &str,
    path: &Path,
    prefix: &str,
    parent: Option<&str>,
    ir: &mut Ir,
) {
    let mut pending_attrs: Vec<String> = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "attribute_item" => {
                pending_attrs.push(node_text(&child, source).to_string());
            }
            "function_item" => {
                if let Some(mut sym) = extract_function(&child, source, path, prefix, parent) {
                    sym.attributes = std::mem::take(&mut pending_attrs);
                    ir.symbols.push(sym);
                } else {
                    pending_attrs.clear();
                }
            }
            "struct_item" => {
                if let Some(mut sym) = extract_struct(&child, source, path, prefix) {
                    sym.attributes = std::mem::take(&mut pending_attrs);
                    ir.symbols.push(sym);
                } else {
                    pending_attrs.clear();
                }
            }
            "enum_item" => {
                if let Some(mut sym) = extract_enum(&child, source, path, prefix) {
                    sym.attributes = std::mem::take(&mut pending_attrs);
                    ir.symbols.push(sym);
                } else {
                    pending_attrs.clear();
                }
            }
            "trait_item" => {
                if let Some(mut sym) = extract_trait(&child, source, path, prefix) {
                    sym.attributes = std::mem::take(&mut pending_attrs);
                    ir.symbols.push(sym);
                } else {
                    pending_attrs.clear();
                }
            }
            "impl_item" => {
                let attrs = std::mem::take(&mut pending_attrs);
                extract_impl(&child, source, path, prefix, &attrs, ir);
            }
            "use_declaration" => {
                pending_attrs.clear();
                if let Some(dep) = extract_use(&child, source, path, prefix) {
                    ir.dependencies.push(dep);
                }
            }
            "const_item" => {
                if let Some(mut sym) = extract_const(&child, source, path, prefix) {
                    sym.attributes = std::mem::take(&mut pending_attrs);
                    ir.symbols.push(sym);
                } else {
                    pending_attrs.clear();
                }
            }
            "type_item" => {
                if let Some(mut sym) = extract_type_alias(&child, source, path, prefix) {
                    sym.attributes = std::mem::take(&mut pending_attrs);
                    ir.symbols.push(sym);
                } else {
                    pending_attrs.clear();
                }
            }
            "mod_item" => {
                if let Some(mut sym) = extract_mod(&child, source, path, prefix) {
                    sym.attributes = std::mem::take(&mut pending_attrs);
                    ir.symbols.push(sym);
                } else {
                    pending_attrs.clear();
                }
            }
            _ => {}
        }
    }
}

fn loc(node: &Node, path: &Path) -> SourceLoc {
    let start = node.start_position();
    SourceLoc {
        file: path.to_path_buf(),
        line: start.row + 1,
        col: start.column + 1,
    }
}

fn node_text<'a>(node: &Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn find_child_by_field<'a>(node: &'a Node<'a>, field: &str) -> Option<Node<'a>> {
    node.child_by_field_name(field)
}

fn get_name(node: &Node, source: &str) -> Option<String> {
    find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string())
}

fn get_visibility_with_source(node: &Node, source: &str) -> Visibility {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            let text = node_text(&child, source);
            if text.contains("crate") {
                return Visibility::Crate;
            }
            return Visibility::Public;
        }
    }
    Visibility::Private
}

fn extract_function(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    parent: Option<&str>,
) -> Option<Symbol> {
    let name = get_name(node, source)?;
    let vis = get_visibility_with_source(node, source);

    let kind = if parent.is_some() {
        SymbolKind::Method
    } else {
        SymbolKind::Function
    };

    let qualified_name = if let Some(p) = parent {
        format!("{}::{}::{}", prefix, p, name)
    } else {
        format!("{}::{}", prefix, name)
    };

    let params = extract_params(node, source);

    let return_type = find_child_by_field(node, "return_type").map(|rt| {
        let text = node_text(&rt, source);
        text.trim().to_string()
    });

    let sig = build_function_signature(node, source);

    Some(Symbol {
        name,
        qualified_name,
        kind,
        loc: loc(node, path),
        visibility: vis,
        signature: Some(sig),
        parent: parent.map(String::from),
        attributes: vec![],
        fields: vec![],
        params,
        return_type,
    })
}

fn build_function_signature(node: &Node, source: &str) -> String {
    let full = node_text(node, source);
    if let Some(pos) = full.find('{') {
        full[..pos].trim().to_string()
    } else {
        full.lines().next().unwrap_or("").trim().to_string()
    }
}

fn extract_params(node: &Node, source: &str) -> Vec<Param> {
    let mut params = Vec::new();
    if let Some(param_list) = find_child_by_field(node, "parameters") {
        let mut cursor = param_list.walk();
        for child in param_list.children(&mut cursor) {
            match child.kind() {
                "parameter" => {
                    let name = find_child_by_field(&child, "pattern")
                        .map(|n| node_text(&n, source).to_string())
                        .unwrap_or_default();
                    let type_name = find_child_by_field(&child, "type")
                        .map(|n| node_text(&n, source).to_string())
                        .unwrap_or_default();
                    if !name.is_empty() {
                        params.push(Param { name, type_name });
                    }
                }
                "self_parameter" => {
                    let text = node_text(&child, source);
                    params.push(Param {
                        name: "self".to_string(),
                        type_name: text.to_string(),
                    });
                }
                _ => {}
            }
        }
    }
    params
}

fn extract_struct(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Symbol> {
    let name = get_name(node, source)?;
    let vis = get_visibility_with_source(node, source);
    let qualified_name = format!("{}::{}", prefix, name);
    let fields = extract_struct_fields(node, source);

    Some(Symbol {
        name,
        qualified_name,
        kind: SymbolKind::Struct,
        loc: loc(node, path),
        visibility: vis,
        signature: None,
        parent: None,
        attributes: vec![],
        fields,
        params: vec![],
        return_type: None,
    })
}

fn extract_struct_fields(node: &Node, source: &str) -> Vec<Field> {
    let mut fields = Vec::new();
    if let Some(body) = find_child_by_field(node, "body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "field_declaration" {
                let name = get_name(&child, source).unwrap_or_default();
                let type_name = find_child_by_field(&child, "type")
                    .map(|n| node_text(&n, source).to_string())
                    .unwrap_or_default();
                let vis = get_visibility_with_source(&child, source);
                if !name.is_empty() {
                    fields.push(Field {
                        name,
                        type_name,
                        visibility: vis,
                    });
                }
            }
        }
    }
    fields
}

fn extract_enum(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Symbol> {
    let name = get_name(node, source)?;
    let vis = get_visibility_with_source(node, source);
    let qualified_name = format!("{}::{}", prefix, name);

    Some(Symbol {
        name,
        qualified_name,
        kind: SymbolKind::Enum,
        loc: loc(node, path),
        visibility: vis,
        signature: None,
        parent: None,
        attributes: vec![],
        fields: vec![],
        params: vec![],
        return_type: None,
    })
}

fn extract_trait(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Symbol> {
    let name = get_name(node, source)?;
    let vis = get_visibility_with_source(node, source);
    let qualified_name = format!("{}::{}", prefix, name);

    Some(Symbol {
        name,
        qualified_name,
        kind: SymbolKind::Trait,
        loc: loc(node, path),
        visibility: vis,
        signature: None,
        parent: None,
        attributes: vec![],
        fields: vec![],
        params: vec![],
        return_type: None,
    })
}

fn extract_impl(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    outer_attrs: &[String],
    ir: &mut Ir,
) {
    let type_name = find_child_by_field(node, "type")
        .map(|n| node_text(&n, source).to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let qualified_name = format!("{}::{}", prefix, type_name);

    let trait_name = find_child_by_field(node, "trait")
        .map(|n| node_text(&n, source).to_string());

    let impl_name = if let Some(ref tr) = trait_name {
        format!("impl {} for {}", tr, type_name)
    } else {
        format!("impl {}", type_name)
    };

    ir.symbols.push(Symbol {
        name: impl_name,
        qualified_name: qualified_name.clone(),
        kind: SymbolKind::Impl,
        loc: loc(node, path),
        visibility: Visibility::Private,
        signature: None,
        parent: None,
        attributes: outer_attrs.to_vec(),
        fields: vec![],
        params: vec![],
        return_type: None,
    });

    if let Some(tr) = &trait_name {
        ir.dependencies.push(Dependency {
            from_qualified: qualified_name.clone(),
            to_name: tr.clone(),
            kind: DepKind::TraitImpl,
            loc: loc(node, path),
        });
    }

    // Extract methods inside the impl body
    if let Some(body) = find_child_by_field(node, "body") {
        // Use the same pending-attrs approach for methods inside impl blocks
        let mut pending_attrs: Vec<String> = Vec::new();
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "attribute_item" => {
                    pending_attrs.push(node_text(&child, source).to_string());
                }
                "function_item" => {
                    if let Some(mut sym) =
                        extract_function(&child, source, path, prefix, Some(&type_name))
                    {
                        sym.attributes = std::mem::take(&mut pending_attrs);
                        ir.symbols.push(sym);
                    } else {
                        pending_attrs.clear();
                    }
                }
                "const_item" => {
                    if let Some(mut sym) = extract_const(&child, source, path, prefix) {
                        sym.attributes = std::mem::take(&mut pending_attrs);
                        ir.symbols.push(sym);
                    } else {
                        pending_attrs.clear();
                    }
                }
                "type_item" => {
                    if let Some(mut sym) = extract_type_alias(&child, source, path, prefix) {
                        sym.attributes = std::mem::take(&mut pending_attrs);
                        ir.symbols.push(sym);
                    } else {
                        pending_attrs.clear();
                    }
                }
                _ => {}
            }
        }
    }
}

fn extract_use(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Dependency> {
    let text = node_text(node, source).to_string();
    let import_path = text
        .strip_prefix("use ")
        .unwrap_or(&text)
        .trim_end_matches(';')
        .trim()
        .to_string();

    Some(Dependency {
        from_qualified: prefix.to_string(),
        to_name: import_path,
        kind: DepKind::Import,
        loc: loc(node, path),
    })
}

fn extract_const(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Symbol> {
    let name = get_name(node, source)?;
    let vis = get_visibility_with_source(node, source);
    let qualified_name = format!("{}::{}", prefix, name);

    Some(Symbol {
        name,
        qualified_name,
        kind: SymbolKind::Const,
        loc: loc(node, path),
        visibility: vis,
        signature: None,
        parent: None,
        attributes: vec![],
        fields: vec![],
        params: vec![],
        return_type: None,
    })
}

fn extract_type_alias(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Symbol> {
    let name = get_name(node, source)?;
    let vis = get_visibility_with_source(node, source);
    let qualified_name = format!("{}::{}", prefix, name);

    Some(Symbol {
        name,
        qualified_name,
        kind: SymbolKind::TypeAlias,
        loc: loc(node, path),
        visibility: vis,
        signature: None,
        parent: None,
        attributes: vec![],
        fields: vec![],
        params: vec![],
        return_type: None,
    })
}

fn extract_mod(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Symbol> {
    let name = get_name(node, source)?;
    let vis = get_visibility_with_source(node, source);
    let qualified_name = format!("{}::{}", prefix, name);

    Some(Symbol {
        name,
        qualified_name,
        kind: SymbolKind::Module,
        loc: loc(node, path),
        visibility: vis,
        signature: None,
        parent: None,
        attributes: vec![],
        fields: vec![],
        params: vec![],
        return_type: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_prefix_from_path() {
        assert_eq!(module_prefix_from_path(Path::new("src/main.rs")), "crate");
        assert_eq!(module_prefix_from_path(Path::new("src/lib.rs")), "crate");
        assert_eq!(
            module_prefix_from_path(Path::new("src/parser/rust.rs")),
            "crate::parser::rust"
        );
        assert_eq!(
            module_prefix_from_path(Path::new("src/ir/mod.rs")),
            "crate::ir"
        );
    }

    #[test]
    fn test_parse_simple_function() {
        let source = "pub fn hello(x: i32) -> String { todo!() }";
        let ir = parse_file(Path::new("src/main.rs"), source).unwrap();
        assert_eq!(ir.symbols.len(), 1);
        assert_eq!(ir.symbols[0].name, "hello");
        assert_eq!(ir.symbols[0].kind, SymbolKind::Function);
        assert_eq!(ir.symbols[0].visibility, Visibility::Public);
        assert_eq!(ir.symbols[0].params.len(), 1);
        assert_eq!(ir.symbols[0].params[0].name, "x");
    }

    #[test]
    fn test_parse_struct_with_fields() {
        let source = r#"
pub struct Point {
    pub x: f64,
    pub y: f64,
}
"#;
        let ir = parse_file(Path::new("src/types.rs"), source).unwrap();
        let structs: Vec<_> = ir
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .collect();
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].name, "Point");
        assert_eq!(structs[0].fields.len(), 2);
        assert_eq!(structs[0].fields[0].name, "x");
    }

    #[test]
    fn test_attributes_collected_from_siblings() {
        let source = r#"
#[derive(Debug)]
pub struct Foo {
    pub x: i32,
}
"#;
        let ir = parse_file(Path::new("src/main.rs"), source).unwrap();
        let foo = ir
            .symbols
            .iter()
            .find(|s| s.name == "Foo")
            .expect("should find Foo");
        assert!(!foo.attributes.is_empty());
        assert!(foo.attributes[0].contains("derive"));
    }
}
