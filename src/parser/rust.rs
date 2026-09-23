use std::path::Path;

use anyhow::Result;
use tree_sitter::{Node, Parser};

use crate::ir::types::*;
use crate::parser::common::{call_target, loc, node_text, push_call_deps};

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
                    extract_calls(&child, source, path, &sym.qualified_name, ir);
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
                    let trait_name = sym.name.clone();
                    ir.symbols.push(sym);
                    extract_trait_default_methods(&child, source, path, prefix, &trait_name, ir);
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
                extract_use(&child, source, path, prefix, ir);
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

fn find_child_by_field<'a>(node: &Node<'a>, field: &str) -> Option<Node<'a>> {
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
        "method"
    } else {
        "fn"
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

    let mut sym = Symbol::new(name, qualified_name, kind, loc(node, path), vis);
    sym.signature = Some(sig);
    sym.parent = parent.map(String::from);
    sym.params = params;
    sym.return_type = return_type;
    Some(sym)
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

    let mut sym = Symbol::new(name, qualified_name, "struct", loc(node, path), vis);
    sym.fields = fields;
    Some(sym)
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

    Some(Symbol::new(name, qualified_name, "enum", loc(node, path), vis))
}

fn extract_trait(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Symbol> {
    let name = get_name(node, source)?;
    let vis = get_visibility_with_source(node, source);
    let qualified_name = format!("{}::{}", prefix, name);

    Some(Symbol::new(name, qualified_name, "trait", loc(node, path), vis))
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

    let mut sym = Symbol::new(impl_name, qualified_name.clone(), "impl", loc(node, path), Visibility::Private);
    sym.attributes = outer_attrs.to_vec();
    ir.symbols.push(sym);

    if let Some(tr) = &trait_name {
        ir.dependencies.push(Dependency {
            from_qualified: qualified_name.clone(),
            to_name: tr.clone(),
            kind: DepKind::Implements,
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
                        extract_calls(&child, source, path, &sym.qualified_name, ir);
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

/// Emit one Import dep per imported leaf, with its full path:
/// `use a::{B, c::D};` → `a::B`, `a::c::D`; `use x::y as z;` → `x::y`;
/// `use std::fmt::{self, Display};` → `std::fmt`, `std::fmt::Display`.
fn extract_use(node: &Node, source: &str, path: &Path, prefix: &str, ir: &mut Ir) {
    let Some(arg) = find_child_by_field(node, "argument") else {
        return;
    };
    let mut leaves = Vec::new();
    collect_use_leaves(&arg, source, "", &mut leaves);
    for (import_path, leaf) in leaves {
        ir.dependencies.push(Dependency {
            from_qualified: prefix.to_string(),
            to_name: import_path,
            kind: DepKind::Import,
            loc: loc(&leaf, path),
        });
    }
}

fn join_use_path(base: &str, rest: &str) -> String {
    let rest = rest.trim();
    match (base.is_empty(), rest.is_empty()) {
        (true, _) => rest.to_string(),
        (false, true) => base.to_string(),
        (false, false) => format!("{}::{}", base, rest),
    }
}

fn collect_use_leaves<'a>(
    node: &Node<'a>,
    source: &str,
    base: &str,
    out: &mut Vec<(String, Node<'a>)>,
) {
    match node.kind() {
        "scoped_use_list" => {
            let new_base = match find_child_by_field(node, "path") {
                Some(p) => join_use_path(base, node_text(&p, source)),
                None => base.to_string(),
            };
            if let Some(list) = find_child_by_field(node, "list") {
                collect_use_leaves(&list, source, &new_base, out);
            }
        }
        "use_list" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() == "line_comment" || child.kind() == "block_comment" {
                    continue;
                }
                collect_use_leaves(&child, source, base, out);
            }
        }
        "use_as_clause" => {
            if let Some(p) = find_child_by_field(node, "path") {
                collect_use_leaves(&p, source, base, out);
            }
        }
        // `{self, ...}` imports the list's base module itself.
        "self" if !base.is_empty() => out.push((base.to_string(), *node)),
        _ => {
            let text: String = node_text(node, source)
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            out.push((join_use_path(base, &text), *node));
        }
    }
}

/// Emit methods for trait items that have a default body, so their calls are
/// attributed (`crate::m::Trait::method`). Required methods (no body) are not
/// emitted as symbols.
fn extract_trait_default_methods(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    trait_name: &str,
    ir: &mut Ir,
) {
    let Some(body) = find_child_by_field(node, "body") else {
        return;
    };
    let mut pending_attrs: Vec<String> = Vec::new();
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "attribute_item" => pending_attrs.push(node_text(&child, source).to_string()),
            "function_item" => {
                if let Some(mut sym) =
                    extract_function(&child, source, path, prefix, Some(trait_name))
                {
                    sym.attributes = std::mem::take(&mut pending_attrs);
                    extract_calls(&child, source, path, &sym.qualified_name, ir);
                    ir.symbols.push(sym);
                } else {
                    pending_attrs.clear();
                }
            }
            _ => pending_attrs.clear(),
        }
    }
}

/// Emit `Call` deps for every call expression in a function's body. Calls in
/// closures and nested fns are attributed to this function. Macros are not
/// calls (and their token trees are not parsed), struct literals are skipped.
fn extract_calls(func: &Node, source: &str, path: &Path, from_qualified: &str, ir: &mut Ir) {
    let Some(body) = find_child_by_field(func, "body") else {
        return;
    };
    let mut calls = Vec::new();
    let mut stack = vec![body];
    while let Some(node) = stack.pop() {
        if node.kind() == "call_expression" {
            if let Some(callee) = find_child_by_field(&node, "function") {
                if let Some((name, name_node)) = rust_callee(&callee, source) {
                    calls.push((name, loc(&name_node, path)));
                }
            }
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    push_call_deps(&mut ir.dependencies, from_qualified, calls);
}

/// Prelude enum-variant constructors: syntactically calls, semantically noise.
const RUST_SKIP_CALLEES: &[&str] = &["Some", "Ok", "Err"];

/// Resolve a call's `function` node to the recorded callee name and the node
/// to use for its location.
fn rust_callee<'a>(callee: &Node<'a>, source: &str) -> Option<(String, Node<'a>)> {
    match callee.kind() {
        "identifier" | "scoped_identifier" => {
            let name = call_target(node_text(callee, source))?;
            if RUST_SKIP_CALLEES.contains(&name.as_str()) {
                return None;
            }
            Some((name, *callee))
        }
        // Method call on a receiver: only the method name is resolvable.
        "field_expression" => {
            let field = find_child_by_field(callee, "field")?;
            Some((call_target(node_text(&field, source))?, field))
        }
        // `foo::<T>()`, `x.collect::<Vec<_>>()`
        "generic_function" => {
            let inner = find_child_by_field(callee, "function")?;
            rust_callee(&inner, source)
        }
        _ => None,
    }
}

fn extract_const(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Symbol> {
    let name = get_name(node, source)?;
    let vis = get_visibility_with_source(node, source);
    let qualified_name = format!("{}::{}", prefix, name);

    Some(Symbol::new(name, qualified_name, "const", loc(node, path), vis))
}

fn extract_type_alias(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Symbol> {
    let name = get_name(node, source)?;
    let vis = get_visibility_with_source(node, source);
    let qualified_name = format!("{}::{}", prefix, name);

    Some(Symbol::new(name, qualified_name, "type", loc(node, path), vis))
}

fn extract_mod(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Symbol> {
    let name = get_name(node, source)?;
    let vis = get_visibility_with_source(node, source);
    let qualified_name = format!("{}::{}", prefix, name);

    Some(Symbol::new(name, qualified_name, "mod", loc(node, path), vis))
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
        assert_eq!(ir.symbols[0].kind, "fn");
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
            .filter(|s| s.kind == "struct")
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
